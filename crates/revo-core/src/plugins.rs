//! Pure plugin naming and plugin-profile module selection helpers.

use std::collections::{HashMap, HashSet};

/// Title-cases each whitespace-separated word, uppercasing only the first
/// ASCII character of every word and preserving the remaining characters.
pub fn title_case_words(input: &str) -> String {
    input
        .split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// Derives a human-readable display label from a libobs module file stem,
/// for example `obs-websocket` becomes `Websocket`.
pub fn plugin_display_name_from_module(module_name: &str) -> String {
    let base = module_name.trim_start_matches("obs-");
    let words = base.replace(['-', '_'], " ");
    title_case_words(words.trim())
}

/// Filters requested plugin modules against the modules actually available:
/// trims entries, drops empties, keeps exact matches, maps sanitized aliases
/// onto available modules, then sorts and deduplicates.
pub fn normalize_enabled_modules(
    requested: Vec<String>,
    available: &HashSet<String>,
) -> Vec<String> {
    let normalized_to_available: HashMap<String, String> = available
        .iter()
        .map(|m| (crate::settings::sanitize_profile_name(m), m.clone()))
        .collect();

    let mut modules: Vec<String> = requested
        .into_iter()
        .filter_map(|n| {
            let trimmed = n.trim().to_string();
            if trimmed.is_empty() {
                return None;
            }
            if available.contains(&trimmed) {
                return Some(trimmed);
            }
            let normalized = crate::settings::sanitize_profile_name(&trimmed);
            if normalized.is_empty() {
                return None;
            }
            normalized_to_available.get(&normalized).cloned()
        })
        .collect();
    modules.sort();
    modules.dedup();
    modules
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn title_case_words_uppercases_first_letters_only() {
        assert_eq!(title_case_words("hello world"), "Hello World");
        assert_eq!(title_case_words("x264 encoder"), "X264 Encoder");
        assert_eq!(title_case_words("  spaced   out  "), "Spaced Out");
        assert_eq!(title_case_words(""), "");
    }

    #[test]
    fn plugin_display_names_match_current_behavior() {
        assert_eq!(
            plugin_display_name_from_module("obs-websocket"),
            "Websocket"
        );
        assert_eq!(
            plugin_display_name_from_module("vlc-video-source"),
            "Vlc Video Source"
        );
        assert_eq!(plugin_display_name_from_module("obs_x264"), "Obs X264");
        // Leading whitespace prevents the "obs-" prefix from being stripped;
        // this quirk is preserved intentionally.
        assert_eq!(
            plugin_display_name_from_module("  obs-win-dshow  "),
            "Obs Win Dshow"
        );
        assert_eq!(plugin_display_name_from_module("win-dshow"), "Win Dshow");
    }

    #[test]
    fn normalize_enabled_modules_matches_current_behavior() {
        let mut available = HashSet::new();
        available.insert("obs_x264".to_string());
        available.insert("win-dshow".to_string());

        let normalized = normalize_enabled_modules(
            vec![
                "  obs_x264  ".to_string(),
                "obs x264".to_string(),
                "".to_string(),
                "   ".to_string(),
                "win-dshow".to_string(),
                "totally-unknown".to_string(),
            ],
            &available,
        );

        assert_eq!(
            normalized,
            vec!["obs_x264".to_string(), "win-dshow".to_string()]
        );
    }

    #[test]
    fn normalize_enabled_modules_sorts_and_deduplicates() {
        let mut available = HashSet::new();
        available.insert("b_mod".to_string());
        available.insert("a_mod".to_string());

        let normalized = normalize_enabled_modules(
            vec![
                "b_mod".to_string(),
                "a_mod".to_string(),
                "b mod".to_string(),
            ],
            &available,
        );

        assert_eq!(normalized, vec!["a_mod".to_string(), "b_mod".to_string()]);
    }

    #[test]
    fn window_capabilities_keep_wire_shape() {
        let capabilities = crate::models::PluginWindowCapabilities {
            can_list: false,
            can_open: false,
            reason: "without an OBS Qt frontend bridge".into(),
        };

        assert_eq!(
            serde_json::to_value(capabilities).expect("capabilities should serialize"),
            json!({
                "can_list": false,
                "can_open": false,
                "reason": "without an OBS Qt frontend bridge"
            })
        );
    }
}
