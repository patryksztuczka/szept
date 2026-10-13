use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Application, Bounds, ClipboardItem, Context, FocusHandle, HighlightStyle,
    KeyBinding, Menu, MenuItem, ScrollHandle, SharedString, StyledText, SystemMenuType, Task,
    TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions, actions, div, prelude::*,
    px, rgb, size,
};

use crate::api_key::{self, KeySource};
use crate::desktop_ui::{self as ui, ButtonStyle};
use crate::live;
use crate::login_item::{self, LoginItemStatus};
use crate::settings::{AudioSource, Settings};
use crate::soniox::Event;
use crate::transcript::{Transcript, Utterance, View};
use crate::{APP_NAME, Pane, app_paths, sparkle, window_level};

actions!(
    szept,
    [
        CheckForUpdates,
        ClearTranscript,
        CloseWindow,
        CopyTranscript,
        HideApplication,
        MinimizeWindow,
        Quit,
        ShowBoth,
        ShowEnglish,
        ShowLive,
        ShowPolish,
        ShowSettings,
        ToggleFullscreen,
        ToggleListening,
    ]
);

const WINDOW_WIDTH: f32 = 420.0;
const WINDOW_HEIGHT: f32 = 680.0;
/// How long meeting audio may stay silent before the window suggests a cause.
const SILENCE_HINT_AFTER: Duration = Duration::from_secs(6);
/// Peaks below this count as silence.
const SILENCE_LEVEL: f32 = 0.002;
const SONIOX_CONSOLE_URL: &str = "https://console.soniox.com";

type MainWindow = Rc<RefCell<Option<WindowHandle<AppWindow>>>>;

/// Runs the GPUI application, opening the main window on `initial_pane`.
/// Returns when the app quits.
pub fn run(initial_pane: Pane) {
    let main_window = MainWindow::default();
    let application = Application::new();

    // Clicking the Dock icon with no open windows reopens the main window.
    let reopen = main_window.clone();
    application.on_reopen(move |cx| show_pane(&reopen, None, cx));

    application.run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-h", HideApplication, None),
            KeyBinding::new("cmd-w", CloseWindow, None),
            KeyBinding::new("cmd-m", MinimizeWindow, None),
            KeyBinding::new("ctrl-cmd-f", ToggleFullscreen, None),
            KeyBinding::new("cmd-,", ShowSettings, None),
            KeyBinding::new("cmd-l", ToggleListening, None),
            KeyBinding::new("cmd-k", ClearTranscript, None),
            KeyBinding::new("cmd-shift-c", CopyTranscript, None),
            KeyBinding::new("cmd-1", ShowPolish, None),
            KeyBinding::new("cmd-2", ShowBoth, None),
            KeyBinding::new("cmd-3", ShowEnglish, None),
        ]);

        // App-level actions work even when no window is open. Window-level
        // actions live on the root element.
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &HideApplication, cx| cx.hide());
        cx.on_action(|_: &CheckForUpdates, cx| {
            cx.activate(true);
            sparkle::check_for_updates();
        });
        let window = main_window.clone();
        cx.on_action(move |_: &ShowLive, cx| show_pane(&window, Some(Pane::Live), cx));
        let window = main_window.clone();
        cx.on_action(move |_: &ShowSettings, cx| show_pane(&window, Some(Pane::Settings), cx));

        sparkle::start();
        cx.set_menus(menus());
        show_pane(&main_window, Some(initial_pane), cx);
    });
}

fn menus() -> Vec<Menu> {
    let mut app_items = vec![
        MenuItem::action(format!("About {APP_NAME}"), ShowSettings),
        MenuItem::separator(),
    ];
    if sparkle::is_available() {
        app_items.push(MenuItem::action("Check for Updates…", CheckForUpdates));
    }
    app_items.extend([
        MenuItem::action("Settings…", ShowSettings),
        MenuItem::separator(),
        MenuItem::os_submenu("Services", SystemMenuType::Services),
        MenuItem::separator(),
        MenuItem::action(format!("Hide {APP_NAME}"), HideApplication),
        MenuItem::separator(),
        MenuItem::action(format!("Quit {APP_NAME}"), Quit),
    ]);
    vec![
        Menu {
            name: APP_NAME.into(),
            items: app_items,
        },
        Menu {
            name: "File".into(),
            items: vec![MenuItem::action("Close Window", CloseWindow)],
        },
        Menu {
            name: "Session".into(),
            items: vec![
                MenuItem::action("Start or Stop Listening", ToggleListening),
                MenuItem::separator(),
                MenuItem::action("Copy Transcript", CopyTranscript),
                MenuItem::action("Clear Transcript", ClearTranscript),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Polish", ShowPolish),
                MenuItem::action("Polish and English", ShowBoth),
                MenuItem::action("English", ShowEnglish),
                MenuItem::separator(),
                MenuItem::action("Transcript", ShowLive),
                MenuItem::action("Settings", ShowSettings),
            ],
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Minimize", MinimizeWindow),
                MenuItem::action("Enter Full Screen", ToggleFullscreen),
            ],
        },
    ]
}

/// Focuses the existing main window, or opens a new one, then optionally
/// switches it to `pane`.
fn show_pane(main_window: &MainWindow, pane: Option<Pane>, cx: &mut App) {
    let existing = main_window.borrow().as_ref().copied();
    if let Some(handle) = existing
        && handle
            .update(cx, |this, window, cx| {
                if let Some(pane) = pane {
                    this.select(pane, cx);
                }
                window.activate_window();
            })
            .is_ok()
    {
        cx.activate(true);
        return;
    }

    // Open at the right edge of the screen, beside the meeting app.
    let bounds = cx
        .primary_display()
        .map(|display| {
            let screen = display.bounds();
            let height = px(WINDOW_HEIGHT).min(screen.size.height - px(80.0));
            Bounds::new(
                gpui::point(
                    screen.origin.x + screen.size.width - px(WINDOW_WIDTH) - px(24.0),
                    screen.origin.y + (screen.size.height - height) / 2.0,
                ),
                size(px(WINDOW_WIDTH), height),
            )
        })
        .unwrap_or_else(|| Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx));
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(APP_NAME.into()),
            appears_transparent: true,
            ..Default::default()
        }),
        window_min_size: Some(size(px(340.0), px(360.0))),
        ..Default::default()
    };
    match cx.open_window(options, |window, cx| {
        cx.new(|cx| AppWindow::new(pane.unwrap_or(Pane::Live), window, cx))
    }) {
        Ok(handle) => {
            *main_window.borrow_mut() = Some(handle);
            cx.activate(true);
        }
        Err(error) => tracing::error!(%error, "could not open the main window"),
    }
}

enum Listening {
    Idle,
    Running {
        /// Dropping it stops the audio capture.
        _session: live::Session,
        started: Instant,
        heard_audio: bool,
    },
    /// Audio stopped; waiting for Soniox to send the last words.
    Stopping,
}

enum LoginItem {
    Loading,
    Ready(LoginItemStatus),
    Failed(SharedString),
}

pub struct AppWindow {
    pane: Pane,
    settings: Settings,
    api_key: Option<(String, KeySource)>,
    listening: Listening,
    transcript: Transcript,
    level: f32,
    /// Last problem worth showing: a failed session, a settings error, …
    error: Option<SharedString>,
    notice: Option<SharedString>,
    scroll: ScrollHandle,
    login_item: LoginItem,
    focus_handle: FocusHandle,
    // Dropping a GPUI task cancels it, so in-flight work is owned here.
    _events_task: Option<Task<()>>,
    _login_item_task: Option<Task<()>>,
}

impl AppWindow {
    fn new(pane: Pane, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut error = None;
        let settings = Settings::load().unwrap_or_else(|e| {
            tracing::error!(error = %e, "could not load settings");
            error = Some(format!("Could not load settings: {e}").into());
            Settings::default()
        });
        let api_key = api_key::load().unwrap_or_else(|e| {
            tracing::error!(error = %e, "could not load the API key");
            error = Some(format!("Could not read the API key: {e}").into());
            None
        });
        window_level::set_floating(window, settings.keep_on_top);
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);
        let mut this = Self {
            pane,
            settings,
            api_key,
            listening: Listening::Idle,
            transcript: Transcript::default(),
            level: 0.0,
            error,
            notice: None,
            scroll: ScrollHandle::new(),
            login_item: LoginItem::Loading,
            focus_handle,
            _events_task: None,
            _login_item_task: None,
        };
        this.request_login_item(None, cx);
        this
    }

    fn select(&mut self, pane: Pane, cx: &mut Context<Self>) {
        self.pane = pane;
        cx.notify();
    }

    /// Applies a change and persists it. Settings files are tiny, so saving
    /// inline is fine.
    fn update_settings(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Settings)) {
        change(&mut self.settings);
        if let Err(error) = self.settings.save() {
            tracing::error!(%error, "could not save settings");
            self.error = Some(format!("Could not save settings: {error}").into());
        }
        cx.notify();
    }

    fn set_view(&mut self, view: View, cx: &mut Context<Self>) {
        self.update_settings(cx, |settings| settings.view = view);
    }

    fn toggle_listening(&mut self, cx: &mut Context<Self>) {
        match self.listening {
            Listening::Idle => self.start(cx),
            Listening::Running { .. } => self.stop(cx),
            Listening::Stopping => {}
        }
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        let Some((key, _)) = self.api_key.clone() else {
            self.error = Some("Add your Soniox API key first.".into());
            self.select(Pane::Settings, cx);
            return;
        };
        let (session, events) = live::start(self.settings.source, key);
        self.listening = Listening::Running {
            _session: session,
            started: Instant::now(),
            heard_audio: false,
        };
        self.error = None;
        self.notice = None;
        self.level = 0.0;
        self._events_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                let finished = matches!(event, Event::Finished);
                if this
                    .update(cx, |this, cx| this.on_event(event, cx))
                    .is_err()
                    || finished
                {
                    break;
                }
            }
        }));
        cx.notify();
    }

    /// Stops capturing; the session ends once Soniox finalises the last words.
    fn stop(&mut self, cx: &mut Context<Self>) {
        if matches!(self.listening, Listening::Running { .. }) {
            self.listening = Listening::Stopping;
            self.level = 0.0;
            cx.notify();
        }
    }

    fn on_event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Tokens(tokens) => {
                let follow = self.is_scrolled_to_bottom();
                self.transcript.apply(&tokens);
                if follow {
                    self.scroll.scroll_to_bottom();
                }
            }
            Event::Level(level) => {
                self.level = level;
                if let Listening::Running { heard_audio, .. } = &mut self.listening {
                    *heard_audio |= level > SILENCE_LEVEL;
                }
            }
            Event::Notice(notice) => self.notice = Some(notice.into()),
            Event::Failed(error) => {
                self.error = Some(error.into());
                // Dropping the session stops the audio.
                self.listening = Listening::Stopping;
            }
            Event::Finished => {
                self.transcript.close_current();
                self.listening = Listening::Idle;
                self.level = 0.0;
            }
        }
        cx.notify();
    }

    fn is_scrolled_to_bottom(&self) -> bool {
        let offset = -self.scroll.offset().y;
        offset >= self.scroll.max_offset().height - px(48.0)
    }

    fn copy_transcript(&mut self, cx: &mut Context<Self>) {
        if !self.transcript.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.transcript.to_text(self.settings.view),
            ));
        }
    }

    fn clear_transcript(&mut self, cx: &mut Context<Self>) {
        self.transcript.clear();
        cx.notify();
    }

    fn paste_api_key(&mut self, cx: &mut Context<Self>) {
        let clipboard = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        match api_key::save(&clipboard) {
            Ok(key) => {
                // A key in the environment still wins; keep showing that one.
                if !matches!(self.api_key, Some((_, KeySource::Environment))) {
                    self.api_key = Some((key, KeySource::File));
                }
                self.error = None;
            }
            Err(error) => self.error = Some(format!("Could not save the key: {error}").into()),
        }
        cx.notify();
    }

    fn remove_api_key(&mut self, cx: &mut Context<Self>) {
        match api_key::remove() {
            Ok(()) => {
                if matches!(self.api_key, Some((_, KeySource::File))) {
                    self.api_key = None;
                }
            }
            Err(error) => self.error = Some(format!("Could not remove the key: {error}").into()),
        }
        cx.notify();
    }

    /// `None` refreshes the status; `Some(enabled)` registers or unregisters.
    fn request_login_item(&mut self, enabled: Option<bool>, cx: &mut Context<Self>) {
        self._login_item_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match enabled {
                        Some(enabled) => login_item::set_enabled(enabled),
                        None => login_item::status(),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.login_item = match result {
                    Ok(status) => LoginItem::Ready(status),
                    Err(error) => LoginItem::Failed(format!("{error}").into()),
                };
                cx.notify();
            })
            .ok();
        }));
    }

    fn render_titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let on_top = self.settings.keep_on_top;
        let in_settings = self.pane == Pane::Settings;
        ui::titlebar(
            APP_NAME,
            div()
                .flex()
                .gap_1()
                .child(
                    ui::icon_button("keep-on-top", "pin", on_top, true).on_click(cx.listener(
                        move |this, _, window, cx| {
                            window_level::set_floating(window, !on_top);
                            this.update_settings(cx, |settings| settings.keep_on_top = !on_top);
                        },
                    )),
                )
                .child(
                    ui::icon_button("settings", "gearshape", in_settings, true).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.select(
                                if in_settings {
                                    Pane::Live
                                } else {
                                    Pane::Settings
                                },
                                cx,
                            )
                        }),
                    ),
                ),
        )
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = self.settings.view;
        let source = self.settings.source;
        let idle = matches!(self.listening, Listening::Idle);
        let views = [
            (View::Polish, "PL"),
            (View::Both, "PL + EN"),
            (View::English, "EN"),
        ];
        let sources = [
            (AudioSource::Meeting, "Meeting"),
            (AudioSource::Microphone, "Mic"),
        ];
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .px_3()
            .py_2()
            .child(
                ui::segmented_control().children(views.map(|(option, label)| {
                    ui::segment(label, label, view == option, true)
                        .on_click(cx.listener(move |this, _, _, cx| this.set_view(option, cx)))
                })),
            )
            // The source is fixed while a session runs.
            .child(
                ui::segmented_control().children(sources.map(|(option, label)| {
                    ui::segment(label, label, source == option, idle).on_click(cx.listener(
                        move |this, _, _, cx| {
                            if matches!(this.listening, Listening::Idle) {
                                this.update_settings(cx, |settings| settings.source = option);
                            }
                        },
                    ))
                })),
            )
    }

    fn render_transcript(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.transcript.is_empty() {
            let (symbol, title, detail): (_, SharedString, SharedString) =
                match (&self.api_key, &self.listening) {
                    (None, _) => (
                        "key",
                        "Add your Soniox API key".into(),
                        "Copy the key from console.soniox.com, then paste it in Settings.".into(),
                    ),
                    (_, Listening::Running { .. }) => (
                        "waveform",
                        "Listening…".into(),
                        "Words appear here as people speak.".into(),
                    ),
                    _ => (
                        "captions.bubble",
                        "Ready when your meeting is".into(),
                        match self.settings.source {
                            AudioSource::Meeting => "Press Start to caption what the Mac plays: the other people in your call.",
                            AudioSource::Microphone => "Press Start to caption what the microphone hears.",
                        }
                        .into(),
                    ),
                };
            let mut empty = ui::empty_state(symbol, title, detail).max_w(px(300.0));
            if self.api_key.is_none() {
                empty = empty.child(
                    div().pt_2().child(
                        ui::button(
                            "open-key-settings",
                            "Open Settings",
                            ButtonStyle::Primary,
                            true,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.select(Pane::Settings, cx))),
                    ),
                );
            }
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(empty)
                .into_any_element();
        }

        let view = self.settings.view;
        let mut previous_speaker: Option<&Option<String>> = None;
        let mut rows = Vec::new();
        for utterance in self.transcript.utterances() {
            let color = ui::speaker_color(utterance.speaker.as_deref());
            if previous_speaker != Some(&utterance.speaker) {
                let label = match &utterance.speaker {
                    Some(speaker) => format!("Speaker {speaker}"),
                    None => "Speaker".to_owned(),
                };
                rows.push(ui::speaker_label(label, color).into_any_element());
            }
            previous_speaker = Some(&utterance.speaker);
            rows.push(render_bubble(utterance, view, color).into_any_element());
        }

        div()
            .id("transcript")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .px_3()
                    .pb_3()
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (status, positive): (SharedString, bool) = match (&self.error, &self.listening) {
            (Some(error), _) => (error.clone(), false),
            (None, Listening::Running {
                started,
                heard_audio: false,
                ..
            }) if started.elapsed() > SILENCE_HINT_AFTER => (
                match self.settings.source {
                    AudioSource::Meeting => "No sound yet. Is the call playing audio? Check System Audio Recording in Privacy settings.",
                    AudioSource::Microphone => "No sound from the microphone yet.",
                }
                .into(),
                false,
            ),
            (None, Listening::Running { .. }) => (
                self.notice.clone().unwrap_or_else(|| "Listening".into()),
                true,
            ),
            (None, Listening::Stopping) => ("Finishing the last words…".into(), true),
            (None, Listening::Idle) => (
                self.notice.clone().unwrap_or_else(|| "Not listening".into()),
                true,
            ),
        };
        let running = matches!(self.listening, Listening::Running { .. });
        let stopping = matches!(self.listening, Listening::Stopping);
        let has_text = !self.transcript.is_empty();

        ui::footer()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(ui::level_meter(self.level, running))
                    .child(
                        div()
                            .min_w_0()
                            .text_size(px(12.0))
                            .text_color(rgb(if positive { ui::MUTED } else { ui::NEGATIVE }))
                            .child(status),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        ui::icon_button("copy", "doc.on.doc", false, has_text)
                            .on_click(cx.listener(|this, _, _, cx| this.copy_transcript(cx))),
                    )
                    .child(
                        ui::icon_button("clear", "trash", false, has_text)
                            .on_click(cx.listener(|this, _, _, cx| this.clear_transcript(cx))),
                    )
                    .child(
                        div().pl_1().child(
                            ui::button(
                                "listen",
                                if running {
                                    "Stop"
                                } else if stopping {
                                    "Stopping…"
                                } else {
                                    "Start"
                                },
                                if running {
                                    ButtonStyle::Secondary
                                } else {
                                    ButtonStyle::Primary
                                },
                                !stopping,
                            )
                            .min_w(px(72.0))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_listening(cx))),
                        ),
                    ),
            )
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let (key_detail, key_on_file): (SharedString, bool) = match &self.api_key {
            None => ("Not set. Copy it, then paste it here.".into(), false),
            Some((key, KeySource::File)) => (format!("Saved: {}", api_key::mask(key)).into(), true),
            Some((key, KeySource::Environment)) => (
                format!(
                    "From the {} environment variable: {}",
                    api_key::ENV_VAR,
                    api_key::mask(key)
                )
                .into(),
                false,
            ),
        };
        let mut key_panel = ui::panel()
            .child(ui::setting_row(
                "Soniox API key",
                key_detail,
                ui::button(
                    "paste-key",
                    "Paste from Clipboard",
                    if self.api_key.is_none() {
                        ButtonStyle::Primary
                    } else {
                        ButtonStyle::Secondary
                    },
                    true,
                )
                .on_click(cx.listener(|this, _, _, cx| this.paste_api_key(cx))),
            ))
            .child(ui::divider())
            .child(ui::setting_row(
                "Get a key",
                "Create one in the Soniox console.",
                ui::button("open-console", "Open Console", ButtonStyle::Secondary, true)
                    .on_click(|_, _, cx| cx.open_url(SONIOX_CONSOLE_URL)),
            ));
        if key_on_file {
            key_panel = key_panel.child(ui::divider()).child(ui::setting_row(
                "Forget the key",
                "Deletes it from this Mac.",
                ui::button("remove-key", "Remove", ButtonStyle::Secondary, true)
                    .on_click(cx.listener(|this, _, _, cx| this.remove_api_key(cx))),
            ));
        }

        let on_top = self.settings.keep_on_top;
        let window_panel = ui::panel().child(ui::setting_row(
            "Keep on top",
            "Float above the meeting app, also in full screen.",
            ui::toggle("keep-on-top-toggle", on_top).on_click(cx.listener(
                move |this, _, window, cx| {
                    window_level::set_floating(window, !on_top);
                    this.update_settings(cx, |settings| settings.keep_on_top = !on_top);
                },
            )),
        ));

        let (enabled, detail): (bool, SharedString) = match &self.login_item {
            LoginItem::Loading => (false, "Checking…".into()),
            LoginItem::Ready(LoginItemStatus::Enabled) => (true, "Opens when you log in.".into()),
            LoginItem::Ready(LoginItemStatus::Disabled) => {
                (false, "Requires the signed app in /Applications.".into())
            }
            LoginItem::Ready(LoginItemStatus::RequiresApproval) => (
                true,
                "Waiting for approval in System Settings → Login Items.".into(),
            ),
            LoginItem::Failed(error) => (false, error.clone()),
        };
        let mut startup = ui::panel().child(ui::setting_row(
            "Open at login",
            detail,
            ui::toggle("login-item", enabled).on_click(cx.listener(move |this, _, _, cx| {
                this.login_item = LoginItem::Loading;
                this.request_login_item(Some(!enabled), cx);
                cx.notify();
            })),
        ));
        if matches!(
            self.login_item,
            LoginItem::Ready(LoginItemStatus::RequiresApproval)
        ) {
            startup = startup.child(ui::divider()).child(ui::setting_row(
                "Approve login item",
                "macOS asks you to confirm new login items.",
                ui::button(
                    "open-login-items",
                    "Open System Settings",
                    ButtonStyle::Secondary,
                    true,
                )
                .on_click(|_, _, _| login_item::open_system_settings()),
            ));
        }

        ui::pane("settings-pane", "Settings", "Saved as you change them.")
            .child(
                ui::pane_body()
                    .child(key_panel)
                    .children(self.error.clone().map(|e| ui::status_text(e, false)))
                    .child(window_panel)
                    .child(startup)
                    .child(render_about()),
            )
            .into_any_element()
    }
}

fn render_bubble(utterance: &Utterance, view: View, accent: u32) -> impl IntoElement {
    let original = live_text(&utterance.original, &utterance.original_pending);
    let translation = live_text(&utterance.translation, &utterance.translation_pending);
    let bubble = ui::bubble(accent);
    match view {
        View::English => bubble.child(ui::primary_text().child(original)),
        View::Polish => bubble.child(ui::primary_text().child(translation)),
        View::Both => bubble
            .child(ui::primary_text().child(translation))
            .child(ui::secondary_text().child(original)),
    }
}

/// Final text followed by the still-changing tail, which is drawn fainter.
/// Shows an ellipsis until the first word arrives.
fn live_text(done: &str, pending: &str) -> StyledText {
    let done = done.trim_start();
    let pending = if done.is_empty() {
        pending.trim_start()
    } else {
        pending
    };
    if done.is_empty() && pending.is_empty() {
        return StyledText::new("…").with_highlights([(0.."…".len(), faint())]);
    }
    let text = format!("{done}{pending}");
    StyledText::new(text.clone()).with_highlights([(done.len()..text.len(), faint())])
}

fn faint() -> HighlightStyle {
    HighlightStyle {
        color: Some(rgb(ui::MUTED).into()),
        ..Default::default()
    }
}

fn render_about() -> impl IntoElement {
    let data_dir = app_paths::support_dir().ok();
    let data_detail: SharedString = data_dir
        .as_ref()
        .map(|path| path.display().to_string().into())
        .unwrap_or_else(|| "Unavailable".into());
    let updates_available = sparkle::is_available();

    let mut info = ui::panel()
        .child(ui::setting_row(
            "Version",
            format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION")),
            div(),
        ))
        .child(ui::divider())
        .child(ui::setting_row(
            "Data folder",
            data_detail,
            ui::button(
                "reveal-data",
                "Show in Finder",
                ButtonStyle::Secondary,
                data_dir.is_some(),
            )
            .on_click(move |_, _, cx| {
                if let Some(path) = &data_dir {
                    cx.reveal_path(path);
                }
            }),
        ));
    if updates_available {
        info = info.child(ui::divider()).child(ui::setting_row(
            "Updates",
            "Checks automatically through Sparkle.",
            ui::button(
                "check-updates",
                "Check for Updates…",
                ButtonStyle::Secondary,
                true,
            )
            .on_click(|_, window, cx| window.dispatch_action(Box::new(CheckForUpdates), cx)),
        ));
    }
    info
}

impl Render for AppWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.pane {
            Pane::Live => div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(self.render_toolbar(cx))
                .child(self.render_transcript(cx))
                .child(self.render_footer(cx))
                .into_any_element(),
            Pane::Settings => self.render_settings(cx),
        };
        ui::window_frame()
            .track_focus(&self.focus_handle)
            .on_action(|_: &CloseWindow, window, _| window.remove_window())
            .on_action(|_: &MinimizeWindow, window, _| window.minimize_window())
            .on_action(|_: &ToggleFullscreen, window, _| window.toggle_fullscreen())
            .on_action(cx.listener(|this, _: &ToggleListening, _, cx| this.toggle_listening(cx)))
            .on_action(cx.listener(|this, _: &CopyTranscript, _, cx| this.copy_transcript(cx)))
            .on_action(cx.listener(|this, _: &ClearTranscript, _, cx| this.clear_transcript(cx)))
            .on_action(cx.listener(|this, _: &ShowPolish, _, cx| this.set_view(View::Polish, cx)))
            .on_action(cx.listener(|this, _: &ShowBoth, _, cx| this.set_view(View::Both, cx)))
            .on_action(cx.listener(|this, _: &ShowEnglish, _, cx| this.set_view(View::English, cx)))
            .child(self.render_titlebar(cx))
            .child(body)
    }
}
