//! Safe publication of derived render artifacts.
//!
//! A renderer writes a manifest and content-addressed objects into a staging
//! directory. This module validates every declared object and atomically moves
//! the directory into the immutable commit cache. The renderer is deliberately
//! outside this module: a worker may use web-ifc, a native engine, or a future
//! service without gaining authority over Vex commits.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::Digest;
use uuid::Uuid;
use vex_bridge_protocol as proto;

use crate::errors::{BridgeError, BridgeResult};

/// Create an empty staging directory for a render worker.
///
/// Staging is placed beside the final artifact so publication is an atomic
/// filesystem rename. Call [`publish_staged_artifact`] only after the worker
/// has written `manifest.json` and all declared `objects/<sha256>` files.
pub fn create_artifact_staging_dir(project_dir: &Path, commit_hash: &str) -> BridgeResult<PathBuf> {
    validate_commit_hash(commit_hash)?;
    let root = render_cache_root(project_dir);
    fs::create_dir_all(&root)?;
    let staging = root.join(format!(".{commit_hash}-{}.partial", Uuid::now_v7()));
    fs::create_dir(&staging)?;
    Ok(staging)
}

/// Validate and atomically publish a worker-generated artifact.
///
/// The cache accepts exactly one artifact per complete commit hash. A renderer
/// configuration change must use a distinct cache root in a future schema
/// revision rather than overwriting a published artifact in place.
pub fn publish_staged_artifact(
    project_dir: &Path,
    project_id: &str,
    commit_hash: &str,
    staging_dir: &Path,
) -> BridgeResult<PathBuf> {
    validate_commit_hash(commit_hash)?;
    let root = render_cache_root(project_dir);
    let staging = validate_staging_dir(&root, staging_dir)?;
    let manifest_path = staging.join("manifest.json");
    require_regular_file(&manifest_path, "render artifact manifest")?;
    let raw = fs::read_to_string(&manifest_path)?;
    let mut manifest: proto::RenderArtifactManifest = serde_json::from_str(&raw)?;
    validate_manifest_identity(&manifest, project_id, commit_hash)?;
    canonicalize_resource_uris(&mut manifest, project_id, commit_hash);
    write_validated_manifest(&staging, &manifest)?;
    verify_declared_objects(&staging, &manifest)?;

    let final_dir = root.join(commit_hash);
    if final_dir.exists() {
        return Err(BridgeError::Config(format!(
            "a render artifact is already published for commit `{commit_hash}`"
        )));
    }
    fs::rename(staging, &final_dir)?;
    Ok(final_dir)
}

/// Root owned by the bridge for immutable render outputs.
#[must_use]
pub fn render_cache_root(project_dir: &Path) -> PathBuf {
    project_dir.join(".vex").join("cache").join("render")
}

/// Filename of the bridge-owned, per-artifact access marker. It records the
/// unix time of the last valid, permitted serve so eviction can order
/// completed artifacts least-recently-used. The leading dot keeps it out of
/// the published-artifact enumeration and it is never a content object.
const ACCESS_MARKER: &str = ".accessed";

/// Directory name of the embedded Node render runtime living under the cache
/// root. It is never a commit hash, so eviction skips it, but naming it keeps
/// the intent explicit.
const RUNTIME_DIR_PREFIX: &str = "render-runtime";

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Process-global registry of published render artifacts that are currently
/// being read for an HTTP serve, keyed by the canonicalized artifact
/// directory. Eviction never removes a directory with a live guard, so a
/// client download can never race a delete.
fn active_serves() -> &'static Mutex<HashMap<PathBuf, usize>> {
    static SERVES: OnceLock<Mutex<HashMap<PathBuf, usize>>> = OnceLock::new();
    SERVES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Canonicalize an artifact directory to a stable eviction/serve key, falling
/// back to the literal path when the directory cannot be resolved (e.g. it was
/// already removed).
fn serve_key(artifact_dir: &Path) -> PathBuf {
    artifact_dir
        .canonicalize()
        .unwrap_or_else(|_| artifact_dir.to_path_buf())
}

/// RAII guard that marks a published render artifact directory as actively
/// served. While any guard for a directory is alive, [`prune_render_cache`]
/// will not evict it. Acquire it before reading an artifact's bytes and hold
/// it until the response (and any access-tracking write) is complete.
#[must_use = "hold the guard for the duration of the serve"]
pub struct ArtifactServeGuard {
    key: PathBuf,
}

impl ArtifactServeGuard {
    pub fn acquire(artifact_dir: &Path) -> Self {
        let key = serve_key(artifact_dir);
        *active_serves()
            .lock()
            .expect("render serve registry poisoned")
            .entry(key.clone())
            .or_insert(0) += 1;
        Self { key }
    }
}

impl Drop for ArtifactServeGuard {
    fn drop(&mut self) {
        let mut registry = active_serves()
            .lock()
            .expect("render serve registry poisoned");
        if let Some(count) = registry.get_mut(&self.key) {
            *count -= 1;
            if *count == 0 {
                registry.remove(&self.key);
            }
        }
    }
}

fn is_being_served(artifact_dir: &Path) -> bool {
    let key = serve_key(artifact_dir);
    active_serves()
        .lock()
        .expect("render serve registry poisoned")
        .contains_key(&key)
}

/// Record that a completed artifact was just served to a permitted client.
///
/// The recorded timestamp drives least-recently-used eviction. Writing it is
/// best-effort: a lost marker only makes an artifact look older, which can at
/// worst evict it sooner, never incorrectly. Callers must only invoke this
/// after a fully validated, authorized serve.
pub fn record_artifact_access(artifact_dir: &Path) {
    let marker = artifact_dir.join(ACCESS_MARKER);
    let _ = fs::write(marker, now_unix().to_string());
}

/// Last-access time used to order eviction. Prefers the access marker written
/// by [`record_artifact_access`]; falls back to the validated manifest's mtime
/// so a never-served artifact still has a stable age.
fn artifact_last_access(artifact_dir: &Path) -> i64 {
    if let Ok(contents) = fs::read_to_string(artifact_dir.join(ACCESS_MARKER)) {
        if let Ok(timestamp) = contents.trim().parse::<i64>() {
            return timestamp;
        }
    }
    fs::metadata(artifact_dir.join("manifest.validated.json"))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// True only for a complete 64-character hex commit hash — the exact form of a
/// published artifact directory name.
fn is_commit_hash(name: &str) -> bool {
    name.len() == 64 && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Total size, in bytes, of the regular files inside `dir`. Symlinks are never
/// followed, so a symlink planted in an artifact directory can neither inflate
/// the measured size nor cause traversal outside the directory.
fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let meta = match fs::symlink_metadata(entry.path()) {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                total += meta.len();
            }
        }
    }
    total
}

/// Outcome of a cache-pruning pass.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RenderCachePruneReport {
    /// Commit hashes whose artifact directories were evicted, oldest first.
    pub evicted: Vec<String>,
    /// Bytes retained across all completed artifacts after pruning.
    pub retained_bytes: u64,
    /// Number of completed artifacts considered as candidates.
    pub scanned: usize,
}

/// Enforce a soft byte cap on a project's local render artifact cache by
/// evicting least-recently-served *completed* artifacts.
///
/// Safety guarantees, all enforced here rather than trusted from the caller:
///
/// * Only directories named by a complete commit hash that contain the
///   bridge-owned `manifest.validated.json` are eviction candidates. Staging
///   partials (`.<hash>-<uuid>.partial`), the embedded worker runtime, and any
///   other entry are never touched.
/// * `protected_commits` (the current HEAD and any artifact a worker is still
///   building) are never evicted.
/// * An artifact with a live [`ArtifactServeGuard`] is never evicted.
/// * Symlinks are never followed, and immediately before each removal the
///   directory is re-checked to be a real directory whose canonical parent is
///   the cache root, so eviction can never delete an arbitrary path target.
///
/// The cap is a target, not a hard ceiling: if protected/served artifacts
/// alone exceed it, usage stays above the cap rather than deleting live data.
pub fn prune_render_cache(
    project_dir: &Path,
    max_bytes: u64,
    protected_commits: &HashSet<String>,
) -> BridgeResult<RenderCachePruneReport> {
    let root = render_cache_root(project_dir);
    let canonical_root = match root.canonicalize() {
        Ok(canonical) => canonical,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RenderCachePruneReport::default());
        }
        Err(error) => return Err(BridgeError::Io(error)),
    };

    struct Candidate {
        commit: String,
        dir: PathBuf,
        size: u64,
        last_access: i64,
        protected: bool,
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut total: u64 = 0;
    for entry in fs::read_dir(&canonical_root)? {
        let entry = entry?;
        let name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => continue,
        };
        // Only published artifacts — whose directory name is a full commit
        // hash — are eligible. This intentionally excludes dot-prefixed staging
        // partials and the `render-runtime*` worker payload.
        debug_assert!(!name.starts_with(RUNTIME_DIR_PREFIX) || !is_commit_hash(&name));
        if !is_commit_hash(&name) {
            continue;
        }
        let dir = canonical_root.join(&name);
        let meta = fs::symlink_metadata(&dir)?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        // Without the bridge-owned validated manifest the directory is not a
        // completed, servable artifact (e.g. an interrupted publish); leave it.
        if !dir.join("manifest.validated.json").is_file() {
            continue;
        }
        let size = dir_size(&dir);
        total += size;
        let protected = protected_commits.contains(&name) || is_being_served(&dir);
        let last_access = artifact_last_access(&dir);
        candidates.push(Candidate {
            commit: name,
            dir,
            size,
            last_access,
            protected,
        });
    }

    let scanned = candidates.len();
    if total <= max_bytes {
        return Ok(RenderCachePruneReport {
            evicted: Vec::new(),
            retained_bytes: total,
            scanned,
        });
    }

    // Evict least-recently-served, unprotected, completed artifacts until the
    // cache is back under capacity (or nothing evictable remains).
    let mut evictable: Vec<&Candidate> = candidates
        .iter()
        .filter(|candidate| !candidate.protected)
        .collect();
    evictable.sort_by(|a, b| {
        a.last_access
            .cmp(&b.last_access)
            .then_with(|| a.commit.cmp(&b.commit))
    });

    let mut evicted = Vec::new();
    for candidate in evictable {
        if total <= max_bytes {
            break;
        }
        // Re-verify liveness and containment immediately before deletion so a
        // serve that started after enumeration, or a symlink swapped in to
        // redirect the delete, cannot be harmed or exploited.
        if is_being_served(&candidate.dir) {
            continue;
        }
        let meta = match fs::symlink_metadata(&candidate.dir) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        match candidate.dir.canonicalize() {
            Ok(resolved) if resolved.parent() == Some(canonical_root.as_path()) => {}
            _ => continue,
        }
        fs::remove_dir_all(&candidate.dir)?;
        total -= candidate.size;
        evicted.push(candidate.commit.clone());
    }

    Ok(RenderCachePruneReport {
        evicted,
        retained_bytes: total,
        scanned,
    })
}

fn validate_commit_hash(commit_hash: &str) -> BridgeResult<()> {
    if commit_hash.len() == 64 && commit_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(BridgeError::Config(
            "render artifacts require a complete 64-character commit hash".into(),
        ))
    }
}

fn validate_staging_dir(root: &Path, staging_dir: &Path) -> BridgeResult<PathBuf> {
    let metadata = fs::symlink_metadata(staging_dir)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(BridgeError::Config(
            "render artifact staging directory must be a regular directory".into(),
        ));
    }
    let root = root.canonicalize()?;
    let staging = staging_dir.canonicalize()?;
    if staging.parent() != Some(root.as_path())
        || !staging
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.') && name.ends_with(".partial"))
    {
        return Err(BridgeError::Config(
            "render artifact staging directory is not owned by the bridge cache".into(),
        ));
    }
    Ok(staging)
}

/// Store the normalized manifest under a filename that the worker cannot
/// pre-create. The server reads only this file after publication; it never
/// follows the worker-supplied `manifest.json` again.
fn write_validated_manifest(
    staging: &Path,
    manifest: &proto::RenderArtifactManifest,
) -> BridgeResult<()> {
    let path = staging.join("manifest.validated.json");
    let bytes = serde_json::to_vec_pretty(manifest)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            BridgeError::Config(format!(
                "could not create bridge-owned validated render manifest: {error}"
            ))
        })?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn validate_manifest_identity(
    manifest: &proto::RenderArtifactManifest,
    project_id: &str,
    commit_hash: &str,
) -> BridgeResult<()> {
    manifest.validate().map_err(|error| {
        BridgeError::Config(format!(
            "render artifact manifest failed validation: {error}"
        ))
    })?;
    if manifest.project_id != project_id || manifest.commit_hash != commit_hash {
        return Err(BridgeError::Config(
            "render artifact manifest identity does not match its target commit".into(),
        ));
    }
    Ok(())
}

fn canonicalize_resource_uris(
    manifest: &mut proto::RenderArtifactManifest,
    project_id: &str,
    commit_hash: &str,
) {
    for resource in manifest.resources_mut() {
        resource.uri = format!(
            "/v1/projects/{project_id}/render/{commit_hash}/objects/{}",
            resource.sha256
        );
    }
}

fn verify_declared_objects(
    staging: &Path,
    manifest: &proto::RenderArtifactManifest,
) -> BridgeResult<()> {
    let mut verified = HashSet::new();
    for resource in manifest.resources() {
        if !verified.insert(&resource.sha256) {
            continue;
        }
        let path = staging.join("objects").join(&resource.sha256);
        require_regular_file(&path, "render artifact object")?;
        let bytes = fs::read(&path)?;
        if bytes.len() as u64 != resource.byte_length {
            return Err(BridgeError::Config(format!(
                "render artifact object `{}` length does not match its manifest",
                resource.sha256
            )));
        }
        let actual = format!("{:x}", sha2::Sha256::digest(&bytes));
        if actual != resource.sha256 {
            return Err(BridgeError::Config(format!(
                "render artifact object `{}` does not match its manifest digest",
                resource.sha256
            )));
        }
    }
    Ok(())
}

fn require_regular_file(path: &Path, label: &str) -> BridgeResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(BridgeError::Config(format!(
            "{label} must be a regular file inside the staging directory"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root() -> PathBuf {
        std::env::temp_dir().join(format!("vex-render-artifact-test-{}", Uuid::now_v7()))
    }

    fn resource(bytes: &[u8]) -> proto::RenderArtifactResource {
        proto::RenderArtifactResource {
            uri: "/render/object".into(),
            content_type: "application/octet-stream".into(),
            sha256: format!("{:x}", sha2::Sha256::digest(bytes)),
            byte_length: bytes.len() as u64,
        }
    }

    fn manifest(
        project_id: &str,
        commit_hash: &str,
        bytes: &[u8],
    ) -> proto::RenderArtifactManifest {
        manifest_with(project_id, commit_hash, bytes, bytes)
    }

    /// Build a v2 manifest whose single full-model tile carries its own
    /// tile-local semantic index, optionally backed by distinct object bytes.
    fn manifest_with(
        project_id: &str,
        commit_hash: &str,
        tile_bytes: &[u8],
        index_bytes: &[u8],
    ) -> proto::RenderArtifactManifest {
        proto::RenderArtifactManifest {
            schema: proto::schema::RENDER_MANIFEST_V2.into(),
            project_id: project_id.into(),
            commit_hash: commit_hash.into(),
            artifact_id: "test-artifact".into(),
            generated_at: "2026-07-17T00:00:00Z".into(),
            render_policy: Some(proto::RenderPolicy {
                id: "vex.render-policy.full-model/1".into(),
                hash: format!("{:x}", sha2::Sha256::digest(b"policy")),
            }),
            tiles: vec![proto::RenderTileDescriptor {
                tile_id: "full-model".into(),
                lod: 0,
                bounds: proto::RenderBounds {
                    min: [0.0, 0.0, 0.0],
                    max: [1.0, 1.0, 1.0],
                },
                geometric_error: 0.0,
                artifact: resource(tile_bytes),
                semantic_index: Some(proto::RenderSemanticIndexDescriptor {
                    schema: proto::schema::RENDER_SEMANTIC_INDEX.into(),
                    entry_count: 1,
                    artifact: resource(index_bytes),
                }),
            }],
            semantic_index: None,
        }
    }

    #[test]
    fn publishes_valid_staged_artifact_atomically() {
        let project_dir = test_root();
        let commit = "a".repeat(64);
        let bytes = b"tile-and-semantic-index";
        let staging = create_artifact_staging_dir(&project_dir, &commit).unwrap();
        let manifest = manifest("project-a", &commit, bytes);
        let object_dir = staging.join("objects");
        fs::create_dir_all(&object_dir).unwrap();
        fs::write(object_dir.join(&manifest.tiles[0].artifact.sha256), bytes).unwrap();
        fs::write(
            staging.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();

        let published =
            publish_staged_artifact(&project_dir, "project-a", &commit, &staging).unwrap();

        assert!(published.join("manifest.validated.json").is_file());
        assert!(!staging.exists());
        let published_manifest: proto::RenderArtifactManifest =
            serde_json::from_slice(&fs::read(published.join("manifest.validated.json")).unwrap())
                .unwrap();
        assert_eq!(
            published_manifest.tiles[0].artifact.uri,
            format!(
                "/v1/projects/project-a/render/{commit}/objects/{}",
                published_manifest.tiles[0].artifact.sha256
            )
        );
        fs::remove_dir_all(project_dir).unwrap();
    }

    #[test]
    fn publishes_v2_artifact_with_distinct_per_tile_index() {
        // The full-model tile and its tile-local semantic index are separate
        // content-addressed objects; publication must verify and canonicalize
        // both.
        let project_dir = test_root();
        let commit = "d".repeat(64);
        let tile_bytes = b"full-model-tile-glb";
        let index_bytes = b"full-model-semantic-index";
        let staging = create_artifact_staging_dir(&project_dir, &commit).unwrap();
        let manifest = manifest_with("project-d", &commit, tile_bytes, index_bytes);
        let object_dir = staging.join("objects");
        fs::create_dir_all(&object_dir).unwrap();
        fs::write(
            object_dir.join(&manifest.tiles[0].artifact.sha256),
            tile_bytes,
        )
        .unwrap();
        let index_sha = &manifest.tiles[0]
            .semantic_index
            .as_ref()
            .unwrap()
            .artifact
            .sha256;
        fs::write(object_dir.join(index_sha), index_bytes).unwrap();
        fs::write(
            staging.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();

        let published =
            publish_staged_artifact(&project_dir, "project-d", &commit, &staging).unwrap();

        let published_manifest: proto::RenderArtifactManifest =
            serde_json::from_slice(&fs::read(published.join("manifest.validated.json")).unwrap())
                .unwrap();
        let tile = &published_manifest.tiles[0];
        let index = tile.semantic_index.as_ref().unwrap();
        assert_ne!(tile.artifact.sha256, index.artifact.sha256);
        assert_eq!(
            tile.artifact.uri,
            format!(
                "/v1/projects/project-d/render/{commit}/objects/{}",
                tile.artifact.sha256
            )
        );
        assert_eq!(
            index.artifact.uri,
            format!(
                "/v1/projects/project-d/render/{commit}/objects/{}",
                index.artifact.sha256
            )
        );
        fs::remove_dir_all(project_dir).unwrap();
    }

    #[test]
    fn rejects_tampered_artifact_before_publication() {
        let project_dir = test_root();
        let commit = "b".repeat(64);
        let staging = create_artifact_staging_dir(&project_dir, &commit).unwrap();
        let manifest = manifest("project-b", &commit, b"expected");
        let object_dir = staging.join("objects");
        fs::create_dir_all(&object_dir).unwrap();
        fs::write(
            object_dir.join(&manifest.tiles[0].artifact.sha256),
            b"tampered",
        )
        .unwrap();
        fs::write(
            staging.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();

        let error =
            publish_staged_artifact(&project_dir, "project-b", &commit, &staging).unwrap_err();

        assert!(error.to_string().contains("length") || error.to_string().contains("digest"));
        assert!(!project_dir.join(".vex/cache/render").join(&commit).exists());
        fs::remove_dir_all(project_dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_worker_manifest() {
        use std::os::unix::fs::symlink;

        let project_dir = test_root();
        let commit = "c".repeat(64);
        let staging = create_artifact_staging_dir(&project_dir, &commit).unwrap();
        let outside = project_dir.join("outside.json");
        fs::write(&outside, "{}").unwrap();
        fs::remove_file(staging.join("manifest.json")).unwrap_or(());
        symlink(&outside, staging.join("manifest.json")).unwrap();

        let error =
            publish_staged_artifact(&project_dir, "project-c", &commit, &staging).unwrap_err();

        assert!(error.to_string().contains("regular file"));
        assert_eq!(fs::read_to_string(outside).unwrap(), "{}");
        fs::remove_dir_all(project_dir).unwrap();
    }

    /// Stage, verify, and atomically publish an artifact whose single object
    /// carries `bytes`, returning the published directory.
    fn publish_artifact(
        project_dir: &Path,
        project_id: &str,
        commit: &str,
        bytes: &[u8],
    ) -> PathBuf {
        let staging = create_artifact_staging_dir(project_dir, commit).unwrap();
        let manifest = manifest(project_id, commit, bytes);
        let object_dir = staging.join("objects");
        fs::create_dir_all(&object_dir).unwrap();
        fs::write(object_dir.join(&manifest.tiles[0].artifact.sha256), bytes).unwrap();
        fs::write(
            staging.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        publish_staged_artifact(project_dir, project_id, commit, &staging).unwrap()
    }

    fn set_access(artifact_dir: &Path, unix: i64) {
        fs::write(artifact_dir.join(ACCESS_MARKER), unix.to_string()).unwrap();
    }

    #[test]
    fn prune_noops_when_under_capacity() {
        let project_dir = test_root();
        let commit = "1".repeat(64);
        let dir = publish_artifact(&project_dir, "project", &commit, b"payload-bytes");

        let report = prune_render_cache(&project_dir, u64::MAX, &HashSet::new()).unwrap();

        assert!(report.evicted.is_empty());
        assert_eq!(report.scanned, 1);
        assert_eq!(report.retained_bytes, dir_size(&dir));
        assert!(dir.exists());
        fs::remove_dir_all(project_dir).unwrap();
    }

    #[test]
    fn prune_evicts_least_recently_served_completed_artifact() {
        let project_dir = test_root();
        let old_commit = "2".repeat(64);
        let new_commit = "3".repeat(64);
        let old_dir = publish_artifact(&project_dir, "project", &old_commit, &[7u8; 4096]);
        let new_dir = publish_artifact(&project_dir, "project", &new_commit, &[9u8; 4096]);
        set_access(&old_dir, 100);
        set_access(&new_dir, 200);

        // A cap that fits only the newest artifact forces exactly one eviction.
        let cap = dir_size(&new_dir);
        let report = prune_render_cache(&project_dir, cap, &HashSet::new()).unwrap();

        assert_eq!(report.evicted, vec![old_commit.clone()]);
        assert!(!old_dir.exists());
        assert!(new_dir.exists());
        assert!(report.retained_bytes <= cap);
        fs::remove_dir_all(project_dir).unwrap();
    }

    #[test]
    fn prune_orders_by_recorded_access_not_publish_time() {
        // Publishing `old` first then serving it must make it survive over the
        // later-published-but-never-served `new` artifact.
        let project_dir = test_root();
        let old_commit = "4".repeat(64);
        let new_commit = "5".repeat(64);
        let old_dir = publish_artifact(&project_dir, "project", &old_commit, &[1u8; 4096]);
        let new_dir = publish_artifact(&project_dir, "project", &new_commit, &[2u8; 4096]);
        set_access(&new_dir, 200);
        // A real, permitted serve of the older artifact touches its marker.
        record_artifact_access(&old_dir);

        let cap = dir_size(&old_dir);
        let report = prune_render_cache(&project_dir, cap, &HashSet::new()).unwrap();

        assert_eq!(report.evicted, vec![new_commit.clone()]);
        assert!(old_dir.exists());
        assert!(!new_dir.exists());
        fs::remove_dir_all(project_dir).unwrap();
    }

    #[test]
    fn prune_never_evicts_protected_head_or_building_artifact() {
        let project_dir = test_root();
        let protected_commit = "6".repeat(64);
        let evictable_commit = "7".repeat(64);
        let protected_dir =
            publish_artifact(&project_dir, "project", &protected_commit, &[3u8; 4096]);
        let evictable_dir =
            publish_artifact(&project_dir, "project", &evictable_commit, &[4u8; 4096]);
        // The protected artifact is the least-recently-served, so LRU alone
        // would evict it first — protection must override recency.
        set_access(&protected_dir, 1);
        set_access(&evictable_dir, 999);

        let mut protected = HashSet::new();
        protected.insert(protected_commit.clone());
        let report = prune_render_cache(&project_dir, 0, &protected).unwrap();

        assert_eq!(report.evicted, vec![evictable_commit.clone()]);
        assert!(protected_dir.exists());
        assert!(!evictable_dir.exists());
        fs::remove_dir_all(project_dir).unwrap();
    }

    #[test]
    fn prune_never_evicts_artifact_being_served() {
        let project_dir = test_root();
        let served_commit = "8".repeat(64);
        let idle_commit = "9".repeat(64);
        let served_dir = publish_artifact(&project_dir, "project", &served_commit, &[5u8; 4096]);
        let idle_dir = publish_artifact(&project_dir, "project", &idle_commit, &[6u8; 4096]);
        set_access(&served_dir, 1);
        set_access(&idle_dir, 999);

        // While a serve guard is alive, even a zero cap must keep the served
        // artifact and evict only the idle one.
        let guard = ArtifactServeGuard::acquire(&served_dir);
        let report = prune_render_cache(&project_dir, 0, &HashSet::new()).unwrap();
        drop(guard);

        assert_eq!(report.evicted, vec![idle_commit.clone()]);
        assert!(served_dir.exists());
        assert!(!idle_dir.exists());
        fs::remove_dir_all(project_dir).unwrap();
    }

    #[test]
    fn prune_ignores_staging_partials_and_non_artifact_entries() {
        let project_dir = test_root();
        let published_commit = "a".repeat(64);
        let published_dir =
            publish_artifact(&project_dir, "project", &published_commit, &[8u8; 4096]);

        // A worker's in-flight staging directory and the embedded runtime must
        // survive an aggressive prune.
        let staging = create_artifact_staging_dir(&project_dir, &"b".repeat(64)).unwrap();
        let runtime = render_cache_root(&project_dir).join("render-runtime-v1");
        fs::create_dir_all(&runtime).unwrap();
        let stray = render_cache_root(&project_dir).join("not-a-commit-hash");
        fs::create_dir_all(&stray).unwrap();

        let report = prune_render_cache(&project_dir, 0, &HashSet::new()).unwrap();

        // Only the one published artifact is a candidate; it alone is evicted.
        assert_eq!(report.scanned, 1);
        assert_eq!(report.evicted, vec![published_commit]);
        assert!(!published_dir.exists());
        assert!(staging.exists());
        assert!(runtime.exists());
        assert!(stray.exists());
        fs::remove_dir_all(project_dir).unwrap();
    }

    #[test]
    fn prune_on_missing_cache_root_is_a_noop() {
        let project_dir = test_root();
        let report = prune_render_cache(&project_dir, 0, &HashSet::new()).unwrap();
        assert_eq!(report, RenderCachePruneReport::default());
    }
}
