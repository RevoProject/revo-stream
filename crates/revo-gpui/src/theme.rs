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
    /// Keyboard focus indicator (plan §10): accent normally, white when
    /// "stronger keyboard focus indicators" is enabled.
    pub focus_ring: Rgba,
}

/// Settings > Accessibility options (plan §10). Stored as strings in the
/// `ui_profile`; the workspace reads them after a settings load and folds them
/// into the palette so they take effect without a restart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Accessibility {
    pub high_contrast: bool,
    pub reduce_motion: bool,
    pub strong_focus: bool,
    /// Font-size choice index (`FONT_SIZES` in the settings modal).
    pub font_size: FontSize,
    pub font_family: FontFamily,
    pub color_vision: ColorVisionMode,
}

impl Default for Accessibility {
    fn default() -> Self {
        Self {
            high_contrast: false,
            reduce_motion: false,
            strong_focus: false,
            font_size: FontSize::Default,
            font_family: FontFamily::Default,
            color_vision: ColorVisionMode::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontSize {
    Small,
    Default,
    Large,
    ExtraLarge,
}

impl FontSize {
    /// Settings-modal order: Small / Default / Large / Extra large.
    pub const ALL: [Self; 4] = [Self::Small, Self::Default, Self::Large, Self::ExtraLarge];

    pub fn from_profile(value: Option<&str>) -> Self {
        match value
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "small" => Self::Small,
            "large" => Self::Large,
            "extra large" | "extra_large" | "xlarge" => Self::ExtraLarge,
            _ => Self::Default,
        }
    }

    /// Multiplier applied to the base text size (1.0 for the default).
    pub fn scale(self) -> f32 {
        match self {
            Self::Small => 0.9,
            Self::Default => 1.0,
            Self::Large => 1.15,
            Self::ExtraLarge => 1.3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontFamily {
    /// WCAG-friendly default stack offered by the settings modal.
    Default,
    Serif,
    Monospace,
}

impl FontFamily {
    pub fn from_profile(value: Option<&str>) -> Self {
        match value
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "serif" | "szeryfowy" => Self::Serif,
            "monospace" | "mono" => Self::Monospace,
            _ => Self::Default,
        }
    }

    /// gpui font-family name; the platform falls back when it is missing.
    pub fn gpui_family(self) -> &'static str {
        match self {
            Self::Default => "Zed Plex Sans",
            Self::Serif => "Zed Plex Serif",
            Self::Monospace => "Zed Plex Mono",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorVisionMode {
    None,
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

impl ColorVisionMode {
    pub fn from_profile(value: Option<&str>) -> Self {
        match value
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '-'], "_")
            .as_str()
        {
            "protanopia" => Self::Protanopia,
            "deuteranopia" => Self::Deuteranopia,
            "tritanopia" => Self::Tritanopia,
            _ => Self::None,
        }
    }

    /// 3x3 RGB mixing matrix for the mode (row-major).
    fn matrix(self) -> Option<[f32; 9]> {
        match self {
            Self::None => None,
            // Machado et al. severity-1.0 approximations.
            Self::Protanopia => Some([0.567, 0.433, 0.0, 0.558, 0.442, 0.0, 0.0, 0.242, 0.758]),
            Self::Deuteranopia => Some([0.625, 0.375, 0.0, 0.700, 0.300, 0.0, 0.0, 0.300, 0.700]),
            Self::Tritanopia => Some([0.950, 0.050, 0.0, 0.000, 0.433, 0.567, 0.000, 0.475, 0.525]),
        }
    }

    fn apply(self, color: Rgba) -> Rgba {
        let Some([r0, r1, r2, g0, g1, g2, b0, b1, b2]) = self.matrix() else {
            return color;
        };
        let (r, g, b) = (color.r, color.g, color.b);
        Rgba {
            r: (r0 * r + r1 * g + r2 * b).clamp(0.0, 1.0),
            g: (g0 * r + g1 * g + g2 * b).clamp(0.0, 1.0),
            b: (b0 * r + b1 * g + b2 * b).clamp(0.0, 1.0),
            a: color.a,
        }
    }
}

/// WCAG 2.1 relative-luminance contrast ratio (1.0 = identical, 21.0 = black on
/// white). `None` for fully transparent inputs, which have no defined ratio.
pub fn contrast_ratio(a: Rgba, b: Rgba) -> Option<f32> {
    if a.a == 0.0 || b.a == 0.0 {
        return None;
    }
    let luminance = |color: Rgba| {
        let channel = |value: f32| {
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
    };
    let (lighter, darker) = {
        let (la, lb) = (luminance(a), luminance(b));
        if la >= lb {
            (la, lb)
        } else {
            (lb, la)
        }
    };
    Some((lighter + 0.05) / (darker + 0.05))
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
            focus_ring: rgb(0x5b7cfa),
        }
    }

    /// High-contrast palette (plan §10): near-black surfaces, near-white text,
    /// stronger borders. Keeps the same semantic slots so every view keeps
    /// working unchanged.
    pub fn high_contrast(kind: ThemeKind) -> Self {
        let accent_family = match kind {
            ThemeKind::RevoClassic => (rgb(0x9ec1ff), rgb(0xc0d6ff), rgb(0x7aa5f5)),
            ThemeKind::RevoFuture => (rgb(0x8fb0ff), rgb(0xb4cbff), rgb(0x6f95f0)),
        };
        Self {
            background: rgb(0x000000),
            header: rgb(0x000000),
            surface: rgb(0x0a0a0a),
            surface_elevated: rgb(0x141414),
            surface_active: rgb(0x242424),
            sidebar: rgb(0x000000),
            surface_selected: rgb(0x2e2e2e),
            border_subtle: rgb(0x5c5c5c),
            border: rgb(0x8a8a8a),
            border_strong: rgb(0xc4c4c4),
            border_selected: rgb(0xffffff),
            text_strong: rgb(0xffffff),
            text: rgb(0xffffff),
            text_secondary: rgb(0xf2f2f2),
            text_muted: rgb(0xd6d6d6),
            text_disabled: rgb(0xa8a8a8),
            accent: accent_family.0,
            accent_hover: accent_family.1,
            accent_pressed: accent_family.2,
            danger: rgb(0xff8a8a),
            danger_hover: rgb(0xffb0b0),
            danger_pressed: rgb(0xe06060),
            success: rgb(0x6ee7a0),
            success_hover: rgb(0x9af0bd),
            success_active: rgb(0x3fae74),
            warning: rgb(0xffcf6b),
            overlay: rgba(0x000000d0),
            focus_ring: rgb(0xffffff),
        }
    }

    /// Applies the accessibility profile to a base palette (plan §10): high
    /// contrast, color-vision-safe semantic accents, and a dedicated keyboard
    /// focus ring.
    pub fn with_accessibility(self, kind: ThemeKind, a11y: Accessibility) -> Self {
        // High contrast swaps in the per-theme high-contrast palette, whose
        // accents are brightened for near-black surfaces.
        let mut theme = if a11y.high_contrast {
            Self::high_contrast(kind)
        } else {
            self
        };
        let mode = a11y.color_vision;
        theme.accent = mode.apply(theme.accent);
        theme.accent_hover = mode.apply(theme.accent_hover);
        theme.accent_pressed = mode.apply(theme.accent_pressed);
        theme.danger = mode.apply(theme.danger);
        theme.danger_hover = mode.apply(theme.danger_hover);
        theme.danger_pressed = mode.apply(theme.danger_pressed);
        theme.success = mode.apply(theme.success);
        theme.success_hover = mode.apply(theme.success_hover);
        theme.success_active = mode.apply(theme.success_active);
        theme.warning = mode.apply(theme.warning);
        // A keyboard focus ring must stay visible on every surface: pure white
        // normally, the accent in high contrast (already near-white).
        theme.focus_ring = if a11y.high_contrast {
            rgb(0xffffff)
        } else if a11y.strong_focus {
            rgb(0xffffff)
        } else {
            theme.accent
        };
        theme
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
            focus_ring: rgb(0x285cfc),
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

    /// Every text colour must stay readable on every surface it can sit on
    /// (WCAG AA: 4.5:1 for body text).
    fn assert_text_contrast(theme: &RevoTheme, label: &str) {
        let surfaces = [
            ("background", theme.background),
            ("surface", theme.surface),
            ("surface_elevated", theme.surface_elevated),
            ("surface_active", theme.surface_active),
            ("surface_selected", theme.surface_selected),
        ];
        let texts = [
            ("text_strong", theme.text_strong),
            ("text", theme.text),
            ("text_secondary", theme.text_secondary),
            ("text_muted", theme.text_muted),
        ];
        for (surface_name, surface) in surfaces {
            for (text_name, text) in texts {
                let ratio = contrast_ratio(text, surface).unwrap_or(0.0);
                assert!(
                    ratio >= 4.5,
                    "{label}: {text_name} on {surface_name} is {ratio:.2}:1 (needs 4.5:1)"
                );
            }
        }
    }

    #[test]
    fn every_palette_meets_wcag_text_contrast() {
        for (label, theme) in [
            ("classic", RevoTheme::classic()),
            ("future", RevoTheme::future()),
            (
                "classic high-contrast",
                RevoTheme::high_contrast(ThemeKind::RevoClassic),
            ),
            (
                "future high-contrast",
                RevoTheme::high_contrast(ThemeKind::RevoFuture),
            ),
        ] {
            assert_text_contrast(&theme, label);
        }
    }

    #[test]
    fn high_contrast_keeps_the_semantic_slots_and_darkens_surfaces() {
        let base = RevoTheme::classic();
        let high = RevoTheme::high_contrast(ThemeKind::RevoClassic);
        // Semantic slots survive so existing views keep working, brightened
        // enough to stay readable on near-black surfaces.
        assert!(contrast_ratio(high.accent, high.background).unwrap() >= 7.0);
        assert!(contrast_ratio(high.danger, high.background).unwrap() >= 7.0);
        assert!(contrast_ratio(high.success, high.background).unwrap() >= 7.0);
        // The danger family keeps its hue but is brightened for contrast.
        assert!(high.danger.r > base.danger.r);
        // Surfaces get darker and text brighter.
        assert!(contrast_ratio(high.background, base.background).unwrap() > 1.0);
        assert_eq!(high.text, rgb(0xffffff));
        assert_eq!(high.border_selected, rgb(0xffffff));
    }

    #[test]
    fn color_vision_modes_map_semantic_colors_deterministically() {
        let base = RevoTheme::classic();
        let protan = base.with_accessibility(
            ThemeKind::RevoClassic,
            Accessibility {
                color_vision: ColorVisionMode::Protanopia,
                ..Accessibility::default()
            },
        );
        // Red loses intensity under protanopia; the mapping is stable.
        assert!(protan.danger.r < base.danger.r);
        assert_eq!(
            protan.danger,
            base.with_accessibility(
                ThemeKind::RevoClassic,
                Accessibility {
                    color_vision: ColorVisionMode::Protanopia,
                    ..Accessibility::default()
                },
            )
            .danger
        );
        // Tritanopia leaves red nearly untouched.
        let tritan = base.with_accessibility(
            ThemeKind::RevoClassic,
            Accessibility {
                color_vision: ColorVisionMode::Tritanopia,
                ..Accessibility::default()
            },
        );
        assert!((tritan.danger.r - base.danger.r).abs() < 0.06);
    }

    #[test]
    fn accessibility_flags_compose_without_losing_contrast() {
        let theme = RevoTheme::future().with_accessibility(
            ThemeKind::RevoFuture,
            Accessibility {
                high_contrast: true,
                strong_focus: true,
                reduce_motion: true,
                color_vision: ColorVisionMode::Deuteranopia,
                ..Accessibility::default()
            },
        );
        assert_text_contrast(&theme, "future high-contrast + deuteranopia");
        assert_eq!(theme.focus_ring, rgb(0xffffff));
    }

    #[test]
    fn accessibility_profile_parsers_are_forgiving() {
        assert_eq!(FontSize::from_profile(Some("Large")), FontSize::Large);
        assert_eq!(FontSize::from_profile(Some("nonsense")), FontSize::Default);
        assert_eq!(FontSize::from_profile(None), FontSize::Default);
        assert_eq!(FontSize::ExtraLarge.scale(), 1.3);
        assert_eq!(FontFamily::from_profile(Some("Serif")), FontFamily::Serif);
        assert_eq!(
            ColorVisionMode::from_profile(Some("Protanopia")),
            ColorVisionMode::Protanopia
        );
        assert_eq!(
            ColorVisionMode::from_profile(Some("deuteranopia")),
            ColorVisionMode::Deuteranopia
        );
        assert_eq!(ColorVisionMode::from_profile(None), ColorVisionMode::None);
        // Focus ring defaults to the accent when no flag is set.
        let theme = RevoTheme::classic()
            .with_accessibility(ThemeKind::RevoClassic, Accessibility::default());
        assert_eq!(theme.focus_ring, RevoTheme::classic().accent);
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
