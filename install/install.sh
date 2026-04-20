#!/usr/bin/env bash
# install.sh — installs vex + vex-bridge on macOS, then opens the Architur
# setup screen in your browser so you can finish pairing in one go.
#
# Usage (one-liner):
#   curl -fsSL https://studio.planmorph.software/api/install/script.sh | bash
#
# Or directly from GitHub:
#   curl -fsSL https://github.com/Planmorph-Org/vex-bridge/releases/latest/download/install.sh | bash
#
# Steps performed automatically:
#   1. Detects Mac architecture (Apple Silicon or Intel).
#   2. Downloads vex and vex-bridge from GitHub Releases.
#   3. Installs both binaries to /usr/local/bin (or ~/.local/bin as fallback).
#   4. Registers vex-bridge as a launchd agent — starts automatically at every login.
#   5. Starts the daemon immediately (no reboot needed).
#   6. Opens your browser to the Architur setup/pairing page.
#      Approve the device there and you are done.
#
# Requirements: macOS 12+, curl, tar.
set -euo pipefail

REPO_VEX="${VEX_GITHUB_REPO:-Planmorph-Org/vex}"
REPO_BRIDGE="${VEX_BRIDGE_GITHUB_REPO:-Planmorph-Org/vex-bridge}"
DAEMON_LABEL="com.architur.vex-bridge"
PLIST_PATH="$HOME/Library/LaunchAgents/${DAEMON_LABEL}.plist"

# ── Colour helpers ────────────────────────────────────────────────────────────
red()   { printf '\033[0;31m%s\033[0m\n' "$*"; }
green() { printf '\033[0;32m%s\033[0m\n' "$*"; }
bold()  { printf '\033[1m%s\033[0m\n' "$*"; }

step() { bold "==> $*"; }
ok()   { green "    ✓ $*"; }
fail() { red   "    ✗ $*"; exit 1; }

# ── Platform check ────────────────────────────────────────────────────────────
[[ "$(uname)" == "Darwin" ]] || fail "This script is for macOS. Use install.ps1 on Windows."

ARCH="$(uname -m)"
case "$ARCH" in
  arm64)  SUFFIX="macos-arm64"  ;;
  x86_64) SUFFIX="macos-x86_64" ;;
  *)      fail "Unsupported architecture: $ARCH" ;;
esac

# ── Install directory ─────────────────────────────────────────────────────────
if [[ -w /usr/local/bin ]]; then
  INSTALL_DIR="/usr/local/bin"
else
  INSTALL_DIR="$HOME/.local/bin"
  mkdir -p "$INSTALL_DIR"
  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) printf '\nNote: add this to your shell profile: export PATH="$HOME/.local/bin:$PATH"\n\n' ;;
  esac
fi

# ── Fetch latest release tags ─────────────────────────────────────────────────
step "Fetching latest release versions..."

latest_tag() {
  curl -fsSL "https://api.github.com/repos/${1}/releases/latest" \
    | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/'
}

VEX_TAG="$(latest_tag "$REPO_VEX")"
BRIDGE_TAG="$(latest_tag "$REPO_BRIDGE")"

[[ -n "$VEX_TAG" ]]    || fail "Could not determine latest vex version."
[[ -n "$BRIDGE_TAG" ]] || fail "Could not determine latest vex-bridge version."

ok "vex        ${VEX_TAG}"
ok "vex-bridge ${BRIDGE_TAG}"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# ── Download + install vex ────────────────────────────────────────────────────
step "Downloading vex ${VEX_TAG}..."
VEX_ARCHIVE="vex-${VEX_TAG}-${SUFFIX}.tar.gz"
curl -fsSL "https://github.com/${REPO_VEX}/releases/download/${VEX_TAG}/${VEX_ARCHIVE}" \
  -o "$TMP/$VEX_ARCHIVE"
tar -xzf "$TMP/$VEX_ARCHIVE" -C "$TMP"
install -m 755 "$TMP/vex-${VEX_TAG}-${SUFFIX}/vex" "$INSTALL_DIR/vex"
ok "Installed vex → ${INSTALL_DIR}/vex"

# ── Download + install vex-bridge ─────────────────────────────────────────────
step "Downloading vex-bridge ${BRIDGE_TAG}..."
BRIDGE_ARCHIVE="vex-bridge-${BRIDGE_TAG}-${SUFFIX}.tar.gz"
curl -fsSL "https://github.com/${REPO_BRIDGE}/releases/download/${BRIDGE_TAG}/${BRIDGE_ARCHIVE}" \
  -o "$TMP/$BRIDGE_ARCHIVE"
tar -xzf "$TMP/$BRIDGE_ARCHIVE" -C "$TMP"
install -m 755 "$TMP/vex-bridge-${BRIDGE_TAG}-${SUFFIX}/vex-bridge" "$INSTALL_DIR/vex-bridge"
ok "Installed vex-bridge → ${INSTALL_DIR}/vex-bridge"

# ── launchd agent (auto-start at every login) ────────────────────────────────
step "Installing launchd login agent..."
mkdir -p "$HOME/Library/LaunchAgents" "$HOME/Library/Logs"

cat > "$PLIST_PATH" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>             <string>${DAEMON_LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>${INSTALL_DIR}/vex-bridge</string>
    <string>start</string>
  </array>
  <key>KeepAlive</key>         <true/>
  <key>RunAtLoad</key>         <true/>
  <key>StandardOutPath</key>
  <string>$HOME/Library/Logs/vex-bridge.log</string>
  <key>StandardErrorPath</key>
  <string>$HOME/Library/Logs/vex-bridge.log</string>
</dict>
</plist>
PLIST

ok "Registered login agent."

# ── Start daemon now (no reboot needed) ───────────────────────────────────────
step "Starting vex-bridge..."
launchctl unload "$PLIST_PATH" 2>/dev/null || true
launchctl load -w "$PLIST_PATH"

# Give the daemon a moment to bind its port before we talk to it.
sleep 2
ok "vex-bridge is running."

# ── Open setup screen in the browser ─────────────────────────────────────────
# vex-bridge pair --open-browser:
#   - registers an Ed25519 key with Architur
#   - opens the Architur approval page in the user's default browser
#   - polls quietly until the user clicks Approve
#   - on success: this machine is paired and ready to push
step "Opening Architur setup screen..."
DEVICE_LABEL="$(scutil --get ComputerName 2>/dev/null || hostname)"
"$INSTALL_DIR/vex-bridge" pair \
  --device-label "$DEVICE_LABEL" \
  --open-browser

# ── Done ──────────────────────────────────────────────────────────────────────
printf '\n'
bold "All done! This machine is paired with your Architur account."
printf '\n'
printf 'Your CAD plugins can now push models by hitting:\n'
printf '  http://127.0.0.1:7878/v1/repo/push\n\n'
printf 'Run  vex --help  to use the CLI directly.\n\n'
