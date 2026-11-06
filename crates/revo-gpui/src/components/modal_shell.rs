use gpui::{div, prelude::*, Context, IntoElement, Render, SharedString, Window};

use crate::theme::RevoTheme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalSize {
    Small,
    Medium,
    Large,
    Workspace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalKind {
    Standard,
    Quick,
    Confirmation,
}

#[derive(Clone, Debug)]
pub struct ModalConfig {
    pub title: SharedString,
    pub size: ModalSize,
    pub kind: ModalKind,
    pub confirm_label: Option<SharedString>,
    pub cancel_label: Option<SharedString>,
    pub show_close: bool,
}

impl ModalConfig {
    pub fn standard(title: impl Into<SharedString>, size: ModalSize) -> Self {
        Self {
            title: title.into(),
            size,
            kind: ModalKind::Standard,
            confirm_label: Some("Save".into()),
            cancel_label: Some("Cancel".into()),
            show_close: true,
        }
    }

    pub fn quick(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            size: ModalSize::Small,
            kind: ModalKind::Quick,
            confirm_label: Some("Apply".into()),
            cancel_label: Some("Cancel".into()),
            show_close: true,
        }
    }

    pub fn confirmation(title: impl Into<SharedString>, confirm: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            size: ModalSize::Small,
            kind: ModalKind::Confirmation,
            confirm_label: Some(confirm.into()),
            cancel_label: Some("Cancel".into()),
            show_close: true,
        }
    }
}

pub struct ModalShell {
    pub config: ModalConfig,
    pub theme: RevoTheme,
    pub body: Vec<SharedString>,
}

impl ModalShell {
    pub fn new(config: ModalConfig, theme: RevoTheme, body: Vec<SharedString>) -> Self {
        Self {
            config,
            theme,
            body,
        }
    }
}

/// Builds presentation-only modal chrome for wrappers that own their controller state.
/// Close, cancel, and confirm controls intentionally have no listeners until a wrapper
/// supplies native keyboard/input integration.
pub fn modal_shell(
    config: &ModalConfig,
    theme: RevoTheme,
    lines: impl IntoIterator<Item = SharedString>,
) -> gpui::Div {
    let lines: Vec<gpui::AnyElement> = lines
        .into_iter()
        .map(|line| {
            div()
                .text_color(theme.text_secondary)
                .child(line)
                .into_any_element()
        })
        .collect();
    modal_shell_with_body(
        config,
        theme,
        div().flex().flex_col().gap_2().children(lines),
        None,
    )
}

/// Modal shell variant that renders an arbitrary element body, for
/// interactive modals such as Settings that own their own field layout. When
/// `footer` is `Some`, it replaces the default label-only footer so the modal
/// can wire real confirm/cancel behavior.
pub fn modal_shell_with_body(
    config: &ModalConfig,
    theme: RevoTheme,
    body: impl IntoElement,
    footer: Option<gpui::AnyElement>,
) -> gpui::Div {
    let width = match config.size {
        ModalSize::Small => 360.,
        ModalSize::Medium => 560.,
        ModalSize::Large => 800.,
        ModalSize::Workspace => 1040.,
    };
    let body = div().flex().flex_col().gap_2().p_4().child(body);
    let mut header = div()
        .flex()
        .flex_row()
        .justify_between()
        .p_3()
        .bg(theme.surface_elevated)
        .child(
            div()
                .text_color(theme.text_strong)
                .child(config.title.clone()),
        );
    if config.show_close {
        header = header.child(
            div()
                .id("modal-close-structural")
                .text_color(theme.text_muted)
                .child("Close"),
        );
    }
    let footer = footer.unwrap_or_else(|| {
        let mut footer = div().flex().flex_row().justify_end().gap_2().p_3();
        if let Some(label) = config.cancel_label.clone() {
            footer = footer.child(
                div()
                    .id("modal-cancel-structural")
                    .px_3()
                    .py_2()
                    .bg(theme.surface_active)
                    .rounded_md()
                    .child(label),
            );
        }
        if let Some(label) = config.confirm_label.clone() {
            footer = footer.child(
                div()
                    .id("modal-confirm-structural")
                    .px_3()
                    .py_2()
                    .bg(theme.accent)
                    .rounded_md()
                    .child(label),
            );
        }
        footer.into_any_element()
    });
    div()
        .flex()
        .items_center()
        .justify_center()
        .size_full()
        .bg(theme.overlay)
        .child(
            div()
                .w(gpui::px(width))
                .max_w_full()
                .bg(theme.surface)
                .border_1()
                .border_color(theme.border_strong)
                .rounded_lg()
                .text_color(theme.text)
                .child(header)
                .child(body)
                .child(footer),
        )
}

impl Render for ModalShell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        modal_shell(&self.config, self.theme, self.body.iter().cloned())
    }
}
