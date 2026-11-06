use gpui::{Context, IntoElement, Render, SharedString, Window};

use crate::{
    components::modal_shell::{modal_shell, ModalConfig},
    theme::RevoTheme,
};

pub struct QuickColorModal {
    pub theme: RevoTheme,
    pub value: String,
    pub recent: Vec<String>,
}

impl QuickColorModal {
    pub fn normalized_color(&self) -> Option<String> {
        let value = self.value.trim();
        let hex = value.strip_prefix('#').unwrap_or(value);
        ((hex.len() == 6 || hex.len() == 8) && hex.chars().all(|ch| ch.is_ascii_hexdigit()))
            .then(|| format!("#{}", hex.to_ascii_uppercase()))
    }
}

impl Render for QuickColorModal {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut lines = vec![SharedString::from(format!("Color: {}", self.value))];
        lines.extend(
            self.recent
                .iter()
                .map(|color| SharedString::from(format!("Recent: {color}"))),
        );
        modal_shell(&ModalConfig::quick("Change color"), self.theme, lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_draft_is_normalized() {
        let modal = QuickColorModal {
            theme: RevoTheme::classic(),
            value: " 285cfc ".into(),
            recent: vec![],
        };
        assert_eq!(modal.normalized_color().as_deref(), Some("#285CFC"));
    }
}
