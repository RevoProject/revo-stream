//! Default naming for freshly created scenes and sources, mirroring the
//! placeholder scheme the user asked for: `scene-{nr}` and
//! `source-{type}-{nr}`. Numbers continue after the highest existing suffix
//! so a default name never collides with an existing one.

/// Returns the next free number for `prefix`-numbered names among
/// `existing`: one past the highest parsed suffix, or `1` when none match.
fn next_number(existing: &[String], prefix: &str) -> u32 {
    let mut max = 0u32;
    for name in existing {
        if let Some(suffix) = name.strip_prefix(prefix) {
            if !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()) {
                if let Ok(number) = suffix.parse::<u32>() {
                    max = max.max(number);
                }
            }
        }
    }
    max.saturating_add(1)
}

/// Default source name: `source-{type}-{nr}` with the first free `{nr}`.
pub fn default_source_name(existing: &[String], source_type: &str) -> String {
    let prefix = format!("source-{source_type}-");
    format!("{prefix}{}", next_number(existing, &prefix))
}

/// Default scene name: `scene-{nr}` with the first free `{nr}`.
pub fn default_scene_name(existing: &[String]) -> String {
    let prefix = "scene-";
    format!("{prefix}{}", next_number(existing, prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<String> {
        vec![
            "source-color_source-1".into(),
            "source-color_source-3".into(),
            "scene-1".into(),
            "My Camera".into(),
        ]
    }

    #[test]
    fn source_default_continues_after_highest_suffix() {
        assert_eq!(
            default_source_name(&names(), "color_source"),
            "source-color_source-4"
        );
        assert_eq!(
            default_source_name(&names(), "browser_source"),
            "source-browser_source-1"
        );
    }

    #[test]
    fn scene_default_continues_after_highest_suffix() {
        assert_eq!(default_scene_name(&names()), "scene-2");
        assert_eq!(default_scene_name(&[]), "scene-1");
    }

    #[test]
    fn non_numeric_suffixes_are_ignored() {
        let existing = vec!["source-color_source-x".into()];
        assert_eq!(
            default_source_name(&existing, "color_source"),
            "source-color_source-1"
        );
    }

    #[test]
    fn generated_defaults_avoid_existing_duplicates() {
        let existing = vec!["scene-2".into(), "scene-2".into()];
        assert_eq!(default_scene_name(&existing), "scene-3");
    }
}
