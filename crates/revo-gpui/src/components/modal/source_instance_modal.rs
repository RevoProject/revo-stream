use gpui::{Context, IntoElement, Render, SharedString, Window};

use super::TextDraft;
use crate::{
    components::modal_shell::{modal_shell, ModalConfig, ModalSize},
    theme::RevoTheme,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceInstanceOption {
    pub id: String,
    pub name: String,
}

pub struct SourceInstanceModal {
    pub theme: RevoTheme,
    pub source_type_label: String,
    pub existing: Vec<SourceInstanceOption>,
    pub selected_id: Option<String>,
    pub new_name: TextDraft,
}

impl SourceInstanceModal {
    pub fn select_existing(&mut self, id: &str) -> bool {
        if !self.existing.iter().any(|item| item.id == id) {
            return false;
        }
        self.selected_id = Some(id.to_owned());
        true
    }
}

impl Render for SourceInstanceModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut lines = vec![SharedString::from(
            "Select existing source or create a new one",
        )];
        lines.extend(
            self.existing
                .iter()
                .map(|item| SharedString::from(item.name.clone())),
        );
        lines.push(SharedString::from(format!(
            "New source name: {}",
            self.new_name.value
        )));
        modal_shell(
            &ModalConfig::standard(
                format!("Add Source - {}", self.source_type_label),
                ModalSize::Large,
            ),
            self.theme,
            lines,
        )
    }
}
