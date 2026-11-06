//! Native port of the Tauri debug-console auxiliary window
//! (`src/routes/debug-console/+page.svelte`): libobs log parsing, level
//! tabs, fuzzy search, autoscroll, and clear.
//!
//! Mock mode placeholder: the `revo-backend` debug-log registry is only
//! linked with the `obs-backend` feature, so in mock builds entries stay
//! local/empty and the 1200 ms registry-pull loop is not started.

use gpui::{
    div, prelude::*, px, Context, FocusHandle, IntoElement, Render, ScrollHandle, SharedString,
    Task, Window,
};
#[cfg(feature = "obs-backend")]
use std::time::Duration;

use serde_json::Value as JsonValue;

use crate::services::AnyBackend;
use crate::theme::RevoTheme;

/// Registry-pull cadence matching the Svelte `setInterval(refreshHistory, 1200)`.
pub const DEBUG_CONSOLE_POLL_INTERVAL_MS: u64 = 1200;

/// Local cap matching the Svelte `logs.slice(-1000)`.
pub const DEBUG_LOG_CAP: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Action,
}

impl LogLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Action => "action",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelFilter {
    All,
    Action,
    Debug,
    Info,
    Warning,
    Error,
}

impl LevelFilter {
    /// Tab order and labels exactly as the Svelte `levelTabs` array.
    pub const TABS: [Self; 6] = [
        Self::All,
        Self::Action,
        Self::Debug,
        Self::Info,
        Self::Warning,
        Self::Error,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Action => "Action",
            Self::Debug => "Debug",
            Self::Info => "Info",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }

    pub fn matches(self, level: LogLevel) -> bool {
        match self {
            Self::All => true,
            Self::Action => level == LogLevel::Action,
            Self::Debug => level == LogLevel::Debug,
            Self::Info => level == LogLevel::Info,
            Self::Warning => level == LogLevel::Warning,
            Self::Error => level == LogLevel::Error,
        }
    }
}

/// Local typed log row: the framework-neutral twin of the registry's
/// `LibObsActionEvent` (kept independent so mock builds need no revo-backend).
#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry {
    pub timestamp_ms: u64,
    pub action: String,
    pub detail_json: Option<JsonValue>,
}

impl LogEntry {
    pub fn new(
        timestamp_ms: u64,
        action: impl Into<String>,
        detail_json: Option<JsonValue>,
    ) -> Self {
        Self {
            timestamp_ms,
            action: action.into(),
            detail_json,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedLog {
    pub level: LogLevel,
    pub message: String,
    pub detail: Option<String>,
}

/// Exact port of the Svelte `parseLog`:
/// `libobs:<level>:...` actions map onto the level (unknown levels fall back
/// to `debug`); object details carry the message, string details replace the
/// action, everything else renders the action with stringified detail.
pub fn parse_log(entry: &LogEntry) -> ParsedLog {
    if entry.action.starts_with("libobs:") {
        let level_part = entry.action.split(':').nth(1).unwrap_or("debug");
        let level = match level_part {
            "debug" => LogLevel::Debug,
            "info" => LogLevel::Info,
            "warning" => LogLevel::Warning,
            "error" => LogLevel::Error,
            _ => LogLevel::Debug,
        };

        if let Some(detail) = &entry.detail_json {
            if detail.is_object() || detail.is_array() {
                let message = detail
                    .get("message")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("");
                return ParsedLog {
                    level,
                    message: if message.is_empty() {
                        "(empty)".to_owned()
                    } else {
                        message.to_owned()
                    },
                    detail: None,
                };
            }
        }

        return ParsedLog {
            level,
            message: match &entry.detail_json {
                Some(JsonValue::String(text)) => text.clone(),
                _ => entry.action.clone(),
            },
            detail: None,
        };
    }

    ParsedLog {
        level: LogLevel::Action,
        message: entry.action.clone(),
        detail: entry.detail_json.as_ref().map(|detail| detail.to_string()),
    }
}

/// Exact port of the Svelte `normalizeSearch` (lowercase, separators to
/// spaces, whitespace collapse, trim). The `NFKD` decomposition step is
/// approximated by lowercase-only folding: the unicode-normalization crate
/// is not vendored in this offline environment.
pub fn normalize_search(value: &str) -> String {
    let folded: String = value
        .to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                ch
            } else {
                ' '
            }
        })
        .collect();
    let mut out = String::new();
    let mut pending_space = false;
    for ch in folded.chars() {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(ch);
        }
    }
    out
}

/// Exact port of the Svelte `hasFuzzySubsequence`.
pub fn has_fuzzy_subsequence(query: &str, text: &str) -> bool {
    let query_chars: Vec<char> = query.chars().collect();
    if query_chars.is_empty() {
        return true;
    }
    let mut matched = 0;
    for ch in text.chars() {
        if Some(ch) == query_chars.get(matched).copied() {
            matched += 1;
        }
        if matched >= query_chars.len() {
            return true;
        }
    }
    false
}

/// Exact port of the Svelte `fuzzyMatches`: containment, multi-token
/// word matching, then whitespace-stripped fuzzy subsequence.
pub fn fuzzy_matches(query: &str, text: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    if text.is_empty() {
        return false;
    }
    if text.contains(query) {
        return true;
    }

    let query_tokens: Vec<&str> = query.split(' ').filter(|token| !token.is_empty()).collect();
    if query_tokens.len() > 1 {
        let text_tokens: Vec<&str> = text.split(' ').filter(|token| !token.is_empty()).collect();
        if query_tokens.iter().all(|query_token| {
            text_tokens.iter().any(|text_token| {
                text_token.contains(query_token) || has_fuzzy_subsequence(query_token, text_token)
            })
        }) {
            return true;
        }
    }

    has_fuzzy_subsequence(
        query.replace([' '], "").as_str(),
        text.replace([' '], "").as_str(),
    )
}

/// Exact port of the Svelte filtering reactive block: level tab filter plus
/// fuzzy search across `message detail level`.
pub fn entry_matches(filter: LevelFilter, search_query: &str, parsed: &ParsedLog) -> bool {
    if !filter.matches(parsed.level) {
        return false;
    }

    let query = normalize_search(search_query);
    if query.is_empty() {
        return true;
    }

    let haystack = normalize_search(&format!(
        "{} {} {}",
        parsed.message,
        parsed.detail.as_deref().unwrap_or(""),
        parsed.level.label(),
    ));
    fuzzy_matches(&query, &haystack)
}

/// One rendered log row: the source entry plus its parsed view (the Svelte
/// `parsedLogs` pairs).
#[derive(Clone, Debug, PartialEq)]
pub struct LogRow {
    pub entry: LogEntry,
    pub parsed: ParsedLog,
}

/// Pure filtered view over the entries (the Svelte `filteredLogs`): level
/// tab filter plus fuzzy search across `message detail level`.
pub fn visible_rows(entries: &[LogEntry], filter: LevelFilter, search_query: &str) -> Vec<LogRow> {
    entries
        .iter()
        .map(|entry| {
            let parsed = parse_log(entry);
            LogRow {
                entry: entry.clone(),
                parsed,
            }
        })
        .filter(|row| entry_matches(filter, search_query, &row.parsed))
        .collect()
}

/// Append with the Svelte `slice(-1000)` retention cap.
pub fn append_entry(entries: &mut Vec<LogEntry>, entry: LogEntry) {
    entries.push(entry);
    let keep_from = entries.len().saturating_sub(DEBUG_LOG_CAP);
    if keep_from > 0 {
        entries.drain(0..keep_from);
    }
}

/// `new Date(ms).toLocaleTimeString()` equivalent as UTC `HH:MM:SS`
/// (the native window has no locale database; UTC is the documented
/// approximation).
pub fn format_time_of_day(timestamp_ms: u64) -> String {
    let secs = (timestamp_ms / 1000) % 86_400;
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Search-field keystroke handling for the native (non-web) search box.
/// Delegates to the shared single-line policy in `components::text_field`;
/// kept as a re-export so existing call sites and tests stay stable.
pub use crate::components::text_field::apply_line_keystroke as apply_search_keystroke;

pub struct DebugConsoleWindow {
    pub entries: Vec<LogEntry>,
    pub active_filter: LevelFilter,
    pub search_query: String,
    pub auto_scroll: bool,
    pub theme: RevoTheme,
    search_focus: FocusHandle,
    list_scroll: ScrollHandle,
    /// Owned registry-pull loop; dropped (cancelling it) with the view.
    refresh_task: Option<Task<()>>,
}

impl DebugConsoleWindow {
    pub fn new(backend: AnyBackend, cx: &mut Context<Self>) -> Self {
        let theme = RevoTheme::future();
        let mut window = Self {
            entries: Vec::new(),
            active_filter: LevelFilter::All,
            search_query: String::new(),
            auto_scroll: true,
            theme,
            search_focus: cx.focus_handle(),
            list_scroll: ScrollHandle::new(),
            refresh_task: None,
        };
        window.refresh_task = Some(window.spawn_refresh(backend, cx));
        window
    }

    /// Starts the 1200 ms registry-pull loop only for the obs backend; mock
    /// builds keep entries local/empty (documented placeholder).
    fn spawn_refresh(&self, backend: AnyBackend, cx: &mut Context<Self>) -> Task<()> {
        #[cfg(feature = "obs-backend")]
        if matches!(backend, AnyBackend::Obs(_)) {
            return cx.spawn(async move |window, cx| loop {
                cx.background_executor()
                    .timer(Duration::from_millis(DEBUG_CONSOLE_POLL_INTERVAL_MS))
                    .await;
                let history = cx
                    .background_executor()
                    .spawn(async move {
                        revo_backend::logging::debug::get_debug_log_entries()
                            .into_iter()
                            .map(|event| LogEntry {
                                timestamp_ms: event.timestamp_ms,
                                action: event.action,
                                detail_json: event.detail,
                            })
                            .collect::<Vec<_>>()
                    })
                    .await;
                let alive = window
                    .update(cx, |window, cx| window.replace_history(history, cx))
                    .is_ok();
                if !alive {
                    return;
                }
            });
        }
        #[cfg(feature = "obs-backend")]
        {
            let _ = cx;
        }
        #[cfg(not(feature = "obs-backend"))]
        {
            let _ = (backend, cx);
        }
        Task::ready(())
    }

    /// Replaces the local rows from a registry snapshot
    /// (`logs = history.slice(-1000)`).
    pub fn replace_history(&mut self, history: Vec<LogEntry>, cx: &mut Context<'_, Self>) {
        self.entries = history
            .into_iter()
            .rev()
            .take(DEBUG_LOG_CAP)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if self.auto_scroll {
            self.list_scroll.scroll_to_bottom();
        }
        cx.notify();
    }

    pub fn set_filter(&mut self, filter: LevelFilter, cx: &mut Context<'_, Self>) {
        self.active_filter = filter;
        cx.notify();
    }

    pub fn set_search_query(&mut self, query: String, cx: &mut Context<'_, Self>) {
        self.search_query = query;
        cx.notify();
    }

    pub fn toggle_auto_scroll(&mut self, cx: &mut Context<'_, Self>) {
        self.auto_scroll = !self.auto_scroll;
        cx.notify();
    }

    /// Local clear plus registry clear behind `obs-backend` (the Svelte
    /// window clears local rows then invokes `debug_console_clear`).
    pub fn clear(&mut self, cx: &mut Context<'_, Self>) {
        self.entries.clear();
        #[cfg(feature = "obs-backend")]
        {
            revo_backend::logging::debug::clear_debug_log_entries();
        }
        cx.notify();
    }
}

impl Render for DebugConsoleWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let theme = self.theme;
        let rows = visible_rows(&self.entries, self.active_filter, &self.search_query);
        let search_focused = self.search_focus.is_focused(window);

        let mut tabs = div().flex().gap_1().flex_wrap();
        for tab in LevelFilter::TABS {
            let active = self.active_filter == tab;
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("debug-tab-{}", tab.label())))
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_full()
                    .border_1()
                    .border_color(if active {
                        theme.border_selected
                    } else {
                        theme.border
                    })
                    .bg(if active {
                        theme.surface_active
                    } else {
                        theme.surface_elevated
                    })
                    .text_color(if active {
                        theme.text_strong
                    } else {
                        theme.text_secondary
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_filter(tab, cx);
                    }))
                    .child(tab.label()),
            );
        }

        let mut list = div()
            .id("debug-console-rows")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .track_scroll(&self.list_scroll)
            .flex()
            .flex_col()
            .gap_1()
            .p_2();
        if rows.is_empty() {
            list = list.child(
                div()
                    .text_color(theme.text_muted)
                    .child("No actions yet. Press F10 in main window and use app actions."),
            );
        } else {
            for row in rows {
                let level_color = match row.parsed.level {
                    LogLevel::Debug => theme.text_muted,
                    LogLevel::Info => theme.text_secondary,
                    LogLevel::Warning => theme.warning,
                    LogLevel::Error => theme.danger,
                    LogLevel::Action => theme.success,
                };
                let mut row_view = div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_1()
                    .border_1()
                    .border_color(theme.border_subtle)
                    .rounded_md()
                    .bg(theme.surface_elevated)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .items_center()
                            .child(div().text_color(theme.accent).text_size(px(11.)).child(
                                SharedString::from(format_time_of_day(row.entry.timestamp_ms)),
                            ))
                            .child(
                                div()
                                    .text_color(level_color)
                                    .text_size(px(11.))
                                    .child(row.parsed.level.label()),
                            )
                            .child(
                                div()
                                    .text_color(theme.text)
                                    .text_size(px(12.))
                                    .child(SharedString::from(row.parsed.message.clone())),
                            ),
                    );
                if let Some(detail) = row.parsed.detail {
                    row_view = row_view.child(
                        div()
                            .ml_2()
                            .p_1()
                            .rounded_md()
                            .bg(theme.background)
                            .border_1()
                            .border_color(theme.border_subtle)
                            .text_color(theme.text_secondary)
                            .text_size(px(10.))
                            .child(SharedString::from(detail)),
                    );
                }
                list = list.child(row_view);
            }
        }

        div()
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .p_3()
            .bg(theme.background)
            .text_color(theme.text)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_color(theme.text_strong)
                            .text_size(px(14.))
                            .child("Debug Console"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .id("debug-console-autoscroll")
                                    .cursor_pointer()
                                    .text_color(theme.text_secondary)
                                    .child(if self.auto_scroll {
                                        "Auto-scroll [x]"
                                    } else {
                                        "Auto-scroll [ ]"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_auto_scroll(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("debug-console-clear")
                                    .cursor_pointer()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(theme.surface_active)
                                    .text_color(theme.text)
                                    .child("Clear")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.clear(cx);
                                    })),
                            ),
                    ),
            )
            .child(
                div().flex().gap_2().items_center().child(tabs).child(
                    div()
                        .id("debug-console-search")
                        .cursor_pointer()
                        .flex_1()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .border_1()
                        .border_color(if search_focused {
                            theme.border_selected
                        } else {
                            theme.border
                        })
                        .bg(theme.surface)
                        .text_color(if self.search_query.is_empty() {
                            theme.text_muted
                        } else {
                            theme.text
                        })
                        .track_focus(&self.search_focus)
                        .key_context("DebugConsole")
                        .on_click(cx.listener(|this, _, window, cx| {
                            window.focus(&this.search_focus.clone());
                            cx.notify();
                        }))
                        .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                            if event.is_held {
                                return;
                            }
                            let keystroke = &event.keystroke;
                            if let Some(next) = apply_search_keystroke(
                                &this.search_query,
                                &keystroke.key,
                                keystroke.key_char.as_deref(),
                                keystroke.modifiers.control,
                                keystroke.modifiers.alt,
                                keystroke.modifiers.platform,
                            ) {
                                this.set_search_query(next, cx);
                            }
                        }))
                        .child(if self.search_query.is_empty() {
                            SharedString::from("Search logs (similar phrases)")
                        } else {
                            SharedString::from(self.search_query.clone())
                        }),
                ),
            )
            .child(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(action: &str, detail: Option<JsonValue>) -> LogEntry {
        LogEntry::new(1_700_000_000_000, action, detail)
    }

    #[test]
    fn parse_log_routes_libobs_prefix_levels() {
        for (action, expected) in [
            ("libobs:debug:x", LogLevel::Debug),
            ("libobs:info:x", LogLevel::Info),
            ("libobs:warning:x", LogLevel::Warning),
            ("libobs:error:x", LogLevel::Error),
            ("libobs:unknown:x", LogLevel::Debug),
            ("libobs:", LogLevel::Debug),
        ] {
            assert_eq!(parse_log(&entry(action, None)).level, expected);
        }
        assert_eq!(
            parse_log(&entry("app_action", None)).level,
            LogLevel::Action
        );
    }

    #[test]
    fn parse_log_extracts_object_messages_and_string_details() {
        let parsed = parse_log(&entry(
            "libobs:info:log",
            Some(serde_json::json!({ "message": "started" })),
        ));
        assert_eq!(parsed.message, "started");
        assert_eq!(parsed.detail, None);

        let parsed = parse_log(&entry("libobs:info:log", Some(serde_json::json!({}))));
        assert_eq!(parsed.message, "(empty)");

        let parsed = parse_log(&entry(
            "libobs:info:log",
            Some(JsonValue::String("raw".into())),
        ));
        assert_eq!(parsed.message, "raw");

        let parsed = parse_log(&entry("libobs:info:log", None));
        assert_eq!(parsed.message, "libobs:info:log");

        let parsed = parse_log(&entry("app_action", Some(serde_json::json!({ "k": 1 }))));
        assert_eq!(parsed.message, "app_action");
        assert_eq!(parsed.detail.as_deref(), Some("{\"k\":1}"));

        let parsed = parse_log(&entry("app_action", None));
        assert_eq!(parsed.detail, None);
    }

    #[test]
    fn level_tabs_match_exact_levels_and_all() {
        assert!(LevelFilter::All.matches(LogLevel::Action));
        assert!(LevelFilter::All.matches(LogLevel::Error));
        assert!(LevelFilter::Warning.matches(LogLevel::Warning));
        assert!(!LevelFilter::Warning.matches(LogLevel::Info));
        assert_eq!(
            LevelFilter::TABS,
            [
                LevelFilter::All,
                LevelFilter::Action,
                LevelFilter::Debug,
                LevelFilter::Info,
                LevelFilter::Warning,
                LevelFilter::Error,
            ]
        );
    }

    #[test]
    fn normalize_search_folds_case_separators_and_whitespace() {
        assert_eq!(normalize_search("  Hello,  World! "), "hello world");
        assert_eq!(normalize_search("a---b_c"), "a b c");
        assert_eq!(normalize_search("\tfoo\nbar"), "foo bar");
        assert_eq!(normalize_search(""), "");
        assert_eq!(normalize_search("!!!"), "");
    }

    #[test]
    fn fuzzy_subsequence_and_token_matching_track_svelte() {
        assert!(has_fuzzy_subsequence("", "anything"));
        assert!(has_fuzzy_subsequence("hw", "hello world"));
        assert!(!has_fuzzy_subsequence("zx", "hello world"));

        assert!(fuzzy_matches("", "anything"));
        assert!(!fuzzy_matches("q", ""));
        assert!(fuzzy_matches("world", "hello world"));
        assert!(fuzzy_matches("hw", "helloworld"));
        assert!(!fuzzy_matches("zx", "hello world"));
    }

    #[test]
    fn entry_filters_combine_level_and_fuzzy_search() {
        let entries = vec![
            entry("start_stream", None),
            entry(
                "libobs:error:log",
                Some(serde_json::json!({"message": "encoder failed"})),
            ),
        ];
        assert_eq!(visible_rows(&entries, LevelFilter::Error, "").len(), 1);
        assert_eq!(
            visible_rows(&entries, LevelFilter::All, "encdr fild").len(),
            1
        );
        assert_eq!(visible_rows(&entries, LevelFilter::All, "zxq").len(), 0);
        let rows = visible_rows(&entries, LevelFilter::All, "");
        assert_eq!(rows[0].parsed.message, "start_stream");
        assert_eq!(rows[1].entry.action, "libobs:error:log");
        assert_eq!(rows[1].parsed.message, "encoder failed");
    }

    #[test]
    fn append_entry_caps_history_to_last_thousand() {
        let mut entries: Vec<LogEntry> = Vec::new();
        for index in 0..(DEBUG_LOG_CAP + 25) {
            append_entry(&mut entries, entry(&format!("n-{index}"), None));
        }
        assert_eq!(entries.len(), DEBUG_LOG_CAP);
        assert_eq!(entries[0].action, "n-25");
        assert_eq!(entries.last().unwrap().action, "n-1024");
    }

    #[test]
    fn time_of_day_formats_from_unix_millis() {
        assert_eq!(format_time_of_day(0), "00:00:00");
        assert_eq!(format_time_of_day(1_700_000_000_000), "22:13:20");
        assert_eq!(format_time_of_day(86_400_123), "00:00:00");
    }

    #[test]
    fn search_keystrokes_edit_the_query_only_for_text_keys() {
        let query = String::from("obs");
        assert_eq!(
            apply_search_keystroke(&query, "backspace", None, false, false, false),
            Some("ob".into())
        );
        assert_eq!(
            apply_search_keystroke(&query, "a", Some("a"), false, false, false),
            Some("obsa".into())
        );
        assert_eq!(
            apply_search_keystroke(&query, "a", Some("a"), true, false, false),
            None
        );
        assert_eq!(
            apply_search_keystroke(&query, "left", None, false, false, false),
            None
        );
        assert_eq!(
            apply_search_keystroke(&query, "escape", None, false, false, false),
            Some(String::new())
        );
    }
}
