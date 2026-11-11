use gpui::{div, prelude::*, px, Context, IntoElement, Render, SharedString, Window};

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

/// Shared header close control for modal shells: an icon button that turns
/// danger-red on hover (plan §2.3). The caller wires `on_click` because the
/// listener is bound to the modal entity.
pub fn close_x_button(id: &'static str, theme: RevoTheme) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .w(px(28.))
        .h(px(28.))
        .rounded_md()
        .hover(|style| style.bg(theme.danger))
        .child(crate::components::icon::icon(
            theme.text_muted,
            "close",
            14.,
        ))
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
        None,
    )
}

/// Modal card without the full-screen overlay wrapper. Callers that need
/// their own backdrop-click-to-close behavior render this above a backdrop
/// sibling instead of using [`modal_shell_with_body`]. When a caller supplies
/// no footer/close elements, the structural footer renders inert labels only
/// for configs that declare `show_close` false; otherwise a `✕` header
/// button is expected to be wired by the caller.
pub fn modal_card(
    config: &ModalConfig,
    theme: RevoTheme,
    body: impl IntoElement,
    footer: Option<gpui::AnyElement>,
    close: Option<gpui::AnyElement>,
) -> gpui::Div {
    let width = match config.size {
        ModalSize::Small => 360.,
        ModalSize::Medium => 560.,
        ModalSize::Large => 800.,
        ModalSize::Workspace => 1040.,
    };
    // Taller-than-window cards clamp to the viewport (15px top/bottom
    // margin comes from the shell padding) and scroll their body.
    let body = div()
        .id("modal-card-body")
        .flex()
        .flex_col()
        .gap_2()
        .p_4()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .scrollbar_width(px(8.))
        .child(body);
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
        header = header.child(close.unwrap_or_else(|| {
            // No wired close: hide the control instead of rendering a dead
            // label (the footer Cancel is the working dismissal path).
            div().into_any_element()
        }));
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
        .w(gpui::px(width))
        .max_w_full()
        .max_h_full()
        .flex()
        .flex_col()
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border_strong)
        .rounded_lg()
        .text_color(theme.text)
        .child(header)
        .child(body)
        .child(footer)
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
    close: Option<gpui::AnyElement>,
) -> gpui::Div {
    // 15px top/bottom margin keeps every card inside the window; taller
    // bodies scroll inside the card instead. The shell occludes: while a
    // modal is open, clicks must never land on the panels behind it.
    div()
        .flex()
        .items_center()
        .justify_center()
        .size_full()
        .pt(px(15.))
        .pb(px(15.))
        .bg(theme.overlay)
        .occlude()
        .child(modal_card(config, theme, body, footer, close))
}

impl Render for ModalShell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        modal_shell(&self.config, self.theme, self.body.iter().cloned())
    }
}
