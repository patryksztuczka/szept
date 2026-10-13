#!/bin/sh
# Two-step public release, modelled on HEX:
#
#   ./scripts/release-app.sh prepare   # build, notarize, staple, DMG, appcast in dist/
#   ./scripts/release-app.sh publish   # re-verify dist/, upload to R2, then move the appcast
#
# Requires APP_TEAM_ID, APP_NOTARY_PROFILE (an `xcrun notarytool store-credentials`
# profile), and the update settings in scripts/app-config.sh. Publishing also
# needs RELEASE_BUCKET and a logged-in `wrangler`.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$root/scripts/app-config.sh"
. "$root/scripts/macos-signing.sh"
mode=${1:-prepare}
version=$(app_version "$root")
build_number=$(app_build_number "$version")
notary_profile=${APP_NOTARY_PROFILE:?Set APP_NOTARY_PROFILE to a notarytool keychain profile}
release_notes=${APP_RELEASE_NOTES:-$root/docs/releases/$version.md}
dist="$root/dist"
updates="$dist/updates"
dmg="$APP_NAME-$version-arm64.dmg"
update_zip="$APP_NAME-$version-arm64.zip"
latest_dmg="$APP_NAME-latest-arm64.dmg"

[ "$mode" = prepare ] || [ "$mode" = publish ] || { echo "Usage: $0 [prepare|publish]" >&2; exit 1; }
[ "${APP_ADHOC_SIGN:-0}" != 1 ] || { echo "Releases cannot be ad-hoc signed." >&2; exit 1; }
app_updates_enabled || { echo "Set DOWNLOAD_BASE_URL and SPARKLE_PUBLIC_KEY in scripts/app-config.sh." >&2; exit 1; }
if [ -n "$(git -C "$root" status --porcelain)" ]; then
  echo "Commit or remove all working-tree changes before releasing." >&2
  exit 1
fi
upstream=$(git -C "$root" rev-parse --abbrev-ref '@{upstream}' 2>/dev/null || true)
if [ -z "$upstream" ] || [ "$(git -C "$root" rev-parse HEAD)" != "$(git -C "$root" rev-parse "$upstream")" ]; then
  echo "Push the release commit to its upstream branch before releasing." >&2
  exit 1
fi
# Sparkle only offers builds newer than the installed CFBundleVersion.
published_build=$(curl --fail --silent "$DOWNLOAD_BASE_URL/appcast.xml" 2>/dev/null \
  | sed -n 's|.*<sparkle:version>\([0-9][0-9]*\)</sparkle:version>.*|\1|p' | head -1 || true)
if [ -n "$published_build" ] && [ "$build_number" -le "$published_build" ]; then
  echo "Build $build_number must be newer than published build $published_build. Bump the version in Cargo.toml." >&2
  exit 1
fi

if [ "$mode" = publish ]; then
  bucket=${RELEASE_BUCKET:?Set RELEASE_BUCKET (or APP_RELEASE_BUCKET) to your R2 bucket}
  for file in "$dist/$dmg" "$dist/$update_zip" "$updates/appcast.xml"; do
    [ -f "$file" ] || { echo "Missing $file. Run '$0 prepare' first." >&2; exit 1; }
  done
  grep -Fq 'sparkle:edSignature=' "$updates/appcast.xml" || { echo "The appcast is not signed." >&2; exit 1; }
  xcrun stapler validate "$dist/$dmg"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$dist/$dmg"

  put() {
    wrangler r2 object put "$bucket/$1" --remote --file "$2" --content-type "$3" --cache-control "$4"
  }
  put "releases/$dmg" "$dist/$dmg" application/x-apple-diskimage 'public, max-age=31536000, immutable'
  put "releases/$update_zip" "$dist/$update_zip" application/zip 'public, max-age=31536000, immutable'
  # Confirm the public URLs serve the exact bytes before pointing users at them.
  verification=$(mktemp -d "${TMPDIR:-/tmp}/release-verify.XXXXXX")
  trap 'rm -rf "$verification"' EXIT HUP INT TERM
  for file in "$dmg" "$update_zip"; do
    curl --fail --silent --show-error "$DOWNLOAD_BASE_URL/releases/$file" -o "$verification/$file"
    cmp -s "$dist/$file" "$verification/$file" || { echo "$file on $DOWNLOAD_BASE_URL does not match dist/." >&2; exit 1; }
  done
  put "releases/$latest_dmg" "$dist/$dmg" application/x-apple-diskimage no-cache
  # The appcast goes last: once it changes, installed apps start updating.
  put appcast.xml "$updates/appcast.xml" application/xml no-cache
  echo "$DOWNLOAD_BASE_URL/releases/$dmg"
  exit 0
fi

[ -f "$release_notes" ] || { echo "Add release notes at $release_notes or set APP_RELEASE_NOTES." >&2; exit 1; }
export APP_VERSION="$version" APP_BUILD_NUMBER="$build_number"
bundle=$("$root/scripts/build-app.sh")
"$root/scripts/validate-app.sh" "$bundle" "$version" "$build_number" >/dev/null
identity=$(app_codesign_identity)
bundle_name=$(basename "$bundle")
rm -rf "$dist/staging" "$updates"
rm -f "$dist/$dmg" "$dist/$update_zip" "$dist/notarize.zip"
mkdir -p "$dist/staging" "$updates"

# Notarize the app first so its ticket can be stapled inside the DMG.
/usr/bin/ditto -c -k --sequesterRsrc --keepParent "$bundle" "$dist/notarize.zip"
xcrun notarytool submit "$dist/notarize.zip" --keychain-profile "$notary_profile" --wait
rm -f "$dist/notarize.zip"
xcrun stapler staple "$bundle"
xcrun stapler validate "$bundle"

/usr/bin/ditto "$bundle" "$dist/staging/$bundle_name"
ln -s /Applications "$dist/staging/Applications"
hdiutil create -quiet -fs APFS -format ULFO -volname "$APP_NAME" -srcfolder "$dist/staging" "$dist/$dmg"
codesign --force --timestamp --sign "$identity" "$dist/$dmg"
xcrun notarytool submit "$dist/$dmg" --keychain-profile "$notary_profile" --wait
xcrun stapler staple "$dist/$dmg"
spctl --assess --type open --context context:primary-signature --verbose=2 "$dist/$dmg"

# Sparkle installs updates from a ZIP of the same stapled app.
/usr/bin/ditto -c -k --sequesterRsrc --keepParent "$bundle" "$dist/$update_zip"
cp "$dist/$update_zip" "$updates/"
cp "$release_notes" "$updates/$APP_NAME-$version-arm64.md"
curl --fail --silent "$DOWNLOAD_BASE_URL/appcast.xml" -o "$updates/appcast.xml" 2>/dev/null || rm -f "$updates/appcast.xml"
sparkle_dir=$("$root/scripts/setup-sparkle.sh")
# generate_appcast signs with the private key generate_keys stored in your keychain.
"$sparkle_dir/bin/generate_appcast" \
  --download-url-prefix "$DOWNLOAD_BASE_URL/releases/" \
  --embed-release-notes \
  --maximum-deltas 0 \
  "$updates"
grep -Fq 'sparkle:edSignature=' "$updates/appcast.xml"

echo "$dist/$dmg"
echo "Prepared but not published. Test the DMG, then run: $0 publish" >&2
