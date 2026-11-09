use gpui::{div, prelude::*, Context, IntoElement, Render, SharedString, Window};

use crate::theme::RevoTheme;

use super::audio_mixer::MixerEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextMenuEntry {
    AudioFilters,
    AdvancedProperties,
}

impl ContextMenuEntry {
    pub fn label(self) -> &'static str {
        match self {
            Self::AudioFilters => "Audio Filters",
            Self::AdvancedProperties => "Advanced Audio Properties",
        }
    }

    pub fn all() -> [Self; 2] {
        [Self::AudioFilters, Self::AdvancedProperties]
    }
}

pub struct AudioMixerContextMenu {
    pub open: bool,
    pub source_id: Option<String>,
    pub selected_index: Option<usize>,
    pub theme: RevoTheme,
}

impl Default for AudioMixerContextMenu {
    fn default() -> Self {
        Self {
            open: false,
            source_id: None,
            selected_index: None,
            theme: RevoTheme::future(),
        }
    }
}

impl gpui::EventEmitter<MixerEvent> for AudioMixerContextMenu {}

impl AudioMixerContextMenu {
    pub fn open_menu(&mut self, source_id: impl Into<String>) {
        self.open = true;
        self.source_id = Some(source_id.into());
        self.selected_index = Some(0);
    }

    pub fn close(&mut self) {
        self.open = false;
        self.source_id = None;
        self.selected_index = None;
    }

    pub fn move_selection(&mut self, offset: isize) {
        let count = ContextMenuEntry::all().len();
        let Some(index) = self.selected_index else {
            self.selected_index = Some(0);
            return;
        };
        let next = (index as isize + offset).rem_euclid(count as isize) as usize;
        self.selected_index = Some(next);
    }

    pub fn select(&mut self, index: usize) {
        if index < ContextMenuEntry::all().len() {
            self.selected_index = Some(index);
        }
    }

    pub fn select_entry(&mut self, entry: ContextMenuEntry) {
        self.select(
            ContextMenuEntry::all()
                .iter()
                .position(|candidate| candidate == &entry)
                .unwrap_or(0),
        );
    }

    pub fn choose(&mut self, entry: ContextMenuEntry) -> Option<MixerEvent> {
        if !self.open {
            return None;
        }
        let id = self.source_id.clone()?;
        self.close();
        Some(match entry {
            ContextMenuEntry::AudioFilters => MixerEvent::OpenFilters { id },
            ContextMenuEntry::AdvancedProperties => MixerEvent::OpenAdvanced { id },
        })
    }
}

impl Render for AudioMixerContextMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if !self.open {
            return div();
        }
        if self.source_id.is_none() {
            return div();
        }
        let mut menu = div()
            .absolute()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .bg(self.theme.surface_elevated)
            .border_1()
            .border_color(self.theme.border_strong)
            .rounded_md()
            .shadow_md();
        for (index, entry) in ContextMenuEntry::all().into_iter().enumerate() {
            let selected = self.selected_index == Some(index);
            menu = menu.child(
                div()
                    .id(SharedString::from(format!("mixer-context-{index}")))
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(if selected {
                        self.theme.surface_selected
                    } else {
                        self.theme.surface
                    })
                    .text_color(self.theme.text)
                    .child(entry.label())
                    .on_click(cx.listener({
                        move |this, _, _, cx| {
                            if let Some(event) = this.choose(entry) {
                                this.close();
                                cx.emit(event);
                            }
                            cx.notify();
                        }
                    })),
            );
        }
        div()
            .absolute()
            .size_full()
            .child(
                div()
                    .id("mixer-context-overlay")
                    .absolute()
                    .size_full()
                    .top_0()
                    .left_0()
                    .occlude()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close();
                        cx.notify();
                    })),
            )
            .child(menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu() -> AudioMixerContextMenu {
        AudioMixerContextMenu {
            theme: RevoTheme::future(),
            ..AudioMixerContextMenu::default()
        }
    }

    #[test]
    fn open_menu_tracks_source_and_selection() {
        let mut menu = menu();
        menu.open_menu("camera");
        assert!(menu.open);
        assert_eq!(menu.source_id.as_deref(), Some("camera"));
        assert_eq!(menu.selected_index, Some(0));

        menu.move_selection(1);
        assert_eq!(menu.selected_index, Some(1));
        menu.move_selection(1);
        assert_eq!(menu.selected_index, Some(0));
        menu.close();
        assert_eq!(menu.source_id, None);
    }

    #[test]
    fn choose_emits_typed_mixer_actions_and_closes() {
        let mut menu = menu();
        menu.open_menu("camera");
        assert_eq!(
            menu.choose(ContextMenuEntry::AudioFilters),
            Some(MixerEvent::OpenFilters {
                id: "camera".into()
            })
        );
        assert!(!menu.open);

        menu.open_menu("camera");
        assert_eq!(
            menu.choose(ContextMenuEntry::AdvancedProperties),
            Some(MixerEvent::OpenAdvanced {
                id: "camera".into()
            })
        );

        menu.close();
        menu.open = false;
        assert_eq!(menu.choose(ContextMenuEntry::AudioFilters), None);
    }

    #[test]
    fn entry_labels_match_svelte_options() {
        let labels: Vec<&str> = ContextMenuEntry::all()
            .iter()
            .map(|entry| entry.label())
            .collect();
        assert_eq!(labels, ["Audio Filters", "Advanced Audio Properties"]);
    }
}
