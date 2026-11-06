use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

pub(crate) use revo_core::models::{
	AppSettings, AudioDevice, ObsSettingsSelectOptions, PlannerInit, PlannerInitInput,
	PluginInfo, PluginProfile, PluginProfileState, PluginWindowCapabilities, ReusableSourceInfo,
	SceneInfo, SelectOption, SourceCreate, SourceFilterItem, SourceInfo, SourcePropertyOption,
	SourcePropertySpec, SourceTypeItem, SourceUpdate, ThemeInfo, VideoEncoderPreference,
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
