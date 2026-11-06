use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

pub use revo_core::models::{
	AppSettings, AudioDevice, ObsSettingsSelectOptions, PlannerInit, PlannerInitInput,
	PluginInfo, PluginProfile, PluginProfileState, PluginWindowCapabilities, ReusableSourceInfo,
	SceneInfo, SelectOption, SourceCreate, SourceFilterItem, SourceInfo, SourcePropertyOption,
	SourcePropertySpec, SourceTypeItem, SourceUpdate, ThemeInfo, VideoEncoderPreference,
};

#[derive(Serialize, Deserialize, Clone)]
pub struct LibObsActionEvent {
	pub timestamp_ms: u64,
	pub action: String,
	pub detail: Option<JsonValue>,
}

#[derive(Serialize)]
pub struct CefBridgeInfo {
	pub compiled: bool,
	pub major: i32,
	pub minor: i32,
	pub patch: i32,
	pub commit: i32,
}

#[derive(Serialize)]
pub struct BrowserDockState {
	pub is_open: bool,
	pub url: Option<String>,
}
