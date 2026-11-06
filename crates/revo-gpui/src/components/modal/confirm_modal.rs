use gpui::{Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig},
    theme::RevoTheme,
};

pub struct ConfirmModal {
    pub theme: RevoTheme,
    pub title: String,
    pub message: String,
    pub destructive: bool,
}

impl ConfirmModal {
    pub fn answer(&self, accepted: bool) -> crate::ModalEvent {
        crate::ModalEvent::ConfirmChoice(accepted)
    }
}

impl Render for ConfirmModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let label = if self.destructive {
            "Delete"
        } else {
            "Confirm"
        };
        modal_shell(
            &ModalConfig::confirmation(self.title.clone(), label),
            self.theme,
            vec![SharedString::from(self.message.clone())],
        )
    }
}
