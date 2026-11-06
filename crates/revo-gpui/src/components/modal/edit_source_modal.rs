use gpui::{Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig, ModalSize},
    theme::RevoTheme,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceEditorTab {
    General,
    Filters,
    Other,
}

pub struct EditSourceModal {
    pub theme: RevoTheme,
    pub source_name: String,
    pub source_type: String,
    pub active_tab: SourceEditorTab,
    pub dirty: bool,
}

impl Render for EditSourceModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        modal_shell(
            &ModalConfig::standard("Edit Source", ModalSize::Workspace),
            self.theme,
            vec![
                SharedString::from(format!("{} - {}", self.source_name, self.source_type)),
                SharedString::from("Tabs: General | Filters | Other"),
                SharedString::from(format!(
                    "Active tab: {:?}; unsaved changes: {}",
                    self.active_tab, self.dirty
                )),
                SharedString::from(
                    "Structural native editor; preview and OBS property backend are not connected.",
                ),
            ],
        )
    }
}
