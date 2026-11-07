//! Typed draft state machine for the functional plugins modal.
//!
//! The draft mirrors the legacy Svelte workflow: profile tabs list the
//! saved profiles (with their persisted module enablement), the module
//! list toggles libobs modules on and off, and a save-as-new name field
//! turns the current draft into a new profile. Saving dispatches
//! `BackendCommand::SetPluginProfile`, which persists and applies the
//! profile in the engine in one step.

use std::collections::{BTreeMap, BTreeSet};

use revo_core::models::PluginInfo;

use crate::components::text_field::apply_line_keystroke;

#[derive(Clone, Debug, PartialEq)]
pub enum PluginsEvent {
    /// Save the draft module set under a profile name (creating or
    /// overwriting it) and apply it in the engine.
    Saved {
        profile_name: String,
        modules: Vec<String>,
    },
    Cancelled,
}

/// Draft state for the plugins modal.
pub struct PluginsDraft {
    /// Discovered libobs modules, in backend report order.
    pub plugins: Vec<PluginInfo>,
    /// Saved profile names, in persisted order.
    pub profiles: Vec<String>,
    /// Name of the currently applied profile, if any.
    pub active_profile: Option<String>,
    profile_states: BTreeMap<String, Vec<String>>,
    /// Module enablement the draft edits; seeded from the viewed profile.
    pub enabled: BTreeSet<String>,
    /// Profile the draft currently edits (`None` until a tab is chosen or
    /// a save-as-new name is typed).
    pub target_profile: Option<String>,
    /// Text of the save-as-new-profile name field.
    pub new_profile_name: String,
    dirty: bool,
}

impl PluginsDraft {
    /// Seeds the draft from the backend snapshot's plugin data. The module
    /// enablement starts from the currently applied set; the profile tabs
    /// start on the applied profile when one exists.
    pub fn load(
        plugins: Vec<PluginInfo>,
        profiles: Vec<String>,
        profile_states: Vec<(String, Vec<String>)>,
        active_profile: Option<String>,
        enabled_modules: Vec<String>,
    ) -> Self {
        let mut states = BTreeMap::new();
        for (name, modules) in profile_states {
            states.insert(name, modules);
        }
        Self {
            enabled: enabled_modules.into_iter().collect(),
            target_profile: active_profile
                .clone()
                .filter(|name| states.contains_key(name)),
            active_profile,
            plugins,
            profiles,
            profile_states: states,
            new_profile_name: String::new(),
            dirty: false,
        }
    }

    /// Flips one module's enablement in the draft.
    pub fn toggle_module(&mut self, module_name: &str) {
        if !self.enabled.remove(module_name) {
            self.enabled.insert(module_name.to_owned());
        }
        self.dirty = true;
    }

    pub fn is_module_enabled(&self, module_name: &str) -> bool {
        self.enabled.contains(module_name)
    }

    /// Views one saved profile: the draft enablement becomes that profile's
    /// persisted modules and the dirty flag clears.
    pub fn select_profile(&mut self, profile_name: &str) {
        self.target_profile = Some(profile_name.to_owned());
        self.enabled = self
            .profile_states
            .get(profile_name)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect();
        self.new_profile_name.clear();
        self.dirty = false;
    }

    /// Starts a fresh save-as-new draft on top of the current enablement.
    pub fn begin_new_profile(&mut self) {
        self.target_profile = None;
        self.new_profile_name.clear();
        self.dirty = false;
    }

    /// Routes one keystroke into the save-as-new name field.
    pub fn field_keystroke(
        &mut self,
        key: &str,
        key_char: Option<&str>,
        control: bool,
        alt: bool,
        platform: bool,
    ) {
        if let Some(next) = apply_line_keystroke(
            &self.new_profile_name,
            key,
            key_char,
            control,
            alt,
            platform,
        ) {
            self.new_profile_name = next;
        }
    }

    /// Unsaved module changes or a pending save-as-new name.
    pub fn is_dirty(&self) -> bool {
        self.dirty || !self.new_profile_name.trim().is_empty()
    }

    /// Resolves the save target and module set. A typed name wins over the
    /// selected tab; with neither the save is rejected.
    pub fn commit(&self) -> Result<(String, Vec<String>), String> {
        let typed = self.new_profile_name.trim();
        let profile_name = if !typed.is_empty() {
            typed.to_owned()
        } else if let Some(target) = &self.target_profile {
            target.clone()
        } else {
            return Err("Name the profile first".into());
        };
        Ok((
            profile_name,
            self.enabled.iter().cloned().collect::<Vec<String>>(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(name: &str) -> PluginInfo {
        PluginInfo {
            name: name.to_owned(),
            file_name: format!("{name}.so"),
            module_name: name.to_owned(),
            window_capabilities: revo_core::models::PluginWindowCapabilities {
                can_list: false,
                can_open: false,
                reason: "unsupported".into(),
            },
        }
    }

    fn sample() -> PluginsDraft {
        PluginsDraft::load(
            vec![
                plugin("obs-websocket"),
                plugin("obs-vst"),
                plugin("obs-x264"),
            ],
            vec!["default".into(), "streaming".into()],
            vec![
                ("default".into(), Vec::new()),
                (
                    "streaming".into(),
                    vec!["obs-websocket".into(), "obs-x264".into()],
                ),
            ],
            Some("default".into()),
            Vec::new(),
        )
    }

    #[test]
    fn load_seeds_from_the_applied_profile() {
        let draft = sample();
        assert_eq!(draft.target_profile.as_deref(), Some("default"));
        assert!(draft.enabled.is_empty());
        assert!(!draft.is_dirty());
    }

    #[test]
    fn toggle_flips_enablement_and_marks_dirty() {
        let mut draft = sample();
        assert!(!draft.is_module_enabled("obs-websocket"));
        draft.toggle_module("obs-websocket");
        assert!(draft.is_module_enabled("obs-websocket"));
        assert!(draft.is_dirty());
        draft.toggle_module("obs-websocket");
        assert!(!draft.is_module_enabled("obs-websocket"));
        assert!(draft.is_dirty());
    }

    #[test]
    fn select_profile_previews_persisted_enablement() {
        let mut draft = sample();
        draft.toggle_module("obs-vst");
        draft.select_profile("streaming");
        assert_eq!(draft.target_profile.as_deref(), Some("streaming"));
        assert!(draft.is_module_enabled("obs-websocket"));
        assert!(draft.is_module_enabled("obs-x264"));
        assert!(!draft.is_module_enabled("obs-vst"));
        assert!(!draft.is_dirty());
        assert!(draft.new_profile_name.is_empty());
    }

    #[test]
    fn select_profile_unknown_falls_back_to_empty_enablement() {
        let mut draft = sample();
        draft.select_profile("missing");
        assert_eq!(draft.target_profile.as_deref(), Some("missing"));
        assert!(draft.enabled.is_empty());
    }

    #[test]
    fn commit_prefers_the_typed_name_over_the_selected_tab() {
        let mut draft = sample();
        draft.select_profile("streaming");
        draft.field_keystroke("o", Some("o"), false, false, false);
        draft.field_keystroke("b", Some("b"), false, false, false);
        let (name, modules) = draft.commit().expect("typed name commits");
        assert_eq!(name, "ob");
        assert!(modules.contains(&"obs-websocket".to_owned()));
    }

    #[test]
    fn commit_uses_the_selected_tab_without_a_typed_name() {
        let mut draft = sample();
        draft.select_profile("streaming");
        let (name, _) = draft.commit().expect("tab commits");
        assert_eq!(name, "streaming");
    }

    #[test]
    fn commit_requires_a_target() {
        let mut draft = sample();
        draft.begin_new_profile();
        assert_eq!(
            draft.commit().err().as_deref(),
            Some("Name the profile first")
        );
    }

    #[test]
    fn commit_normalizes_whitespace_in_the_typed_name() {
        let mut draft = sample();
        draft.begin_new_profile();
        draft.field_keystroke("m", Some("m"), false, false, false);
        draft.field_keystroke("y", Some("y"), false, false, false);
        draft.field_keystroke(" ", Some(" "), false, false, false);
        assert!(draft.is_dirty());
        let (name, _) = draft.commit().expect("typed name commits");
        assert_eq!(name, "my");
    }
}
