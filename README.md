# Szept

A small macOS window that sits beside your meeting app and shows what people
are saying, live, in English and Polish. Built on
[rust-desktop-starter](https://github.com/patryksztuczka/rust-desktop-starter):
Rust, [GPUI](https://www.gpui.rs/), and `objc2`, with no webview.

- **Meeting** captures everything the Mac plays, so it hears the other people
  in Teams, Zoom, Meet, or any other call app. **Mic** uses the microphone,
  e.g. for a meeting in the room.
- Speech streams to [Soniox](https://soniox.com) real-time speech-to-text,
  which returns the English transcript and its Polish translation together.
- Each pause starts a new chat bubble, coloured by speaker. Words still being
  recognised are drawn dimmer and settle within a second or so.
- Switch between **PL**, **PL + EN**, and **EN** at any time without restarting.
- The window floats above other apps, including full-screen calls (pin
  button, or Settings → Keep on top).

## Install

There is no download yet: you build the app on your own Mac. It takes about
ten minutes, mostly the first compile.

### Requirements

- A Mac with **macOS 15 (Sequoia) or newer**. Tested on Apple silicon.
- **Xcode Command Line Tools**: `xcode-select --install` (skip it if you
  already have them or full Xcode). Full Xcode is not needed.
- **Rust**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`,
  then open a new terminal window.
- About **8 GB of free disk** for the build folder. You can delete `target/`
  afterwards.
- A **Soniox API key** from [console.soniox.com](https://console.soniox.com).
  Soniox bills per hour of streamed audio, and only while Szept is listening.

### Build and install

```sh
git clone https://github.com/patryksztuczka/szept.git
cd szept
APP_ADHOC_SIGN=1 APP_CARGO_FEATURES=runtime-shaders ./scripts/install-app.sh
```

This builds `Szept.app`, copies it to `/Applications`, and opens it. After that,
start it like any other app.

`APP_ADHOC_SIGN=1` signs the app for your Mac only, so no Apple developer
account is needed. `APP_CARGO_FEATURES=runtime-shaders` lets it build without
full Xcode.

### First run

1. Copy your Soniox key, open **Settings** (⌘,), and click **Paste from
   Clipboard**.
2. Close Settings and click **Start** (⌘L).
3. macOS asks to allow **System Audio Recording**. Allow it. If there was no
   prompt, or the footer says "No sound yet" while people are talking, open
   System Settings → Privacy & Security → Screen & System Audio Recording, turn
   on Szept under **System Audio Recording Only**, then quit and reopen Szept.
4. Pick **PL**, **PL + EN**, or **EN** at the top. You can switch at any time.

### Update

```sh
cd szept
git pull
APP_ADHOC_SIGN=1 APP_CARGO_FEATURES=runtime-shaders ./scripts/install-app.sh
```

Each rebuild gets a new ad-hoc signature, so macOS may ask for the audio
permission again.

### Headphones

**Meeting** mode records what the Mac plays on its current output device. If
that device also has a microphone, the app currently records the microphone
instead. This affects headsets with a mic, very likely including AirPods and
other Bluetooth headsets: you would see your own words, not the other people's.
Laptop speakers, wired headphones without a mic, and external speakers work.
**Mic** mode is for meetings in a room.

## Soniox API key

The app reads the key from, in order:

1. The `SONIOX_API_KEY` environment variable, handy with `cargo run`.
2. `~/Library/Application Support/com.patryksztuczka.szept/soniox-api-key`,
   readable only by you. Copy the key, then open Settings (⌘,) and click
   **Paste from Clipboard**; the app writes this file. **Remove** deletes it.

## Permissions

Meeting audio uses a Core Audio process tap. macOS asks once for **System
Audio Recording** (System Settings → Privacy & Security → Screen & System Audio
Recording). Mic mode asks for the microphone. If the footer says "No sound
yet", check that permission. Under `cargo run` the permission belongs to your
terminal app; the installed app asks for itself.

Known limit: if the default output device is also an input (headsets with a
mic), cpal opens that device's microphone instead of tapping its output. See
[Headphones](#headphones).

## Shortcuts

| Keys | Action |
|---|---|
| ⌘L | Start or stop listening |
| ⌘1 / ⌘2 / ⌘3 | Polish / Polish + English / English |
| ⌘⇧C | Copy the transcript |
| ⌘K | Clear the transcript |
| ⌘, | Settings |

## Development

Same requirements as [Install](#requirements). Full Xcode, with its Metal
toolchain, compiles GPUI's shaders at build time; without it, add
`--features runtime-shaders` to every cargo command, and the shaders are
compiled when the app starts.

### Run it

```sh
cargo run                    # opens the app (same as `cargo run -- app`)
cargo run -- app --pane settings   # open straight to a pane
cargo run -- meter           # print meeting-audio levels for 10 s (no API key needed)
cargo run -- meter --source microphone
cargo run -- paths           # where data, settings, the API key, and logs live
cargo run -- settings        # print saved settings
RUST_LOG=szept=debug cargo run

./scripts/check.sh           # fmt, clippy -D warnings, tests
```

Without Xcode: `cargo run --features runtime-shaders` and
`./scripts/check.sh --features runtime-shaders`.

Logs go to stderr and to `~/Library/Application Support/<bundle id>/logs/process.log`.
Set `SZEPT_SUPPORT_DIR=/tmp/somewhere` to run against a throwaway data folder.

## Layout

| Path | Purpose |
|---|---|
| `src/main.rs` | CLI entry point and app identity constants |
| `src/app_window.rs` | GPUI application, menus, actions, the transcript window and Settings |
| `src/desktop_ui.rs` | Colours, sizes, and shared controls (bubbles, segments, buttons, rows) |
| `src/live.rs` | Audio capture with cpal, 16 kHz conversion, session threads |
| `src/soniox.rs` | Soniox websocket client |
| `src/transcript.rs` | Turns Soniox tokens into speaker utterances |
| `src/api_key.rs` | Loads and saves the Soniox API key |
| `src/window_level.rs` | Keeps the window above other apps |
| `src/settings.rs` | Persisted settings with atomic saves and corrupt-file backup |
| `src/app_paths.rs` | Data directories and logging |
| `src/login_item.rs` | Launch at login (`SMAppService`) |
| `src/sparkle.rs` | Sparkle updater, loaded at runtime from the bundle |
| `app/` | `Info.plist` template and entitlements |
| `scripts/` | Build, install, validate, and release scripts |
| `docs/releases/` | Release notes, one Markdown file per version |

## Build the app bundle

Signing has three levels:

| Level | Needs | Good for |
|---|---|---|
| None (`cargo run`) | Nothing | Day-to-day development |
| Ad-hoc | Nothing | Trying the real `.app` on your own Mac |
| Developer ID + notarization | Paid Apple Developer Program | Distributing to other people |

```sh
# Local-only bundle, no Apple account:
APP_ADHOC_SIGN=1 ./scripts/build-app.sh       # → target/app/Szept.app
APP_ADHOC_SIGN=1 ./scripts/install-app.sh     # build, copy to /Applications, open

# Developer ID signed:
APP_TEAM_ID=ABCDE12345 ./scripts/install-app.sh
```

Add `APP_CARGO_FEATURES=runtime-shaders` to either if Xcode is not installed.
Ad-hoc builds skip the Hardened Runtime: its library validation would refuse to
load the bundled Sparkle framework, which has no Team ID to match.

Some features only work from a properly signed app in `/Applications`:
launch at login, and stable permission grants (macOS ties microphone,
accessibility, and similar permissions to the signature, so rebuilding with a
different identity asks again).

## Updates (optional)

Updates use [Sparkle](https://sparkle-project.org/). They stay off until you
configure them, and the bundle is built without Sparkle until then.

1. Get Sparkle's tools: `./scripts/setup-sparkle.sh` prints its folder.
2. Create a signing key once: `<that folder>/bin/generate_keys`. It stores the
   private key in your login keychain and prints the public key. Back up the
   private key (`generate_keys -x private-key-file`); losing it means installed
   apps can no longer accept your updates.
3. In `scripts/app-config.sh`, set `DOWNLOAD_BASE_URL` (where `appcast.xml` and
   `releases/` will be served) and `SPARKLE_PUBLIC_KEY`.

## Release

The release flow is two steps: prepare, test, then publish.

One-time setup:

- A **Developer ID Application** certificate in your keychain (Xcode →
  Settings → Accounts → Manage Certificates, or developer.apple.com).
- A notarization profile:
  `xcrun notarytool store-credentials my-notary --apple-id you@example.com --team-id ABCDE12345`
  (use an app-specific password from appleid.apple.com).
- Sparkle configured as above, and somewhere to host files. The publish step
  uploads to a Cloudflare R2 bucket with `wrangler`; set `RELEASE_BUCKET` and
  point `DOWNLOAD_BASE_URL` at the bucket's public domain. Swap the `put`
  function in `scripts/release-app.sh` for another host.

Each release:

```sh
# 1. Bump `version` in Cargo.toml and add docs/releases/<version>.md. Commit and push.
export APP_TEAM_ID=ABCDE12345 APP_NOTARY_PROFILE=my-notary

# 2. Build, sign, notarize, staple, make the DMG and the signed appcast in dist/.
./scripts/release-app.sh prepare

# 3. Open dist/Szept-<version>-arm64.dmg and test it. Then:
./scripts/release-app.sh publish
```

`prepare` refuses to run with uncommitted or unpushed changes, or when the build
number is not newer than the published one. `publish` uploads the DMG and ZIP,
checks that the public URLs serve identical bytes, and only then replaces the
appcast, so installed apps never see a half-published release.
