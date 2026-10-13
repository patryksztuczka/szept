#!/bin/sh
# Builds the app, replaces the copy in /Applications, and opens it.
# Takes the same environment as build-app.sh.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$root/scripts/app-config.sh"
source_bundle=$("$root/scripts/build-app.sh")
installed="/Applications/$APP_NAME.app"

if [ -e "$installed" ]; then
  existing_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$installed/Contents/Info.plist" 2>/dev/null || true)
  if [ "$existing_id" != "$BUNDLE_ID" ]; then
    echo "Refusing to replace $installed: its bundle ID is '$existing_id', not $BUNDLE_ID." >&2
    exit 1
  fi
  pkill -TERM -f "$installed/Contents/MacOS/$EXECUTABLE" 2>/dev/null || true
  attempts=30
  while pgrep -f "$installed/Contents/MacOS/$EXECUTABLE" >/dev/null; do
    [ "$attempts" -gt 0 ] || { echo "Timed out waiting for $APP_NAME to quit." >&2; exit 1; }
    attempts=$((attempts - 1))
    sleep 1
  done
  rm -rf "$installed"
fi
/usr/bin/ditto "$source_bundle" "$installed"
/usr/bin/open "$installed"
echo "$installed"
