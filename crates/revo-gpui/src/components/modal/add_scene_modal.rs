use gpui::{Context, IntoElement, Render, Window};

use super::{draft_line, TextDraft};
use crate::{
    components::modal_shell::{modal_shell, ModalConfig, ModalSize},
    theme::RevoTheme,
};

pub struct AddSceneModal {
    pub theme: RevoTheme,
    pub name: TextDraft,
}

impl AddSceneModal {
    pub fn confirm(&self) -> Option<String> {
        self.name.normalized()
    }
}

impl Render for AddSceneModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        modal_shell(
            &ModalConfig::standard("Add Scene", ModalSize::Small),
            self.theme,
            vec![draft_line("Scene name", &self.name.value)],
        )
    }
}
