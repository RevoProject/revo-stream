use gpui::{Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig, ModalSize},
    theme::RevoTheme,
};

pub struct AddSourceModal {
    pub theme: RevoTheme,
    pub source_types: Vec<String>,
    pub selected_type: Option<String>,
}

impl AddSourceModal {
    pub fn select(&mut self, source_type: &str) -> bool {
        if !self.source_types.iter().any(|item| item == source_type) {
            return false;
        }
        self.selected_type = Some(source_type.to_owned());
        true
    }

    pub fn confirm(&self) -> Option<String> {
        self.selected_type.clone()
    }
}

impl Render for AddSourceModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut lines = vec![SharedString::from("Choose a native OBS source type")];
        lines.extend(self.source_types.iter().cloned().map(SharedString::from));
        lines.push(SharedString::from("External source types"));
        modal_shell(
            &ModalConfig::standard("Add Source", ModalSize::Large),
            self.theme,
            lines,
        )
    }
}
