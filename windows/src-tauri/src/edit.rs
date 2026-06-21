use serde_json::{json, Value};
use uuid::Uuid;

pub fn move_clip(timeline: &mut Value, clip_id: &str, new_start: i64) -> Result<(), String> {
    let tracks = timeline
        .get_mut("tracks")
        .and_then(|v| v.as_array_mut())
        .ok_or("no tracks")?;

    for track in tracks.iter_mut() {
        if let Some(clips) = track.get_mut("clips").and_then(|v| v.as_array_mut()) {
            for clip in clips.iter_mut() {
                if clip.get("id").and_then(|v| v.as_str()) == Some(clip_id) {
                    clip["startFrame"] = json!(new_start.max(0));
                    return Ok(());
                }
            }
        }
    }
    Err(format!("clip not found: {clip_id}"))
}

pub fn remove_clip(timeline: &mut Value, clip_id: &str) -> Result<(), String> {
    let tracks = timeline
        .get_mut("tracks")
        .and_then(|v| v.as_array_mut())
        .ok_or("no tracks")?;

    for track in tracks.iter_mut() {
        if let Some(clips) = track.get_mut("clips").and_then(|v| v.as_array_mut()) {
            let before = clips.len();
            clips.retain(|c| c.get("id").and_then(|v| v.as_str()) != Some(clip_id));
            if clips.len() < before {
                return Ok(());
            }
        }
    }
    Err(format!("clip not found: {clip_id}"))
}

pub fn set_clip_properties(timeline: &mut Value, clip_id: &str, props: &Value) -> Result<(), String> {
    let tracks = timeline
        .get_mut("tracks")
        .and_then(|v| v.as_array_mut())
        .ok_or("no tracks")?;

    for track in tracks.iter_mut() {
        if let Some(clips) = track.get_mut("clips").and_then(|v| v.as_array_mut()) {
            for clip in clips.iter_mut() {
                if clip.get("id").and_then(|v| v.as_str()) == Some(clip_id) {
                    if let Some(obj) = props.as_object() {
                        for (k, v) in obj {
                            clip[k] = v.clone();
                        }
                    }
                    return Ok(());
                }
            }
        }
    }
    Err(format!("clip not found: {clip_id}"))
}

pub fn split_clip(timeline: &mut Value, clip_id: &str, at_frame: i64) -> Result<String, String> {
    let tracks = timeline
        .get_mut("tracks")
        .and_then(|v| v.as_array_mut())
        .ok_or("no tracks")?;

    for track in tracks.iter_mut() {
        if let Some(clips) = track.get_mut("clips").and_then(|v| v.as_array_mut()) {
            if let Some(idx) = clips.iter().position(|c| c.get("id").and_then(|v| v.as_str()) == Some(clip_id)) {
                let clip = clips[idx].clone();
                let start = clip.get("startFrame").and_then(|v| v.as_i64()).unwrap_or(0);
                let duration = clip.get("durationFrames").and_then(|v| v.as_i64()).unwrap_or(0);
                let end = start + duration;

                if at_frame <= start || at_frame >= end {
                    return Err("split frame must be inside clip".into());
                }

                let left_duration = at_frame - start;
                let right_duration = end - at_frame;
                let speed = clip.get("speed").and_then(|v| v.as_f64()).unwrap_or(1.0);
                let trim_start = clip.get("trimStartFrame").and_then(|v| v.as_i64()).unwrap_or(0);
                let left_trim_end = trim_start + ((left_duration as f64 * speed).round() as i64);

                clips[idx]["durationFrames"] = json!(left_duration);

                let mut right = clip;
                right["id"] = json!(Uuid::new_v4().to_string());
                right["startFrame"] = json!(at_frame);
                right["durationFrames"] = json!(right_duration);
                right["trimStartFrame"] = json!(left_trim_end);

                let new_id = right["id"].as_str().unwrap_or("").to_string();
                clips.insert(idx + 1, right);
                return Ok(new_id);
            }
        }
    }
    Err(format!("clip not found: {clip_id}"))
}

pub fn add_clips(timeline: &mut Value, entries: &[Value]) -> Result<Vec<String>, String> {
    let mut created = Vec::new();
    let tracks = timeline
        .get_mut("tracks")
        .and_then(|v| v.as_array_mut())
        .ok_or("no tracks")?;

    for entry in entries {
        let media_ref = entry.get("mediaRef").and_then(|v| v.as_str()).ok_or("mediaRef required")?;
        let start = entry.get("startFrame").and_then(|v| v.as_i64()).ok_or("startFrame required")?;
        let duration = entry.get("durationFrames").and_then(|v| v.as_i64()).ok_or("durationFrames required")?;
        let track_index = entry.get("trackIndex").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let track_type = entry
            .get("trackType")
            .and_then(|v| v.as_str())
            .unwrap_or("video")
            .to_string();

        while tracks.len() <= track_index {
            tracks.push(json!({
                "id": Uuid::new_v4().to_string(),
                "type": "video",
                "clips": []
            }));
        }

        let clip_id = Uuid::new_v4().to_string();
        let effective_type = tracks[track_index]
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or(&track_type);

        let clip = json!({
            "id": clip_id,
            "mediaRef": media_ref,
            "mediaType": effective_type,
            "sourceClipType": effective_type,
            "startFrame": start,
            "durationFrames": duration,
            "trimStartFrame": entry.get("trimStartFrame").cloned().unwrap_or(json!(0)),
            "trimEndFrame": entry.get("trimEndFrame").cloned().unwrap_or(json!(0)),
            "speed": 1.0,
            "volume": 1.0,
            "opacity": 1.0
        });

        if let Some(clips) = tracks[track_index].get_mut("clips").and_then(|v| v.as_array_mut()) {
            clips.push(clip);
            created.push(clip_id);
        }
    }
    Ok(created)
}

pub fn find_clip(timeline: &Value, clip_id: &str) -> Option<Value> {
    let tracks = timeline.get("tracks")?.as_array()?;
    for track in tracks {
        let clips = track.get("clips")?.as_array()?;
        for clip in clips {
            if clip.get("id")?.as_str()? == clip_id {
                return Some(clip.clone());
            }
        }
    }
    None
}

pub fn clips_at_frame(timeline: &Value, frame: i64) -> Vec<(usize, Value)> {
    let mut result = Vec::new();
    let Some(tracks) = timeline.get("tracks").and_then(|v| v.as_array()) else {
        return result;
    };

    for (ti, track) in tracks.iter().enumerate() {
        if track.get("hidden").and_then(|v| v.as_bool()) == Some(true) {
            continue;
        }
        let Some(clips) = track.get("clips").and_then(|v| v.as_array()) else {
            continue;
        };
        for clip in clips {
            let start = clip.get("startFrame").and_then(|v| v.as_i64()).unwrap_or(0);
            let dur = clip.get("durationFrames").and_then(|v| v.as_i64()).unwrap_or(0);
            if frame >= start && frame < start + dur {
                let media_type = clip
                    .get("mediaType")
                    .and_then(|v| v.as_str())
                    .or_else(|| track.get("type").and_then(|v| v.as_str()))
                    .unwrap_or("video");
                if media_type == "video" || media_type == "image" {
                    result.push((ti, clip.clone()));
                }
            }
        }
    }
    result
}
