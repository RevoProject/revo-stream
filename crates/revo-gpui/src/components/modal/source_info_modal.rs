use gpui::{Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig, ModalSize},
    theme::RevoTheme,
};

pub struct SourceInfoModal {
    pub theme: RevoTheme,
    pub name: String,
    pub source_type: String,
    pub details: Vec<(String, String)>,
}

impl Render for SourceInfoModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let lines = std::iter::once(SharedString::from(format!(
            "{} ({})",
            self.name, self.source_type
        )))
        .chain(
            self.details
                .iter()
                .map(|(key, value)| SharedString::from(format!("{key}: {value}"))),
        );
        modal_shell(
            &ModalConfig::standard("Source details", ModalSize::Medium),
            self.theme,
            lines,
        )
    }
}
