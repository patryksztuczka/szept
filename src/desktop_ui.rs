//! Shared visual tokens and controls. Every pane builds from these so the app
//! keeps one look; add a helper here instead of styling ad hoc in a pane.

use gpui::{
    AnyElement, Div, ElementId, FontWeight, IntoElement, SharedString, Stateful, div, prelude::*,
    px, rgb,
};
use gpui_symbols::{RenderingMode, SymbolWeight};

pub const CHROME: u32 = 0x111111;
pub const CANVAS: u32 = 0x161616;
pub const SURFACE: u32 = 0x1d1d1d;
pub const SURFACE_HOVER: u32 = 0x252525;
pub const SURFACE_SELECTED: u32 = 0x2e2e2e;
pub const LINE: u32 = 0x2a2a2a;
pub const ACCENT: u32 = 0x3b5cf6;
pub const ACCENT_HOVER: u32 = 0x5070ff;
pub const POSITIVE: u32 = 0x69d89f;
pub const NEGATIVE: u32 = 0xf17471;
pub const TEXT: u32 = 0xf2f2f2;
pub const TEXT_SOFT: u32 = 0xd0d0d0;
pub const MUTED: u32 = 0x969696;
pub const FAINT: u32 = 0x6a6a6a;

/// Speaker accents, picked by speaker number so a voice keeps its colour.
const SPEAKER_COLORS: [u32; 6] = [0x7aa2ff, 0xf0a35e, 0x69d89f, 0xd88be8, 0xf17471, 0x5fd0d6];

/// Height of the transparent titlebar, which holds the traffic lights.
pub const TITLEBAR_HEIGHT: f32 = 40.0;
/// Room the traffic-light buttons need on the titlebar's left.
pub const TRAFFIC_LIGHTS_WIDTH: f32 = 76.0;
pub const CONTROL_HEIGHT: f32 = 30.0;
pub const PANEL_RADIUS: f32 = 10.0;
pub const PANE_CONTENT_WIDTH: f32 = 560.0;
pub const BUBBLE_RADIUS: f32 = 12.0;

/// An SF Symbol, e.g. `"gearshape"`. Browse names in Apple's SF Symbols app.
pub fn icon(name: &'static str, size: f32, color: u32) -> AnyElement {
    gpui_symbols::Icon::new(name)
        .size(px(size))
        .color(rgb(color))
        .weight(SymbolWeight::Medium)
        .rendering_mode(RenderingMode::Monochrome)
        .into_any_element()
}

pub fn window_frame() -> Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(rgb(CANVAS))
        .text_color(rgb(TEXT))
        .text_size(px(13.0))
}

/// The scaffold every pane renders: a scrollable column with a title header.
/// Add the pane's `pane_body` as its next child.
pub fn pane(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .h_full()
        .flex()
        .flex_col()
        .overflow_y_scroll()
        .pt_4()
        .pb_8()
        .px_5()
        .child(
            div()
                .max_w(px(PANE_CONTENT_WIDTH))
                .flex()
                .flex_col()
                .gap_1()
                .pb_5()
                .child(
                    div()
                        .text_size(px(18.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.into()),
                )
                .child(div().text_color(rgb(MUTED)).child(subtitle.into())),
        )
}

/// Pane bodies go inside this so every pane shares one content width.
pub fn pane_body() -> Div {
    div()
        .max_w(px(PANE_CONTENT_WIDTH))
        .flex()
        .flex_col()
        .gap_3()
}

pub fn panel() -> Div {
    div()
        .flex()
        .flex_col()
        .rounded(px(PANEL_RADIUS))
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(LINE))
}

/// A labelled row inside a `panel`, with a control aligned to the right.
pub fn setting_row(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    control: impl IntoElement,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .px_4()
        .py_3()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(div().font_weight(FontWeight::MEDIUM).child(title.into()))
                .child(div().text_color(rgb(MUTED)).child(detail.into())),
        )
        .child(div().flex_none().child(control))
}

pub fn divider() -> Div {
    div().h(px(1.0)).bg(rgb(LINE))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    Primary,
    Secondary,
}

pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    style: ButtonStyle,
    enabled: bool,
) -> Stateful<Div> {
    let (background, hover) = match style {
        ButtonStyle::Primary => (ACCENT, ACCENT_HOVER),
        ButtonStyle::Secondary => (SURFACE_HOVER, SURFACE_SELECTED),
    };
    div()
        .id(id)
        .h(px(CONTROL_HEIGHT))
        .px_3()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .font_weight(FontWeight::MEDIUM)
        .bg(rgb(background))
        .when(!enabled, |button| button.opacity(0.45))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|button| button.bg(rgb(hover)))
        })
        .child(label.into())
}

pub fn toggle(id: impl Into<ElementId>, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(36.0))
        .h(px(20.0))
        .p(px(2.0))
        .flex()
        .when(on, |track| track.justify_end())
        .rounded_full()
        .cursor_pointer()
        .bg(rgb(if on { ACCENT } else { SURFACE_SELECTED }))
        .child(div().size(px(16.0)).rounded_full().bg(rgb(TEXT)))
}

/// Small coloured status text, e.g. under a control that can fail.
pub fn status_text(message: impl Into<SharedString>, positive: bool) -> Div {
    div()
        .text_color(rgb(if positive { POSITIVE } else { NEGATIVE }))
        .child(message.into())
}

/// The window's top strip: traffic lights on the left, `title` beside them,
/// and `actions` on the right.
pub fn titlebar(title: impl Into<SharedString>, actions: impl IntoElement) -> Div {
    div()
        .h(px(TITLEBAR_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .pl(px(TRAFFIC_LIGHTS_WIDTH))
        .pr_2()
        .bg(rgb(CHROME))
        .border_b_1()
        .border_color(rgb(LINE))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(TEXT_SOFT))
                .child(title.into()),
        )
        .child(div().flex().items_center().gap_1().child(actions))
}

/// A square button holding one SF Symbol. `active` tints it with the accent.
pub fn icon_button(
    id: impl Into<ElementId>,
    symbol: &'static str,
    active: bool,
    enabled: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .when(!enabled, |button| button.opacity(0.35))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|button| button.bg(rgb(SURFACE_HOVER)))
        })
        .child(icon(
            symbol,
            14.0,
            if active { ACCENT_HOVER } else { MUTED },
        ))
}

/// A row of mutually exclusive `segment`s.
pub fn segmented_control() -> Div {
    div()
        .flex()
        .p(px(2.0))
        .gap(px(2.0))
        .rounded(px(7.0))
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(LINE))
}

pub fn segment(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    enabled: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(24.0))
        .px_2p5()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.0))
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(if selected { TEXT } else { MUTED }))
        .when(selected, |segment| segment.bg(rgb(SURFACE_SELECTED)))
        .when(!enabled, |segment| segment.opacity(0.45))
        .when(enabled && !selected, |segment| {
            segment
                .cursor_pointer()
                .hover(|segment| segment.text_color(rgb(TEXT_SOFT)))
        })
        .child(label.into())
}

pub fn speaker_color(speaker: Option<&str>) -> u32 {
    let index = speaker
        .and_then(|speaker| speaker.parse::<usize>().ok())
        .map_or(0, |number| number.saturating_sub(1));
    SPEAKER_COLORS[index % SPEAKER_COLORS.len()]
}

/// The coloured name shown above a speaker's first bubble in a run.
pub fn speaker_label(label: impl Into<SharedString>, color: u32) -> Div {
    div()
        .flex()
        .items_center()
        .gap_1p5()
        .pt_2()
        .pl_1()
        .text_size(px(11.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(MUTED))
        .child(div().size(px(7.0)).rounded_full().bg(rgb(color)))
        .child(label.into())
}

/// One chat bubble. Add the main text, then any secondary text.
pub fn bubble(accent: u32) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .px_3()
        .py_2()
        .rounded(px(BUBBLE_RADIUS))
        .rounded_tl(px(4.0))
        .bg(rgb(SURFACE))
        .border_l_2()
        .border_color(rgb(accent))
}

/// Main bubble text: the translation, or the original when shown alone.
pub fn primary_text() -> Div {
    div()
        .text_size(px(15.0))
        .line_height(px(21.0))
        .text_color(rgb(TEXT))
}

/// The original under a translation.
pub fn secondary_text() -> Div {
    div()
        .text_size(px(12.5))
        .line_height(px(17.0))
        .text_color(rgb(MUTED))
}

/// Bottom bar under the transcript.
pub fn footer() -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_3()
        .py_2p5()
        .bg(rgb(CHROME))
        .border_t_1()
        .border_color(rgb(LINE))
}

/// Three bars that rise with the input level, 0.0 to 1.0. Dimmed when idle.
pub fn level_meter(level: f32, active: bool) -> Div {
    // Speech peaks sit well below full scale; a square root spreads them out.
    let level = level.clamp(0.0, 1.0).sqrt();
    div()
        .h(px(14.0))
        .flex()
        .items_end()
        .gap(px(2.0))
        .children([0.15f32, 0.35, 0.6].map(|threshold| {
            let lit = active && level > threshold;
            div()
                .w(px(3.0))
                .h(px(4.0 + 10.0 * threshold / 0.6))
                .rounded(px(1.5))
                .bg(rgb(if lit { POSITIVE } else { SURFACE_SELECTED }))
        }))
}

/// Centred guidance shown when there is nothing else to see.
pub fn empty_state(
    symbol: &'static str,
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_2()
        .px_6()
        .text_center()
        .child(icon(symbol, 28.0, FAINT))
        .child(
            div()
                .pt_1()
                .text_size(px(15.0))
                .font_weight(FontWeight::MEDIUM)
                .child(title.into()),
        )
        .child(
            div()
                .text_color(rgb(MUTED))
                .line_height(px(19.0))
                .child(detail.into()),
        )
}
