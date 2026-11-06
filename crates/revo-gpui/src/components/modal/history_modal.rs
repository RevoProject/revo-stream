use gpui::{Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig, ModalSize},
    theme::RevoTheme,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub label: String,
    pub redo: bool,
}

pub struct HistoryModal {
    pub theme: RevoTheme,
    pub entries: Vec<HistoryEntry>,
    pub selected: Option<usize>,
}

impl HistoryModal {
    pub fn restore(&mut self, index: usize) -> Option<usize> {
        (index < self.entries.len()).then(|| {
            self.selected = Some(index);
            index
        })
    }
}

impl Render for HistoryModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let lines = self.entries.iter().map(|entry| {
            SharedString::from(format!(
                "{}: {}",
                if entry.redo { "Redo" } else { "Undo" },
                entry.label
            ))
        });
        modal_shell(
            &ModalConfig::standard("Action history", ModalSize::Medium),
            self.theme,
            lines,
        )
    }
}
