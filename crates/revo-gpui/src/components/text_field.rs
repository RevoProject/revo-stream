//! Shared single-line keystroke handling for native text fields.
//!
//! GPUI 0.2.2 has no first-class text input element, so focused fields handle
//! `KeyDownEvent` themselves. This pure function is the canonical policy:
//! printable text appends, backspace pops, escape clears, and modifier combos
//! or navigation keys leave the value untouched.

/// Applies one keystroke to a single-line text value, returning the new value
/// when it changed.
pub fn apply_line_keystroke(
    text: &str,
    key: &str,
    key_char: Option<&str>,
    control: bool,
    alt: bool,
    platform: bool,
) -> Option<String> {
    if control || alt || platform {
        return None;
    }
    match key {
        "backspace" => {
            let mut next = text.to_owned();
            next.pop();
            Some(next)
        }
        "escape" => Some(String::new()),
        "enter" | "tab" | "up" | "down" | "left" | "right" | "home" | "end" | "pageup"
        | "pagedown" => None,
        _ => match key_char {
            Some(chars) if !chars.chars().any(char::is_control) => {
                let mut next = text.to_owned();
                next.push_str(chars);
                Some(next)
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_text_appends_and_backspace_pops() {
        assert_eq!(
            apply_line_keystroke("", "a", Some("a"), false, false, false),
            Some("a".into())
        );
        assert_eq!(
            apply_line_keystroke("ab", "backspace", None, false, false, false),
            Some("a".into())
        );
        assert_eq!(
            apply_line_keystroke("", "escape", None, false, false, false),
            Some(String::new())
        );
    }

    #[test]
    fn modifier_combos_and_navigation_are_ignored() {
        assert_eq!(
            apply_line_keystroke("keep", "c", Some("c"), true, false, false),
            None
        );
        assert_eq!(
            apply_line_keystroke("keep", "left", None, false, false, false),
            None
        );
        assert_eq!(
            apply_line_keystroke("keep", "enter", None, false, false, false),
            None
        );
    }
}
