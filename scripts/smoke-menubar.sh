#!/usr/bin/env bash
# Smoke test: margin runs as a menubar-only (Accessory) app.
#
# Launches the app, waits for it to check in with Launch Services, and asserts
# lsappinfo reports type "UIElement" (Accessory policy: no Dock icon, not in
# Cmd-Tab). A Regular app reports "Foreground". Then kills the app.
#
# Usage: scripts/smoke-menubar.sh [path-to-margin-binary]
# Default: the release bundle from `bun run tauri build`.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
bin="${1:-$root/target/release/bundle/macos/margin.app/Contents/MacOS/margin}"

if [[ ! -x "$bin" ]]; then
  echo "smoke-menubar: no executable at $bin (run \`bun run tauri build\` or pass a path)" >&2
  exit 2
fi

"$bin" >/dev/null 2>&1 &
pid=$!
trap 'kill "$pid" 2>/dev/null || true' EXIT

# The app checks in as "Foreground" and switches to "UIElement" once setup sets
# the policy, so poll until it is UIElement (or give up after ~10s).
type=""
for _ in $(seq 1 40); do
  type="$(lsappinfo list | grep -Eo "pid = $pid .* type=\"[^\"]*\"" | grep -Eo 'type="[^"]*"' | cut -d'"' -f2 || true)"
  [[ "$type" == "UIElement" ]] && break
  sleep 0.25
done

if [[ "$type" == "UIElement" ]]; then
  echo "smoke-menubar: ok (pid $pid is a UIElement)"
else
  echo "smoke-menubar: FAIL (expected type UIElement, got '${type:-<not registered>}')" >&2
  exit 1
fi
