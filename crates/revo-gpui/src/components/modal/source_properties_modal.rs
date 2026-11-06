use gpui::{Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig, ModalSize},
    theme::RevoTheme,
};

#[derive(Clone, Copy, Debug)]
pub struct SourcePropertiesModal {
    pub theme: RevoTheme,
    pub width: f32,
    pub height: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub position_x: f32,
    pub position_y: f32,
}

impl SourcePropertiesModal {
    pub fn normalize(&mut self) {
        self.width = self.width.max(1.0);
        self.height = self.height.max(1.0);
        self.scale_x = self.scale_x.max(0.01);
        self.scale_y = self.scale_y.max(0.01);
    }
}

impl Render for SourcePropertiesModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        modal_shell(
            &ModalConfig::standard("Properties", ModalSize::Medium),
            self.theme,
            vec![
                SharedString::from(format!("Size: {} x {}", self.width, self.height)),
                SharedString::from(format!("Scale: {} x {}", self.scale_x, self.scale_y)),
                SharedString::from(format!(
                    "Position: {}, {}",
                    self.position_x, self.position_y
                )),
                SharedString::from("Open the full source editor"),
            ],
        )
    }
}
