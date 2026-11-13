//! Caret-aware single-line editing sessions (plan §5.1 / §25.1).
//!
//! GPUI 0.2.2 ships no first-class text input element, so editable fields
//! keep an explicit caret state next to their draft value. This module owns
//! the pure editing rules (insertion at the caret, selection, navigation and
//! deletion) plus the pixel math used to map a click position back onto a
//! caret index. Rendering stays with the hosting modal; only the geometry
//! helper needs a live `Window`.

use gpui::{prelude::*, px, KeyDownEvent, Pixels, Window};
use std::{cell::RefCell, rc::Rc};

/// One editing session for a single-line field: value plus caret and an
/// optional selection anchor (all positions are `char` indices so the model
/// is UTF-8-safe; only `byte_range` converts to GPUI ranges).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaretSession {
    pub value: String,
    pub cursor: usize,
    pub anchor: Option<usize>,
}

/// The modifier flags a session needs from a keystroke.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditModifiers {
    pub control: bool,
    pub alt: bool,
    pub platform: bool,
    pub shift: bool,
}

impl EditModifiers {
    /// True when a non-character chord (Ctrl/Cmd/Alt) is held. Shift alone is
    /// a character modifier and must not block printing.
    pub fn blocks_print(&self) -> bool {
        self.control || self.alt || self.platform
    }
}

impl CaretSession {
    pub fn new(value: String) -> Self {
        Self {
            cursor: value.chars().count(),
            anchor: None,
            value,
        }
    }

    /// Re-seed the session from a draft value change (external mutations such
    /// as Reset defaults or a backend snapshot win over the local caret).
    pub fn sync_value(&mut self, value: String) {
        if value != self.value {
            let len = value.chars().count();
            self.cursor = self.cursor.min(len);
            self.anchor = self.anchor.map(|a| a.min(len));
            self.value = value;
        }
    }

    pub fn len(&self) -> usize {
        self.value.chars().count()
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Normalized `[start, end)` selection in char indices.
    pub fn selection_chars(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        if anchor == self.cursor {
            None
        } else {
            Some((anchor.min(self.cursor), anchor.max(self.cursor)))
        }
    }

    /// The same selection as byte indices for GPUI highlight ranges.
    pub fn selection_bytes(&self) -> Option<(usize, usize)> {
        let (start, end) = self.selection_chars()?;
        Some((self.byte_of(start)?, self.byte_of(end)?))
    }

    pub fn byte_of(&self, char_index: usize) -> Option<usize> {
        if char_index == 0 {
            return Some(0);
        }
        let mut bytes = 0usize;
        for (i, ch) in self.value.char_indices() {
            if i == char_index {
                return Some(bytes);
            }
            bytes += ch.len_utf8();
        }
        if char_index == self.len() {
            Some(self.value.len())
        } else {
            None
        }
    }

    /// Word boundary helper for Ctrl+Backspace / Alt+Backspace deletion.
    fn word_edge(&self, mut index: usize, left: bool) -> usize {
        let chars: Vec<char> = self.value.chars().collect();
        if index > chars.len() {
            index = chars.len();
        }
        if left {
            while index > 0 && chars[index - 1].is_whitespace() {
                index -= 1;
            }
            while index > 0 && !chars[index - 1].is_whitespace() {
                index -= 1;
            }
        } else {
            while index < chars.len() && chars[index].is_whitespace() {
                index += 1;
            }
            while index < chars.len() && !chars[index].is_whitespace() {
                index += 1;
            }
        }
        index
    }

    fn replace_range(&mut self, start: usize, end: usize, text: &str) {
        let (sb, eb) = match (self.byte_of(start), self.byte_of(end)) {
            (Some(s), Some(e)) => (s, e),
            _ => return,
        };
        self.value.replace_range(sb..eb, text);
        self.cursor = start + text.chars().count();
        self.anchor = None;
    }

    fn delete_selection_or(&mut self, backwards: bool, word: bool) {
        if let Some((start, end)) = self.selection_chars() {
            self.replace_range(start, end, "");
            return;
        }
        if backwards {
            if self.cursor == 0 {
                return;
            }
            let start = if word {
                self.word_edge(self.cursor, true)
            } else {
                self.cursor - 1
            };
            self.replace_range(start, self.cursor, "");
        } else {
            let len = self.len();
            if self.cursor >= len {
                return;
            }
            let end = if word {
                self.word_edge(self.cursor, false)
            } else {
                self.cursor + 1
            };
            self.replace_range(self.cursor, end, "");
        }
    }

    /// Applies one keystroke; returns `true` when the value changed (the host
    /// persists it into the draft). Navigation and selection return `false`.
    pub fn apply_keystroke(
        &mut self,
        key: &str,
        key_char: Option<&str>,
        mods: EditModifiers,
    ) -> bool {
        let word_chord = mods.control || mods.alt;
        match key {
            "escape" => {
                if self.selection_chars().is_some() {
                    self.anchor = None;
                    return false;
                }
                let had_value = !self.value.is_empty();
                self.value.clear();
                self.cursor = 0;
                self.anchor = None;
                had_value
            }
            "backspace" => {
                self.delete_selection_or(true, word_chord);
                true
            }
            "delete" => {
                self.delete_selection_or(false, word_chord);
                true
            }
            "left" if word_chord => {
                let target = self.word_edge(self.cursor, true);
                self.shift_cursor(target as isize - self.cursor as isize, mods.shift, target);
                false
            }
            "right" if word_chord => {
                let target = self.word_edge(self.cursor, false);
                self.shift_cursor(target as isize - self.cursor as isize, mods.shift, target);
                false
            }
            "left" => {
                let start = self
                    .selection_chars()
                    .map(|(s, _)| s)
                    .unwrap_or(self.cursor);
                self.shift_cursor(-1, mods.shift, start);
                false
            }
            "right" => {
                let end = self
                    .selection_chars()
                    .map(|(_, e)| e)
                    .unwrap_or(self.cursor);
                self.shift_cursor(1, mods.shift, end);
                false
            }
            "home" => {
                if mods.shift {
                    self.anchor = Some(self.anchor.unwrap_or(self.cursor));
                } else {
                    self.anchor = None;
                }
                self.cursor = 0;
                false
            }
            "end" => {
                if mods.shift {
                    self.anchor = Some(self.anchor.unwrap_or(self.cursor));
                } else {
                    self.anchor = None;
                }
                self.cursor = self.len();
                false
            }
            "a" if mods.control && !mods.alt && !mods.platform => {
                self.anchor = Some(0);
                self.cursor = self.len();
                false
            }
            _ if mods.blocks_print() => false,
            _ => match key_char {
                Some(text) if !text.is_empty() && !text.chars().any(char::is_control) => {
                    let (start, end) = match self.selection_chars() {
                        Some((s, e)) => (s, e),
                        None => (self.cursor, self.cursor),
                    };
                    self.replace_range(start, end, text);
                    true
                }
                _ => false,
            },
        }
    }

    /// Places the caret at `index` (clamped); `select` keeps/extends an
    /// anchor instead of collapsing it.
    pub fn set_caret(&mut self, index: usize, select: bool) {
        let index = index.min(self.len());
        if select {
            self.anchor = Some(self.anchor.unwrap_or(self.cursor));
        } else {
            self.anchor = None;
        }
        self.cursor = index;
    }

    fn shift_cursor(&mut self, delta: isize, select: bool, toward_selection_edge: usize) {
        if select {
            self.anchor = Some(self.anchor.unwrap_or(self.cursor));
            let next = (self.cursor as isize + delta)
                .max(0)
                .min(self.len() as isize) as usize;
            self.cursor = next;
        } else if self.selection_chars().is_some() {
            self.cursor = toward_selection_edge;
            self.anchor = None;
        } else {
            self.cursor = (self.cursor as isize + delta)
                .max(0)
                .min(self.len() as isize) as usize;
        }
    }
}

/// Maps an x offset (pixels from the left edge of the rendered text) to the
/// nearest `char` index using the window's real text metrics: the caret lands
/// at the char whose half-width the pointer crosses first. `font_size_px`
/// must match the size the field renders at, or the mapping drifts.
pub fn caret_index_at_x(window: &Window, value: &str, x: f32, font_size_px: f32) -> usize {
    let chars: Vec<char> = value.chars().collect();
    if chars.is_empty() || x <= 0.0 {
        return 0;
    }
    let style = field_style(window, font_size_px);
    let mut cumulative = 0.0f32;
    for (i, ch) in chars.iter().enumerate() {
        let fragment = ch.to_string();
        // `TextRun.len` is in UTF-8 bytes, so every fragment needs its own run.
        let runs = [style.to_run(fragment.len())];
        let w = f32::from(
            window
                .text_system()
                .layout_line(
                    &fragment,
                    style.font_size.to_pixels(window.rem_size()),
                    &runs,
                    None,
                )
                .width,
        );
        if cumulative + w / 2.0 >= x {
            return i;
        }
        cumulative += w;
    }
    chars.len()
}

/// Width of the caret prefix in pixels (for drawing the caret line).
pub fn caret_prefix_width(window: &Window, value: &str, cursor: usize, font_size_px: f32) -> f32 {
    let prefix: String = value.chars().take(cursor).collect();
    if prefix.is_empty() {
        return 0.0;
    }
    let style = field_style(window, font_size_px);
    let run = style.to_run(prefix.len());
    let layout = window.text_system().layout_line(
        &prefix,
        style.font_size.to_pixels(window.rem_size()),
        &[run],
        None,
    );
    f32::from(layout.width)
}

/// The text style measurement must share with the field rendering, pinned to
/// the field's pixel font size.
pub fn field_style(window: &Window, font_size_px: f32) -> gpui::TextStyle {
    let mut style = window.text_style();
    style.font_size = gpui::AbsoluteLength::Pixels(px(font_size_px));
    style
}

/// Renders the focused field value: selection background highlight plus a
/// static caret line (blinking is intentionally skipped until a frame timer
/// exists). `pad_left` mirrors the field box padding so the caret x matches
/// the painted glyphs.
pub fn render_caret_value(
    window: &Window,
    session: &CaretSession,
    font_size_px: f32,
    pad_left: f32,
    text_color: gpui::Rgba,
    accent: gpui::Rgba,
    selection_bg: gpui::Rgba,
) -> gpui::AnyElement {
    let mut style = field_style(window, font_size_px);
    style.color = text_color.into();
    let highlights: Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> = session
        .selection_bytes()
        .map(|(s, e)| {
            vec![(
                s..e,
                gpui::HighlightStyle {
                    background_color: Some(selection_bg.into()),
                    ..Default::default()
                },
            )]
        })
        .unwrap_or_default();
    let text =
        gpui::StyledText::new(session.value.clone()).with_default_highlights(&style, highlights);
    let caret_x = caret_prefix_width(window, &session.value, session.cursor, font_size_px);
    gpui::div()
        .relative()
        .child(text)
        .child(
            gpui::div()
                .absolute()
                .left(px(pad_left + caret_x))
                .top(px(0.))
                .w(px(1.))
                .h(px(font_size_px + 2.))
                .bg(accent),
        )
        .into_any_element()
}

/// Reusable caret wiring for one single-line field (plan §5.1 completion):
/// owns the editing session plus the bounds probe used to map a click onto a
/// caret index. Host modals keep one `CaretField` per editable box and call
/// `keystroke`/`place_caret`/`display` from their existing handlers.
pub struct CaretField {
    pub session: CaretSession,
    bounds: Rc<RefCell<Option<gpui::Bounds<Pixels>>>>,
    /// Active mouse-drag selection (browser-like): mousedown sets anchor,
    /// mouse-move extends, mouse-up ends the drag.
    dragging: bool,
    drag_anchor: usize,
    /// Consecutive click count (1 = caret, 2 = word, 3 = all) within a short
    /// window, tracked by the host's click handler for double/triple click.
    pub click_count: u8,
}

impl Default for CaretField {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl CaretField {
    pub fn new(value: String) -> Self {
        Self {
            session: CaretSession::new(value),
            bounds: Rc::new(RefCell::new(None)),
            dragging: false,
            drag_anchor: 0,
            click_count: 1,
        }
    }

    pub fn set_value(&mut self, value: String) {
        self.session.sync_value(value);
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Zero-paint canvas recording where the value box rendered, so the next
    /// click can map pointer-x onto a caret index. Add as an absolute child
    /// of the field box.
    pub fn bounds_probe(&self) -> gpui::AnyElement {
        let cell = self.bounds.clone();
        gpui::canvas(
            move |bounds, _, _| *cell.borrow_mut() = Some(bounds),
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }

    /// Applies a key event to the session; returns the new value when the
    /// text changed so the host can persist it into its draft.
    pub fn keystroke(&mut self, event: &KeyDownEvent, current: &str) -> Option<String> {
        self.session.sync_value(current.to_owned());
        let keystroke = &event.keystroke;
        let mods = EditModifiers {
            control: keystroke.modifiers.control,
            alt: keystroke.modifiers.alt,
            platform: keystroke.modifiers.platform,
            shift: keystroke.modifiers.shift,
        };
        if self
            .session
            .apply_keystroke(&keystroke.key, keystroke.key_char.as_deref(), mods)
        {
            return Some(self.session.value.clone());
        }
        None
    }

    fn index_at_x(&self, window: &Window, x: Pixels, font_size_px: f32, pad_left: f32) -> usize {
        let Some(bounds) = *self.bounds.borrow() else {
            return self.session.len();
        };
        let local = f32::from(x - bounds.origin.x) - pad_left;
        caret_index_at_x(
            window,
            &self.session.value,
            local.max(0.),
            font_size_px,
        )
    }

    /// Focus click: place the caret under the pointer using the recorded
    /// box bounds; falls back to the end of the text before a first frame.
    /// `click_count` follows GPUI's mouse-down count (1/2/3) for
    /// single/double/triple click behavior.
    pub fn place_caret(
        &mut self,
        window: &Window,
        click_x: Pixels,
        current: &str,
        font_size_px: f32,
        pad_left: f32,
    ) {
        self.place_caret_with_clicks(window, click_x, current, font_size_px, pad_left, 1, false);
    }

    /// Browser-like click handling: 1 = caret/drag start, 2 = select word,
    /// 3 = select all. Shift+click extends the current selection.
    pub fn place_caret_with_clicks(
        &mut self,
        window: &Window,
        click_x: Pixels,
        current: &str,
        font_size_px: f32,
        pad_left: f32,
        click_count: usize,
        shift: bool,
    ) {
        self.session.sync_value(current.to_owned());
        self.click_count = click_count.min(3) as u8;
        let index = self.index_at_x(window, click_x, font_size_px, pad_left);
        if shift && click_count <= 1 {
            let anchor = self.session.anchor.unwrap_or(self.session.cursor);
            self.session.anchor = Some(anchor);
            self.session.cursor = index;
            self.dragging = true;
            self.drag_anchor = anchor;
            return;
        }
        match click_count {
            2 => {
                let (start, end) = word_bounds(&self.session.value, index);
                self.session.anchor = Some(start);
                self.session.cursor = end;
                self.dragging = true;
                self.drag_anchor = start;
            }
            3.. => {
                self.session.anchor = Some(0);
                self.session.cursor = self.session.len();
                self.dragging = false;
                self.drag_anchor = 0;
            }
            _ => {
                self.session.anchor = None;
                self.session.cursor = index;
                self.dragging = true;
                self.drag_anchor = index;
            }
        }
    }

    /// Extends the selection while the mouse is dragged (browser-like).
    pub fn drag_to(
        &mut self,
        window: &Window,
        x: Pixels,
        _current: &str,
        font_size_px: f32,
        pad_left: f32,
    ) {
        if !self.dragging {
            return;
        }
        let index = self.index_at_x(window, x, font_size_px, pad_left);
        self.session.anchor = Some(self.drag_anchor);
        self.session.cursor = index;
    }

    pub fn end_drag(&mut self) {
        self.dragging = false;
        if self.session.selection_chars().is_none() {
            self.session.anchor = None;
        }
    }

    /// Renders the focused variant (selection highlight + caret line); the
    /// host falls back to its own plain text child when `focused` is false.
    pub fn display(
        &self,
        window: &Window,
        font_size_px: f32,
        pad_left: f32,
        text_color: gpui::Rgba,
        accent: gpui::Rgba,
    ) -> gpui::AnyElement {
        let selection_bg = gpui::Rgba { a: 0.28, ..accent };
        render_caret_value(
            window,
            &self.session,
            font_size_px,
            pad_left,
            text_color,
            accent,
            selection_bg,
        )
    }
}

/// Word bounds at a char index (double-click selection).
pub fn word_bounds(value: &str, index: usize) -> (usize, usize) {
    let chars: Vec<char> = value.chars().collect();
    if chars.is_empty() {
        return (0, 0);
    }
    let mut i = index.min(chars.len());
    if i == chars.len() && i > 0 {
        i -= 1;
    }
    let word_char = |c: char| c.is_alphanumeric() || c == '_' ;
    if !word_char(chars[i]) {
        // Select the separator run or single char.
        let mut start = i;
        let mut end = i + 1;
        while start > 0 && !word_char(chars[start - 1]) && !chars[start - 1].is_whitespace() { start -= 1; }
        while end < chars.len() && !word_char(chars[end]) && !chars[end].is_whitespace() { end += 1; }
        return (start, end);
    }
    let mut start = i;
    let mut end = i + 1;
    while start > 0 && word_char(chars[start - 1]) {
        start -= 1;
    }
    while end < chars.len() && word_char(chars[end]) {
        end += 1;
    }
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(text: &str) -> CaretSession {
        CaretSession::new(text.to_string())
    }

    fn type_text(s: &mut CaretSession, keys: &str) -> bool {
        let mut changed = false;
        for ch in keys.chars() {
            changed |= s.apply_keystroke(
                &ch.to_string(),
                Some(&ch.to_string()),
                EditModifiers::default(),
            );
        }
        changed
    }

    #[test]
    fn typing_at_end_matches_legacy_append() {
        let mut s = session("");
        assert!(type_text(&mut s, "abc"));
        assert_eq!(s.value, "abc");
    }

    #[test]
    fn caret_navigation_then_insert_in_the_middle() {
        let mut s = session("abcd");
        s.apply_keystroke("left", None, EditModifiers::default());
        s.apply_keystroke("left", None, EditModifiers::default());
        assert_eq!(s.cursor, 2);
        type_text(&mut s, "X");
        assert_eq!(s.value, "abXcd");
        assert_eq!(s.cursor, 3);
    }

    #[test]
    fn backspace_deletes_left_of_caret_not_at_end() {
        let mut s = session("abcd");
        s.set_caret(2, false);
        s.apply_keystroke("backspace", None, EditModifiers::default());
        assert_eq!(s.value, "acd");
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn shift_arrows_select_then_typing_replaces_selection() {
        let mut s = session("hello");
        s.set_caret(0, false);
        for _ in 0..3 {
            s.apply_keystroke(
                "right",
                None,
                EditModifiers {
                    shift: true,
                    ..Default::default()
                },
            );
        }
        assert_eq!(s.selection_chars(), Some((0, 3)));
        type_text(&mut s, "X");
        assert_eq!(s.value, "Xlo");
        assert_eq!(s.anchor, None);
    }

    #[test]
    fn arrow_over_selection_collapses_to_edge() {
        let mut s = session("hello");
        s.anchor = Some(1);
        s.cursor = 4;
        s.apply_keystroke("left", None, EditModifiers::default());
        assert_eq!(s.cursor, 1);
        assert_eq!(s.anchor, None);
        s.cursor = 4;
        s.anchor = Some(1);
        s.apply_keystroke("right", None, EditModifiers::default());
        assert_eq!(s.cursor, 4);
    }

    #[test]
    fn home_end_move_caret() {
        let mut s = session("abcd");
        s.apply_keystroke("home", None, EditModifiers::default());
        assert_eq!(s.cursor, 0);
        s.apply_keystroke("end", None, EditModifiers::default());
        assert_eq!(s.cursor, 4);
    }

    #[test]
    fn ctrl_backspace_deletes_word() {
        let mut s = session("alpha beta");
        s.apply_keystroke(
            "backspace",
            None,
            EditModifiers {
                control: true,
                ..Default::default()
            },
        );
        assert_eq!(s.value, "alpha ");
    }

    #[test]
    fn escape_collapses_selection_then_clears_value() {
        let mut s = session("keep");
        s.anchor = Some(0);
        s.apply_keystroke("escape", None, EditModifiers::default());
        assert_eq!(s.anchor, None);
        assert_eq!(s.value, "keep");
        s.apply_keystroke("escape", None, EditModifiers::default());
        assert_eq!(s.value, "");
    }

    #[test]
    fn ctrl_a_selects_all_and_typing_replaces() {
        let mut s = session("abcd");
        s.apply_keystroke(
            "a",
            Some("a"),
            EditModifiers {
                control: true,
                ..Default::default()
            },
        );
        assert_eq!(s.selection_chars(), Some((0, 4)));
        type_text(&mut s, "z");
        assert_eq!(s.value, "z");
    }

    #[test]
    fn ctrl_left_jumps_word_boundary() {
        let mut s = session("alpha beta gamma");
        s.set_caret(11, false); // inside "beta"
        s.apply_keystroke(
            "left",
            None,
            EditModifiers {
                control: true,
                ..Default::default()
            },
        );
        assert_eq!(s.cursor, 6);
    }

    #[test]
    fn unicode_values_use_char_indices() {
        let mut s = session("zażółć gęślą jaźń");
        s.apply_keystroke("home", None, EditModifiers::default());
        type_text(&mut s, "X");
        assert!(s.value.starts_with("X"));
        assert_eq!(s.cursor, 1);
        assert_eq!(s.byte_of(1), Some(1));
    }

    #[test]
    fn sync_value_preserves_clamped_caret() {
        let mut s = session("abcdef");
        s.set_caret(3, false);
        s.sync_value("ab".into());
        assert_eq!(s.cursor, 2);
        assert_eq!(s.value, "ab");
    }
}
