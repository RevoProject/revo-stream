use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

pub(crate) use revo_core::models::{
	AudioDevice, ObsSettingsSelectOptions, PlannerInit, PlannerInitInput, ReusableSourceInfo,
	SelectOption, SourceCreate, SourceFilterItem, SourceInfo, SourcePropertyOption,
	SourcePropertySpec, SourceTypeItem, SourceUpdate, VideoEncoderPreference,
};

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct LibObsActionEvent {
	pub(crate) timestamp_ms: u64,
	pub(crate) action: String,
	pub(crate) detail: Option<JsonValue>,
}

#[derive(Serialize)]
pub(crate) struct CefBridgeInfo {
	pub(crate) compiled: bool,
	pub(crate) major: i32,
	pub(crate) minor: i32,
	pub(crate) patch: i32,
	pub(crate) commit: i32,
}

#[derive(Serialize)]
pub(crate) struct BrowserDockState {
	pub(crate) is_open: bool,
	pub(crate) url: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct SceneInfo {
	pub(crate) name: String,
	pub(crate) active: bool,
	pub(crate) locked: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct AppSettings {
	#[serde(default)]
	pub(crate) root_dir: Option<String>,
	#[serde(default = "crate::settings::core::default_record_path")]
	pub(crate) record_path: String,
	#[serde(default)]
	pub(crate) stream_url: String,
	#[serde(default)]
	pub(crate) stream_key: String,
	#[serde(default = "crate::settings::core::default_preview_quality")]
	pub(crate) preview_quality: Option<String>,
	#[serde(default)]
	pub(crate) encoder_preference: Option<String>,
	#[serde(default)]
	pub(crate) scene_resolution: Option<String>,
	#[serde(default)]
	pub(crate) whep_url: Option<String>,
	#[serde(default)]
	pub(crate) whip_url: Option<String>,
	#[serde(default)]
	pub(crate) auto_retry_preview: Option<bool>,
	#[serde(default)]
	pub(crate) autorescale_inputs: Option<bool>,
	#[serde(default)]
	pub(crate) minimize_to_tray: Option<bool>,
	#[serde(default)]
	pub(crate) built_in_browser_enabled: Option<bool>,
	#[serde(default)]
	pub(crate) obs_plugins_path: Option<String>,
	#[serde(default)]
	pub(crate) obs_vst_enabled: Option<bool>,
	#[serde(default)]
	pub(crate) obs_websocket_enabled: Option<bool>,
	#[serde(default)]
	pub(crate) obs_websocket_password: Option<String>,
	#[serde(default)]
	pub(crate) ui_profile: Option<JsonValue>,
	#[serde(default = "crate::settings::core::default_active_profile")]
	pub(crate) active_profile: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PluginInfo {
	pub(crate) name: String,
	pub(crate) file_name: String,
	pub(crate) module_name: String,
	pub(crate) window_capabilities: PluginWindowCapabilities,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PluginWindowCapabilities {
	pub(crate) can_list: bool,
	pub(crate) can_open: bool,
	pub(crate) reason: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PluginProfile {
	pub(crate) enabled_modules: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PluginProfileState {
	pub(crate) profile_name: String,
	pub(crate) enabled_modules: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct ThemeInfo {
	pub(crate) id: String,
	pub(crate) name: String,
	pub(crate) author: String,
	pub(crate) version: String,
}
