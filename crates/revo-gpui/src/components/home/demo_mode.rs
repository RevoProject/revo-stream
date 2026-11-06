use gpui::{div, prelude::*, Context, IntoElement, Render, Window};

use crate::theme::RevoTheme;

pub struct DemoMode {
    pub theme: RevoTheme,
    pub message: String,
}

impl Default for DemoMode {
    fn default() -> Self {
        Self {
            theme: RevoTheme::classic(),
            message: "Disable DEMO mode to start and see the live preview here.".into(),
        }
    }
}

impl Render for DemoMode {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .bg(self.theme.surface)
            .text_color(self.theme.text)
            .child(div().text_color(self.theme.text_strong).child("RevoStream"))
            .child(
                div()
                    .text_color(self.theme.text_muted)
                    .child(self.message.clone()),
            )
    }
}
