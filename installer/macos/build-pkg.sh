#!/bin/bash
# ─────────────────────────────────────────────────────────────────────────
# Vex Atlas — macOS installer (.pkg) builder
#
# Invoked by the vex-bridge release workflow on macos-latest. Produces a
# single VexAtlas-{version}-{arch}.pkg that:
#
#   1. Installs `vex` and `vex-bridge` to /usr/local/bin (writable for
#      admin users on macOS; pkgbuild + installer prompt for the password
#      once — same as installing any other commercial macOS app).
#   2. Drops a launchd LaunchAgent into /Library/LaunchAgents so the
#      daemon auto-starts at every login for every user on the machine.
#   3. Runs the pair flow: `vex-bridge pair --open-browser` opens the
#      user's default browser at studio.planmorph.software/pair?code=…
#      so they can approve this device with a single click.
#
# The installer is unsigned for now. macOS will block first-launch with
# Gatekeeper — user has to right-click → Open the .pkg once. We can
# Developer ID-sign + notarize later by setting CODESIGN_IDENTITY +
# NOTARY_PROFILE in the workflow.
#
# Inputs (env):
#   VERSION   — e.g. v0.1.0
#   ARCH      — arm64 | x86_64
#   BIN_DIR   — directory containing vex and vex-bridge binaries
#   OUT_DIR   — directory to write the .pkg into
# ─────────────────────────────────────────────────────────────────────────

set -euo pipefail

: "${VERSION:?VERSION required}"
: "${ARCH:?ARCH required}"
: "${BIN_DIR:?BIN_DIR required}"
: "${OUT_DIR:?OUT_DIR required}"

mkdir -p "$OUT_DIR"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# ── Payload ────────────────────────────────────────────────────────────
# pkgbuild expects a directory tree mirroring the install destination.
# We use --root with /usr/local/bin so the binaries land there.
PAYLOAD="$WORK/payload/usr/local/bin"
mkdir -p "$PAYLOAD"
install -m 755 "$BIN_DIR/vex"        "$PAYLOAD/vex"
install -m 755 "$BIN_DIR/vex-bridge" "$PAYLOAD/vex-bridge"

# ── Postinstall script ─────────────────────────────────────────────────
# Writes a system-wide LaunchAgent (/Library/LaunchAgents — runs in the
# user session at every login) and opens the pair URL in the user's
# default browser.
SCRIPTS="$WORK/scripts"
mkdir -p "$SCRIPTS"
cat > "$SCRIPTS/postinstall" <<'POSTINSTALL'
#!/bin/bash
# Postinstall — runs as root after pkgbuild copies the binaries.
# Goal: leave the machine in a state where the user just sees the
# Vex Atlas pairing page in their browser. No Terminal, ever.
set -e

PLIST="/Library/LaunchAgents/com.architur.vex-bridge.plist"
LOG_DIR="/Library/Logs/VexAtlas"
mkdir -p "$LOG_DIR"
chmod 755 "$LOG_DIR"

cat > "$PLIST" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>             <string>com.architur.vex-bridge</string>
  <key>ProgramArguments</key>
  <array>
    <string>/usr/local/bin/vex-bridge</string>
    <string>start</string>
  </array>
  <key>RunAtLoad</key>         <true/>
  <key>KeepAlive</key>         <true/>
  <key>StandardOutPath</key>   <string>/Library/Logs/VexAtlas/vex-bridge.log</string>
  <key>StandardErrorPath</key> <string>/Library/Logs/VexAtlas/vex-bridge.log</string>
</dict>
</plist>
PLIST

chown root:wheel "$PLIST"
chmod 644 "$PLIST"

# Bootstrap the agent into the GUI user's session immediately so the
# daemon is bound to 127.0.0.1:7878 by the time we open the pair URL.
GUI_USER="$(stat -f '%Su' /dev/console)"
GUI_UID="$(id -u "$GUI_USER")"
launchctl bootout "gui/${GUI_UID}" "$PLIST" 2>/dev/null || true
launchctl bootstrap "gui/${GUI_UID}" "$PLIST" || true

# Open the pair flow in the user's browser. `pair --open-browser` polls
# silently and exits when the user clicks Approve in the web UI.
DEVICE_LABEL="$(scutil --get ComputerName 2>/dev/null || hostname)"
sudo -u "$GUI_USER" /usr/local/bin/vex-bridge pair \
  --device-label "$DEVICE_LABEL" --open-browser >/dev/null 2>&1 &

exit 0
POSTINSTALL
chmod 755 "$SCRIPTS/postinstall"

# ── Component package ──────────────────────────────────────────────────
# A single component pkg is enough — we don't need a wrapper distribution
# unless we want a multi-page installer UI. installer.app already shows
# the standard wizard for component pkgs.
COMPONENT_PKG="$WORK/vex-atlas-component.pkg"
pkgbuild \
  --root "$WORK/payload" \
  --identifier "software.planmorph.vexatlas" \
  --version "${VERSION#v}" \
  --install-location "/" \
  --scripts "$SCRIPTS" \
  --ownership recommended \
  "$COMPONENT_PKG"

# ── Distribution wrapper ───────────────────────────────────────────────
# Adds a title, branded HTML welcome / conclusion screens, and arch
# restriction so installer.app refuses to run on the wrong CPU.
#
# Resources (welcome.html, conclusion.html) live next to this script.
# productbuild's --resources flag points at a directory and the
# distribution.xml references files by basename.
DIST="$WORK/distribution.xml"
RESOURCES_SRC="$(cd "$(dirname "$0")" && pwd)/resources"
case "$ARCH" in
  arm64)  HOST_ARCH="arm64" ;;
  x86_64) HOST_ARCH="x86_64" ;;
  *) echo "Unknown ARCH: $ARCH" >&2; exit 1 ;;
esac

cat > "$DIST" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="1">
  <title>Vex Atlas</title>
  <organization>software.planmorph</organization>
  <welcome    file="welcome.html"    mime-type="text/html"/>
  <conclusion file="conclusion.html" mime-type="text/html"/>
  <options customize="never" require-scripts="false" hostArchitectures="${HOST_ARCH}"/>
  <choices-outline>
    <line choice="default">
      <line choice="software.planmorph.vexatlas"/>
    </line>
  </choices-outline>
  <choice id="default"/>
  <choice id="software.planmorph.vexatlas" visible="false">
    <pkg-ref id="software.planmorph.vexatlas"/>
  </choice>
  <pkg-ref id="software.planmorph.vexatlas" version="${VERSION#v}"
           onConclusion="none">vex-atlas-component.pkg</pkg-ref>
</installer-gui-script>
EOF

OUT="$OUT_DIR/VexAtlas-${VERSION}-${ARCH}.pkg"
productbuild \
  --distribution "$DIST" \
  --package-path "$WORK" \
  --resources "$RESOURCES_SRC" \
  "$OUT"

echo "Wrote $OUT"
