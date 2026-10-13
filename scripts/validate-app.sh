#!/bin/sh
# Checks a packaged bundle's identity, version, architecture, and signature.
set -eu

if [ "$#" -ne 3 ]; then
  echo "Usage: $0 <path to .app> <version> <build number>" >&2
  exit 1
fi
bundle=$1
version=$2
build_number=$3
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$root/scripts/app-config.sh"
. "$root/scripts/macos-signing.sh"
plist="$bundle/Contents/Info.plist"

fail() {
  echo "Bundle validation failed: $*" >&2
  exit 1
}
plist_value() {
  /usr/libexec/PlistBuddy -c "Print :$1" "$plist" 2>/dev/null || true
}

[ -d "$bundle" ] || fail "$bundle does not exist"
[ "$(plist_value CFBundleIdentifier)" = "$BUNDLE_ID" ] || fail "wrong bundle identifier"
[ "$(plist_value CFBundleExecutable)" = "$EXECUTABLE" ] || fail "wrong executable name"
[ "$(plist_value CFBundleName)" = "$APP_NAME" ] || fail "wrong bundle name"
[ "$(plist_value CFBundleShortVersionString)" = "$version" ] || fail "wrong version"
[ "$(plist_value CFBundleVersion)" = "$build_number" ] || fail "wrong build number"
[ "$(plist_value LSMinimumSystemVersion)" = "$MINIMUM_MACOS" ] || fail "wrong minimum macOS"
[ -x "$bundle/Contents/MacOS/$EXECUTABLE" ] || fail "packaged executable is missing"
[ "$(lipo -archs "$bundle/Contents/MacOS/$EXECUTABLE")" = arm64 ] || fail "executable is not arm64-only"
if app_updates_enabled; then
  [ "$(plist_value SUFeedURL)" = "$DOWNLOAD_BASE_URL/appcast.xml" ] || fail "wrong Sparkle feed"
  [ "$(plist_value SUPublicEDKey)" = "$SPARKLE_PUBLIC_KEY" ] || fail "wrong Sparkle public key"
  [ -d "$bundle/Contents/Frameworks/Sparkle.framework" ] || fail "Sparkle.framework is missing"
fi

codesign --verify --deep --strict "$bundle" || fail "code signature is invalid"
signature=$(codesign -d --verbose=4 "$bundle" 2>&1)
printf '%s\n' "$signature" | grep -Fq "Identifier=$BUNDLE_ID" || fail "signature has the wrong identifier"
if [ "${APP_ADHOC_SIGN:-0}" != 1 ]; then
  team_id=${APP_TEAM_ID:?Set APP_TEAM_ID}
  printf '%s\n' "$signature" | grep -Fq "TeamIdentifier=$team_id" || fail "signature has the wrong team"
  printf '%s\n' "$signature" | grep -Fq 'Authority=Developer ID Application:' || fail "not signed for Developer ID distribution"
  printf '%s\n' "$signature" | grep -Fq 'runtime' || fail "hardened runtime is not enabled"
fi

echo "$bundle"
