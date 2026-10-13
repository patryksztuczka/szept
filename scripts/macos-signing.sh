#!/bin/sh
# Shared helpers, sourced by the packaging scripts.

# Prints the codesign identity. APP_ADHOC_SIGN=1 selects ad-hoc signing ("-"),
# which runs on this Mac only and cannot be notarized or distributed.
app_codesign_identity() {
  if [ "${APP_ADHOC_SIGN:-0}" = 1 ]; then
    printf '%s\n' -
    return 0
  fi
  team_id=${APP_TEAM_ID:-}
  if [ -z "$team_id" ]; then
    echo "Set APP_TEAM_ID to your Apple Developer team ID, or APP_ADHOC_SIGN=1 for a local-only build." >&2
    return 1
  fi
  identity=${APP_CODESIGN_IDENTITY:-}
  if [ -z "$identity" ]; then
    identity=$(security find-identity -v -p codesigning | sed -n "s/.*\"\(Developer ID Application:.*($team_id)\)\"/\1/p" | head -1)
  fi
  if [ -z "$identity" ]; then
    echo "No Developer ID Application identity found for team $team_id. Set APP_CODESIGN_IDENTITY." >&2
    return 1
  fi
  case "$identity" in
    "Developer ID Application:"*"($team_id)") ;;
    *)
      echo "Signing identity is not a Developer ID Application identity for team $team_id." >&2
      return 1
      ;;
  esac
  printf '%s\n' "$identity"
}

# Signs one path. Ad-hoc signatures cannot carry a secure timestamp, and skip
# the Hardened Runtime because its library validation rejects frameworks
# without a matching Team ID. Notarization requires both, so releases use
# Developer ID signing.
app_codesign() {
  identity=$1
  shift
  if [ "$identity" = - ]; then
    codesign --force --sign - "$@"
  else
    codesign --force --timestamp --options runtime --sign "$identity" "$@"
  fi
}

app_version() {
  printf '%s\n' "${APP_VERSION:-$(sed -n 's/^version = "\(.*\)"$/\1/p' "$1/Cargo.toml" | head -1)}"
}

# 1.2.3 -> 10203. Sparkle compares CFBundleVersion, so it must always increase.
app_build_number() {
  printf '%s\n' "${APP_BUILD_NUMBER:-$(printf '%s\n' "$1" | awk -F. '{ print ($1 * 10000) + ($2 * 100) + $3 }')}"
}

app_updates_enabled() {
  [ -n "$DOWNLOAD_BASE_URL" ] && [ -n "$SPARKLE_PUBLIC_KEY" ]
}
