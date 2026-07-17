//! Safe publication of derived render artifacts.
//!
//! A renderer writes a manifest and content-addressed objects into a staging
//! directory. This module validates every declared object and atomically moves
//! the directory into the immutable commit cache. The renderer is deliberately
//! outside this module: a worker may use web-ifc, a native engine, or a future
//! service without gaining authority over Vex commits.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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
    for resource in manifest
        .tiles
        .iter_mut()
        .map(|tile| &mut tile.artifact)
        .chain(std::iter::once(&mut manifest.semantic_index.artifact))
    {
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
    let resources = manifest
        .tiles
        .iter()
        .map(|tile| &tile.artifact)
        .chain(std::iter::once(&manifest.semantic_index.artifact));
    let mut verified = HashSet::new();
    for resource in resources {
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
        proto::RenderArtifactManifest {
            schema: proto::schema::RENDER_MANIFEST.into(),
            project_id: project_id.into(),
            commit_hash: commit_hash.into(),
            artifact_id: "test-artifact".into(),
            generated_at: "2026-07-17T00:00:00Z".into(),
            tiles: vec![proto::RenderTileDescriptor {
                tile_id: "storey-0".into(),
                lod: 0,
                bounds: proto::RenderBounds {
                    min: [0.0, 0.0, 0.0],
                    max: [1.0, 1.0, 1.0],
                },
                geometric_error: 0.0,
                artifact: resource(bytes),
            }],
            semantic_index: proto::RenderSemanticIndexDescriptor {
                schema: proto::schema::RENDER_SEMANTIC_INDEX.into(),
                entry_count: 1,
                artifact: resource(bytes),
            },
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
}
