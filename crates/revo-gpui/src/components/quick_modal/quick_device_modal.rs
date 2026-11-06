use gpui::{Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig},
    theme::RevoTheme,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceOption {
    pub value: String,
    pub label: String,
}

pub struct QuickDeviceModal {
    pub theme: RevoTheme,
    pub value: String,
    pub options: Vec<DeviceOption>,
    pub monitoring: Option<String>,
}

impl QuickDeviceModal {
    pub fn select(&mut self, value: &str) -> bool {
        if !self.options.iter().any(|option| option.value == value) {
            return false;
        }
        self.value = value.to_owned();
        true
    }
}

impl Render for QuickDeviceModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut lines = vec![SharedString::from(format!(
            "Selected device: {}",
            self.value
        ))];
        lines.extend(
            self.options
                .iter()
                .map(|option| SharedString::from(option.label.clone())),
        );
        if let Some(monitoring) = &self.monitoring {
            lines.push(SharedString::from(format!("Monitoring: {monitoring}")));
        }
        modal_shell(&ModalConfig::quick("Select device"), self.theme, lines)
    }
}
