use gpui::{Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig},
    theme::RevoTheme,
};

pub struct QuickTextEditModal {
    pub theme: RevoTheme,
    pub value: String,
}

impl QuickTextEditModal {
    pub fn confirm(&self) -> String {
        self.value.clone()
    }
}

impl Render for QuickTextEditModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        modal_shell(
            &ModalConfig::quick("Text Edit"),
            self.theme,
            vec![SharedString::from(self.value.clone())],
        )
    }
}
