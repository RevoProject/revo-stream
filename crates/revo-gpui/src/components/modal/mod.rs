use gpui::SharedString;

macro_rules! structural_modal {
    ($name:ident, $title:literal, $size:ident, [$($line:literal),+ $(,)?]) => {
        pub struct $name {
            pub theme: crate::theme::RevoTheme,
        }

        impl gpui::Render for $name {
            fn render(
                &mut self,
                _window: &mut gpui::Window,
                _cx: &mut gpui::Context<'_, Self>,
            ) -> impl gpui::IntoElement {
                let config = crate::components::modal_shell::ModalConfig::standard(
                    $title,
                    crate::components::modal_shell::ModalSize::$size,
                );
                crate::components::modal_shell::modal_shell(
                    &config,
                    self.theme,
                    vec![$(gpui::SharedString::from($line)),+],
                )
            }
        }
    };
}

mod add_scene_modal;
mod add_source_modal;
mod browser_interaction_modal;
mod confirm_modal;
mod debug_console_modal;
mod edit_source_modal;
mod graphic_planner_modal;
mod history_modal;
mod plugins_modal;
pub mod plugins_state;
mod preview_modal;
mod settings_modal;
mod settings_state;
mod source_info_modal;
mod source_instance_modal;
mod source_properties_modal;

pub use add_scene_modal::AddSceneModal;
pub use add_source_modal::AddSourceModal;
pub use browser_interaction_modal::BrowserInteractionModal;
pub use confirm_modal::ConfirmModal;
pub use debug_console_modal::DebugConsoleModal;
pub use edit_source_modal::{EditSourceModal, SourceEditorTab};
pub use graphic_planner_modal::GraphicPlannerModal;
pub use history_modal::{HistoryEntry, HistoryModal};
pub use plugins_modal::PluginsModal;
pub use plugins_state::{PluginsDraft, PluginsEvent};
pub use preview_modal::PreviewModal;
pub use settings_modal::SettingsModal;
pub use settings_state::{OutputTab, SettingsDraft, SettingsEvent, SettingsSection, TextField};
pub use source_info_modal::SourceInfoModal;
pub use source_instance_modal::{SourceInstanceModal, SourceInstanceOption};
pub use source_properties_modal::SourcePropertiesModal;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextDraft {
    pub initial: String,
    pub value: String,
}

impl TextDraft {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            initial: value.clone(),
            value,
        }
    }

    pub fn normalized(&self) -> Option<String> {
        let value = self.value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    }

    pub fn is_dirty(&self) -> bool {
        self.value != self.initial
    }
}

pub(crate) fn draft_line(label: &str, value: &str) -> SharedString {
    SharedString::from(format!("{label}: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_draft_trims_and_rejects_empty_input() {
        let draft = TextDraft {
            initial: String::new(),
            value: "  Scene A  ".into(),
        };
        assert_eq!(draft.normalized().as_deref(), Some("Scene A"));
        let empty = TextDraft {
            initial: String::new(),
            value: "   ".into(),
        };
        assert_eq!(empty.normalized(), None);
    }
}
