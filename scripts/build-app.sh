#!/bin/sh
# Builds and signs target/app/<APP_NAME>.app, then prints its path.
#
#   APP_TEAM_ID=ABCDE12345 ./scripts/build-app.sh   # Developer ID signed
#   APP_ADHOC_SIGN=1 ./scripts/build-app.sh         # local-only, no Apple account
#
# APP_CARGO_FEATURES=runtime-shaders builds without full Xcode installed.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$root/scripts/app-config.sh"
. "$root/scripts/macos-signing.sh"
identity=$(app_codesign_identity)
version=$(app_version "$root")
build_number=$(app_build_number "$version")

bundle="$root/target/app/$APP_NAME.app"
contents="$bundle/Contents"
frameworks="$contents/Frameworks"
plist="$contents/Info.plist"

export MACOSX_DEPLOYMENT_TARGET="$MINIMUM_MACOS"
cargo build --locked --release --manifest-path "$root/Cargo.toml" \
  ${APP_CARGO_FEATURES:+--features "$APP_CARGO_FEATURES"} >&2

rm -rf "$bundle"
mkdir -p "$contents/MacOS" "$contents/Resources"
cp "$root/target/release/$EXECUTABLE" "$contents/MacOS/$EXECUTABLE"
cp "$root/app/Info.plist" "$plist"
set_plist() {
  /usr/libexec/PlistBuddy -c "Set :$1 $2" "$plist" 2>/dev/null \
    || /usr/libexec/PlistBuddy -c "Add :$1 ${3:-string} $2" "$plist"
}
set_plist CFBundleName "$APP_NAME"
set_plist CFBundleDisplayName "$APP_NAME"
set_plist CFBundleIdentifier "$BUNDLE_ID"
set_plist CFBundleExecutable "$EXECUTABLE"
set_plist CFBundleShortVersionString "$version"
set_plist CFBundleVersion "$build_number"
set_plist LSMinimumSystemVersion "$MINIMUM_MACOS"

if [ -f "$root/app/AppIcon.icns" ]; then
  cp "$root/app/AppIcon.icns" "$contents/Resources/AppIcon.icns"
  set_plist CFBundleIconFile AppIcon
fi

if app_updates_enabled; then
  sparkle_dir=$("$root/scripts/setup-sparkle.sh")
  mkdir -p "$frameworks"
  /usr/bin/ditto "$sparkle_dir/Sparkle.framework" "$frameworks/Sparkle.framework"
  set_plist SUFeedURL "$DOWNLOAD_BASE_URL/appcast.xml"
  set_plist SUPublicEDKey "$SPARKLE_PUBLIC_KEY"
  set_plist SUEnableAutomaticChecks true bool
  set_plist SUVerifyUpdateBeforeExtraction true bool
  # Nested code is signed inside-out, before the app itself.
  sparkle="$frameworks/Sparkle.framework/Versions/B"
  app_codesign "$identity" "$sparkle/XPCServices/Installer.xpc"
  app_codesign "$identity" --preserve-metadata=entitlements "$sparkle/XPCServices/Downloader.xpc"
  app_codesign "$identity" "$sparkle/Autoupdate"
  app_codesign "$identity" "$sparkle/Updater.app"
  app_codesign "$identity" "$frameworks/Sparkle.framework"
else
  echo "Sparkle updates are not configured; packaging without the updater." >&2
fi

app_codesign "$identity" --entitlements "$root/app/App.entitlements" "$bundle"
codesign --verify --deep --strict "$bundle" >&2
echo "$bundle"
