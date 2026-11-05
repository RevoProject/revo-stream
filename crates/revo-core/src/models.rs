use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub source_type: String,
    #[serde(default)]
    pub audio_capable: bool,
    pub params: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SourceTypeItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub audio_capable: bool,
}

#[derive(Clone, Serialize)]
pub struct ReusableSourceInfo {
    pub name: String,
    pub source_type: String,
}

#[derive(Serialize)]
pub struct SourcePropertyOption {
    pub value: String,
    pub label: String,
}

#[derive(Serialize)]
pub struct SourcePropertySpec {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub hint: String,
    pub options: Vec<SourcePropertyOption>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PlannerInit {
    pub sources: Vec<SourceInfo>,
    pub scene_resolution: String,
    pub selected_source_id: Option<String>,
    #[serde(default)]
    pub scene_name: Option<String>,
}

#[derive(Deserialize)]
pub struct PlannerInitInput {
    pub sources: Vec<SourceInfo>,
    pub scene_resolution: String,
    pub selected_source_id: Option<String>,
    #[serde(default)]
    pub scene_name: Option<String>,
}

#[derive(Deserialize)]
pub struct SourceUpdate {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub params: HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct SourceCreate {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub params: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SourceFilterItem {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub params: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn source_type_defaults_audio_capability() {
        let item: SourceTypeItem = serde_json::from_value(json!({
            "id": "image_source",
            "label": "Image"
        }))
        .expect("source type should deserialize");

        assert!(!item.audio_capable);
    }

    #[test]
    fn source_filter_defaults_optional_fields() {
        let filter: SourceFilterItem = serde_json::from_value(json!({
            "id": "filter-1",
            "name": "Color Correction",
            "kind": "color_filter",
            "enabled": true
        }))
        .expect("source filter should deserialize");

        assert!(!filter.locked);
        assert!(filter.params.is_empty());
    }

    #[test]
    fn planner_init_defaults_scene_name() {
        let planner: PlannerInit = serde_json::from_value(json!({
            "sources": [],
            "scene_resolution": "1920x1080",
            "selected_source_id": null
        }))
        .expect("planner state should deserialize");

        assert!(planner.scene_name.is_none());
    }

    #[test]
    fn source_info_preserves_wire_shape() {
        let source = SourceInfo {
            id: "camera-1".into(),
            name: "Camera".into(),
            visible: true,
            source_type: "v4l2_input".into(),
            audio_capable: false,
            params: HashMap::from([("device_id".into(), "/dev/video0".into())]),
        };

        assert_eq!(
            serde_json::to_value(source).expect("source should serialize"),
            json!({
                "id": "camera-1",
                "name": "Camera",
                "visible": true,
                "source_type": "v4l2_input",
                "audio_capable": false,
                "params": { "device_id": "/dev/video0" }
            })
        );
    }

    #[test]
    fn source_commands_keep_snake_case_payloads() {
        let payload = json!({
            "id": "source-1",
            "name": "Source",
            "source_type": "image_source",
            "params": { "file": "/tmp/image.png" }
        });

        let create: SourceCreate =
            serde_json::from_value(payload.clone()).expect("create payload should deserialize");
        let update: SourceUpdate =
            serde_json::from_value(payload).expect("update payload should deserialize");

        assert_eq!(create.source_type, "image_source");
        assert_eq!(
            update.params.get("file").map(String::as_str),
            Some("/tmp/image.png")
        );
    }

    #[test]
    fn planner_payload_round_trips_without_field_changes() {
        let payload = json!({
            "sources": [{
                "id": "source-1",
                "name": "Source",
                "visible": true,
                "source_type": "image_source",
                "audio_capable": false,
                "params": {}
            }],
            "scene_resolution": "1280x720",
            "selected_source_id": "source-1",
            "scene_name": "Scene 1"
        });
        let planner: PlannerInit =
            serde_json::from_value(payload.clone()).expect("planner payload should deserialize");
        let serialized: Value =
            serde_json::to_value(planner).expect("planner payload should serialize");

        assert_eq!(serialized, payload);
    }
}
