use gpui::{Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig},
    theme::RevoTheme,
    ModalEvent,
};

pub struct StreamConfirmModal {
    pub theme: RevoTheme,
    pub title: String,
    pub message: String,
}

impl StreamConfirmModal {
    pub fn answer(&self, accepted: bool) -> ModalEvent {
        ModalEvent::ConfirmChoice(accepted)
    }
}

impl Render for StreamConfirmModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        modal_shell(
            &ModalConfig::confirmation(self.title.clone(), "Continue"),
            self.theme,
            vec![SharedString::from(self.message.clone())],
        )
    }
}
