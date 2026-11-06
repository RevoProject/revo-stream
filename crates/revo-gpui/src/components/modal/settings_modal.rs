use gpui::{Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig, ModalSize},
    theme::RevoTheme,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSection {
    General,
    Look,
    Broadcast,
    Output,
    Audio,
    Video,
    Shortcuts,
    Accessibility,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputTab {
    Streaming,
    Recording,
    Audio,
}

pub struct SettingsModal {
    pub theme: RevoTheme,
    pub active: SettingsSection,
    pub output_tab: OutputTab,
    pub dirty: bool,
}

impl Render for SettingsModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        modal_shell(
            &ModalConfig::standard("Settings", ModalSize::Workspace),
            self.theme,
            vec![
                SharedString::from(
                    "Sections: General | Look | Broadcast | Output | Audio | Video | Shortcuts | Accessibility | Other",
                ),
                SharedString::from("Output tabs: Streaming | Recording | Audio"),
                SharedString::from(format!(
                    "Active section: {:?}; output tab: {:?}; unsaved changes: {}",
                    self.active, self.output_tab, self.dirty
                )),
                SharedString::from(
                    "Structural native settings; device enumeration and persistence backend are not connected.",
                ),
            ],
        )
    }
}
