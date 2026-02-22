use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use crate::obs::obs_sys as obs;

#[derive(Default)]
pub struct ObsRuntime {
    pub initialized: bool,
    pub scenes: HashMap<String, SceneState>,
    pub current_scene: Option<String>,
    pub locked_scenes: HashSet<String>,
    pub output_record: *mut obs::obs_output,
    pub record_video_encoder: *mut obs::obs_encoder,
    pub record_audio_encoder: *mut obs::obs_encoder,
    pub output_stream: *mut obs::obs_output,
    pub stream_service: *mut obs::obs_service,
    pub stream_video_encoder: *mut obs::obs_encoder,
    pub stream_audio_encoder: *mut obs::obs_encoder,
    pub preview_view: *mut obs::obs_view,
    pub preview_texrender: *mut obs::gs_texrender_t,
    pub last_record_path: Option<PathBuf>,
}

pub struct SceneState {
    pub name: String,
    pub scene: *mut obs::obs_scene,
    pub scene_source: *mut obs::obs_source,
    pub item_accent: *mut obs::obs_scene_item,
    pub item_text: *mut obs::obs_scene_item,
    pub custom_items: HashMap<String, *mut obs::obs_scene_item>,
}

impl SceneState {
    pub fn new(name: String, scene: *mut obs::obs_scene, scene_source: *mut obs::obs_source) -> Self {
        SceneState {
            name,
            scene,
            scene_source,
            item_accent: std::ptr::null_mut(),
            item_text: std::ptr::null_mut(),
            custom_items: HashMap::new(),
        }
    }
}

pub struct ObsState {
    pub runtime: Mutex<ObsRuntime>,
}

unsafe impl Send for ObsRuntime {}
unsafe impl Sync for ObsRuntime {}
