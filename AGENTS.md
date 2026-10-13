# Agent Guide

Szept: a native macOS window that captions meetings live in English and
Polish. GPUI draws the UI; cpal captures audio; Soniox does speech-to-text and
translation over a websocket; `objc2` crates reach Apple frameworks; Sparkle
ships updates. See `README.md` for commands.

## Architecture

- `main`: clap CLI. `app` (default) opens the GUI; other commands are plain
  terminal tools sharing the same modules. Holds the `APP_NAME`/`BUNDLE_ID`
  identity constants.
- `app_window`: the GPUI `Application`, key bindings, menus, global actions, and
  the single main window (transcript pane and Settings pane). App-level actions
  are registered with `cx.on_action`; window-level ones on the root element.
- `desktop_ui`: every colour, size, and shared control. Panes render through
  `pane` + `pane_body` and build from `panel`, `setting_row`, `button`, `toggle`;
  the transcript from `bubble`, `speaker_label`, `segment`, `footer`.
- `live`: one listening session. An audio thread owns the cpal stream (meeting
  audio via a Core Audio process tap, or the microphone) and converts it to
  16 kHz mono PCM; a Soniox thread streams it. Dropping `live::Session` stops
  capture, and the event channel ends with `Event::Finished`.
- `soniox`: the websocket client and its config. Translation to Polish is
  always on so switching views never restarts a session.
- `transcript`: pure token-to-utterance logic; `<end>` tokens split bubbles.
- `api_key`: `SONIOX_API_KEY`, else a 0600 file in the data folder.
- `window_level`: floats the window above other apps through its `NSWindow`.
- `settings`: the persisted `Settings` struct, with atomic writes and
  corrupt-file preservation.
- `app_paths`: data directories and tracing setup.
- `login_item`: `SMAppService` launch at login. macOS owns this state; never
  mirror it into settings.
- `sparkle`: Sparkle loaded at runtime from `Contents/Frameworks`; absent outside
  a packaged, update-enabled bundle.

## Rules

- Keep app logic in modules with no GPUI dependency, so it can be unit-tested.
  Views call into it.
- Never block the UI thread. Do slow or blocking work with
  `cx.spawn(async move |this, cx| { cx.background_executor().spawn(...).await; this.update(...) })`
  and store the returned `Task` on the view; dropping a task cancels it.
- Style only through `desktop_ui` tokens and helpers. Add a helper there rather
  than hand-styling one pane.
- New `Settings` fields need defaults (`#[serde(default)]` is on the struct) so
  older files keep loading.
- `objc2` calls go in small modules with `// SAFETY:` comments on each `unsafe`
  block, wrapped in `autoreleasepool` when called off the main thread.
- The app identity lives in `scripts/app-config.sh` and `src/main.rs`; a unit test
  keeps them in sync.
- Do not add abstractions for other platforms or plugins until a second real
  implementation needs them.

## Validate

```sh
./scripts/check.sh            # add --features runtime-shaders without full Xcode
```

For UI changes, also run the app (`cargo run`) and look at the affected panes.
For packaging changes, run `APP_ADHOC_SIGN=1 ./scripts/build-app.sh` and
`APP_ADHOC_SIGN=1 ./scripts/validate-app.sh target/app/<APP_NAME>.app <version> <build>`.

## Releases

`scripts/release-app.sh prepare` then `publish`; see README. Never reuse a
version for changed code: Sparkle compares `CFBundleVersion`, derived from the
`Cargo.toml` version (1.2.3 → 10203). Never overwrite a published artifact with
different bytes.
