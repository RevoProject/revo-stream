use gpui::{rgb, rgba, Rgba};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeKind {
    RevoClassic,
    RevoFuture,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RevoTheme {
    pub background: Rgba,
    pub header: Rgba,
    pub surface: Rgba,
    pub surface_elevated: Rgba,
    pub surface_active: Rgba,
    pub sidebar: Rgba,
    pub surface_selected: Rgba,
    pub border_subtle: Rgba,
    pub border: Rgba,
    pub border_strong: Rgba,
    pub border_selected: Rgba,
    pub text_strong: Rgba,
    pub text: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub text_disabled: Rgba,
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_pressed: Rgba,
    pub danger: Rgba,
    pub danger_hover: Rgba,
    pub danger_pressed: Rgba,
    pub success: Rgba,
    pub success_hover: Rgba,
    /// Calmer success variant for "already active" states (for example the
    /// Go live button while streaming) so long-running active states do not
    /// glow in the bright success accent.
    pub success_active: Rgba,
    pub warning: Rgba,
    pub overlay: Rgba,
}

impl RevoTheme {
    pub fn for_kind(kind: ThemeKind) -> Self {
        match kind {
            ThemeKind::RevoClassic => Self::classic(),
            ThemeKind::RevoFuture => Self::future(),
        }
    }

    pub fn classic() -> Self {
        Self {
            background: rgb(0x0f1115),
            header: rgb(0x151820),
            surface: rgb(0x151820),
            surface_elevated: rgb(0x1b1f2a),
            surface_active: rgb(0x232838),
            sidebar: rgb(0x151820),
            surface_selected: rgb(0x232838),
            border_subtle: rgb(0x232838),
            border: rgb(0x2b3142),
            border_strong: rgb(0x3a4258),
            border_selected: rgb(0x5b7cfa),
            text_strong: rgb(0xffffff),
            text: rgb(0xe5e7eb),
            text_secondary: rgb(0xc8ceda),
            text_muted: rgb(0xa1a1aa),
            text_disabled: rgb(0x6b7280),
            accent: rgb(0x5b7cfa),
            accent_hover: rgb(0x7690ff),
            accent_pressed: rgb(0x3b5bdb),
            danger: rgb(0xef4444),
            danger_hover: rgb(0xf26767),
            danger_pressed: rgb(0xef4444),
            success: rgb(0x22c55e),
            success_hover: rgb(0x34d399),
            success_active: rgb(0x15803d),
            warning: rgb(0xf59e0b),
            overlay: rgba(0x0000009e),
        }
    }

    pub fn future() -> Self {
        Self {
            background: rgb(0x080810),
            header: rgb(0x0a111b),
            surface: rgb(0x0c131b),
            surface_elevated: rgb(0x0f1720),
            surface_active: rgb(0x152238),
            sidebar: rgb(0x0a1017),
            surface_selected: rgb(0x081838),
            border_subtle: rgb(0x182230),
            border: rgb(0x202c3c),
            border_strong: rgb(0x2c3a4e),
            border_selected: rgb(0x1c3a78),
            text_strong: rgb(0xf4f7fc),
            text: rgb(0xe2e8f2),
            text_secondary: rgb(0xb8c2d2),
            text_muted: rgb(0x8b98a9),
            text_disabled: rgb(0x5a6678),
            accent: rgb(0x285cfc),
            accent_hover: rgb(0x4c78fc),
            accent_pressed: rgb(0x2048c8),
            danger: rgb(0xe42c2c),
            danger_hover: rgb(0xf04448),
            danger_pressed: rgb(0xc82028),
            success: rgb(0x20e438),
            success_hover: rgb(0x3cff78),
            success_active: rgb(0x16a34a),
            warning: rgb(0xf0ac50),
            overlay: rgba(0x0000009e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_semantic_values_are_exact() {
        let theme = RevoTheme::classic();
        assert_eq!(theme.background, rgb(0x0f1115));
        assert_eq!(theme.surface_elevated, rgb(0x1b1f2a));
        assert_eq!(theme.border_strong, rgb(0x3a4258));
        assert_eq!(theme.text_secondary, rgb(0xc8ceda));
        assert_eq!(theme.accent_hover, rgb(0x7690ff));
        assert_eq!(theme.danger_hover, rgb(0xf26767));
        assert_eq!(theme.success_hover, rgb(0x34d399));
        assert_eq!(theme.success_active, rgb(0x15803d));
    }

    #[test]
    fn future_semantic_values_are_exact() {
        let theme = RevoTheme::future();
        assert_eq!(theme.background, rgb(0x080810));
        assert_eq!(theme.header, rgb(0x0a111b));
        assert_eq!(theme.surface_selected, rgb(0x081838));
        assert_eq!(theme.border_selected, rgb(0x1c3a78));
        assert_eq!(theme.accent_pressed, rgb(0x2048c8));
        assert_eq!(theme.text_disabled, rgb(0x5a6678));
        assert_eq!(theme.success_active, rgb(0x16a34a));
        assert_eq!(theme.overlay, rgba(0x0000009e));
    }
}
