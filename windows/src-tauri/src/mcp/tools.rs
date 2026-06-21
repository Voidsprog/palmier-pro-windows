use crate::edit::{add_clips, move_clip, remove_clip, set_clip_properties, split_clip};
use crate::export::{timeline_summary_json, export_timeline};
use crate::media::list_media;
use crate::project::ProjectPackage;
use crate::state::{AppState, EditorState};
use serde_json::{json, Value};

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "get_timeline",
            "description": "Returns project settings, tracks, and clips.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_media",
            "description": "Returns all media entries in the project manifest.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "import_media",
            "description": "Import local media files into the project library. source.path must be an absolute file path.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Absolute path to a media file" }
                        },
                        "required": ["path"]
                    },
                    "name": { "type": "string" }
                },
                "required": ["source"]
            }
        }),
        json!({
            "name": "add_clips",
            "description": "Add clips to the timeline.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entries": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "mediaRef": { "type": "string" },
                                "trackIndex": { "type": "integer" },
                                "startFrame": { "type": "integer" },
                                "durationFrames": { "type": "integer" },
                                "trimStartFrame": { "type": "integer" },
                                "trimEndFrame": { "type": "integer" }
                            },
                            "required": ["mediaRef", "startFrame", "durationFrames"]
                        }
                    }
                },
                "required": ["entries"]
            }
        }),
        json!({
            "name": "remove_clips",
            "description": "Remove clips by ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "clipIds": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["clipIds"]
            }
        }),
        json!({
            "name": "move_clips",
            "description": "Move a clip to a new start frame.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "clipId": { "type": "string" },
                    "startFrame": { "type": "integer" }
                },
                "required": ["clipId", "startFrame"]
            }
        }),
        json!({
            "name": "set_clip_properties",
            "description": "Update clip properties.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "clipId": { "type": "string" },
                    "properties": { "type": "object" }
                },
                "required": ["clipId", "properties"]
            }
        }),
        json!({
            "name": "split_clip",
            "description": "Split a clip at a project frame.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "clipId": { "type": "string" },
                    "atFrame": { "type": "integer" }
                },
                "required": ["clipId", "atFrame"]
            }
        }),
        json!({
            "name": "undo",
            "description": "Undo the last timeline edit.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "export_video",
            "description": "Export timeline to H.264 or H.265 MP4.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "outputPath": { "type": "string" },
                    "codec": { "type": "string", "enum": ["h264", "h265"] }
                },
                "required": ["outputPath"]
            }
        }),
    ]
}

pub fn call_tool(state: &AppState, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "get_timeline" => {
            let s = state.lock().map_err(|e| e.to_string())?;
            Ok(timeline_summary_json(&s))
        }
        "get_media" => {
            let s = state.lock().map_err(|e| e.to_string())?;
            Ok(json!({ "entries": list_media(s.manifest.as_ref()) }))
        }
        "import_media" => {
            let source = args.get("source").ok_or("source required")?;
            let path = source
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("source.path required")?;
            let mut s = state.lock().map_err(|e| e.to_string())?;
            let ids = crate::import_media::import_paths(&mut s, &[path.to_string()])?;
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                if let Some(manifest) = s.manifest.as_mut() {
                    if let Some(entries) = manifest.get_mut("entries").and_then(|v| v.as_array_mut()) {
                        if let Some(last) = entries.last_mut() {
                            last["name"] = json!(name);
                        }
                    }
                }
                persist(&s)?;
            }
            Ok(json!({ "mediaRef": ids.first().cloned().unwrap_or_default(), "ids": ids }))
        }
        "add_clips" => {
            let entries = args
                .get("entries")
                .and_then(|v| v.as_array())
                .ok_or("entries required")?
                .clone();
            let mut s = state.lock().map_err(|e| e.to_string())?;
            s.push_undo();
            let ids = add_clips(&mut s.timeline, &entries)?;
            persist(&s)?;
            Ok(json!({ "clipIds": ids }))
        }
        "remove_clips" => {
            let ids = args
                .get("clipIds")
                .and_then(|v| v.as_array())
                .ok_or("clipIds required")?;
            let mut s = state.lock().map_err(|e| e.to_string())?;
            s.push_undo();
            for id in ids {
                if let Some(cid) = id.as_str() {
                    remove_clip(&mut s.timeline, cid)?;
                }
            }
            persist(&s)?;
            Ok(json!({ "ok": true }))
        }
        "move_clips" => {
            let clip_id = args.get("clipId").and_then(|v| v.as_str()).ok_or("clipId required")?;
            let start = args.get("startFrame").and_then(|v| v.as_i64()).ok_or("startFrame required")?;
            let mut s = state.lock().map_err(|e| e.to_string())?;
            s.push_undo();
            move_clip(&mut s.timeline, clip_id, start)?;
            persist(&s)?;
            Ok(json!({ "ok": true }))
        }
        "set_clip_properties" => {
            let clip_id = args.get("clipId").and_then(|v| v.as_str()).ok_or("clipId required")?;
            let props = args.get("properties").ok_or("properties required")?;
            let mut s = state.lock().map_err(|e| e.to_string())?;
            s.push_undo();
            set_clip_properties(&mut s.timeline, clip_id, props)?;
            persist(&s)?;
            Ok(json!({ "ok": true }))
        }
        "split_clip" => {
            let clip_id = args.get("clipId").and_then(|v| v.as_str()).ok_or("clipId required")?;
            let at = args.get("atFrame").and_then(|v| v.as_i64()).ok_or("atFrame required")?;
            let mut s = state.lock().map_err(|e| e.to_string())?;
            s.push_undo();
            let new_id = split_clip(&mut s.timeline, clip_id, at)?;
            persist(&s)?;
            Ok(json!({ "newClipId": new_id }))
        }
        "undo" => {
            let mut s = state.lock().map_err(|e| e.to_string())?;
            let ok = s.undo();
            if ok {
                persist(&s)?;
            }
            Ok(json!({ "undone": ok }))
        }
        "export_video" => {
            let output = args.get("outputPath").and_then(|v| v.as_str()).ok_or("outputPath required")?;
            let codec = args.get("codec").and_then(|v| v.as_str()).unwrap_or("h264");
            let s = state.lock().map_err(|e| e.to_string())?;
            export_timeline(&s, std::path::Path::new(output), codec)?;
            Ok(json!({ "outputPath": output, "codec": codec }))
        }
        _ => Err(format!("unknown tool: {name}")),
    }
}

fn persist(state: &EditorState) -> Result<(), String> {
    let path = state.project_path.as_deref().ok_or("no project open")?;
    let timeline = serde_json::to_string_pretty(&state.timeline).map_err(|e| e.to_string())?;
    let manifest = state
        .manifest
        .as_ref()
        .map(|m| serde_json::to_string_pretty(m).unwrap_or_default());
    ProjectPackage::save(path, &timeline, manifest.as_deref()).map_err(|e| e.to_string())
}

pub fn tool_result_text(value: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
        "isError": false
    })
}

pub fn tool_result_error(msg: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": msg }],
        "isError": true
    })
}
