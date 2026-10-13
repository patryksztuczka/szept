# App identity and release settings, sourced by every packaging script.
# APP_NAME and BUNDLE_ID must match the constants in src/main.rs, and
# EXECUTABLE must match the [[bin]] name in Cargo.toml (`cargo test` checks).
APP_NAME=Szept
BUNDLE_ID=com.patryksztuczka.szept
EXECUTABLE=szept
MINIMUM_MACOS=15.0

# Optional Sparkle updates. Leave either empty to package without the updater.
# DOWNLOAD_BASE_URL hosts appcast.xml and releases/, e.g. https://downloads.example.com
DOWNLOAD_BASE_URL=${APP_DOWNLOAD_BASE_URL:-}
# Public key printed by Sparkle's bin/generate_keys (see README → Updates).
SPARKLE_PUBLIC_KEY=${APP_SPARKLE_PUBLIC_KEY:-}

# Cloudflare R2 bucket that `release-app.sh publish` uploads to with wrangler.
RELEASE_BUCKET=${APP_RELEASE_BUCKET:-}
