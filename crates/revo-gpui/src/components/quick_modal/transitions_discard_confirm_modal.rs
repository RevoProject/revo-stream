use gpui::{Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig},
    theme::RevoTheme,
    ModalEvent,
};

pub struct TransitionsDiscardConfirmModal {
    pub theme: RevoTheme,
    pub changed_fields: usize,
}

impl TransitionsDiscardConfirmModal {
    pub fn answer(&self, discard: bool) -> ModalEvent {
        ModalEvent::ConfirmChoice(discard)
    }
}

impl Render for TransitionsDiscardConfirmModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        modal_shell(
            &ModalConfig::confirmation("Unsaved changes", "Discard"),
            self.theme,
            vec![SharedString::from(format!(
                "Discard {} changed transition field(s)?",
                self.changed_fields
            ))],
        )
    }
}
