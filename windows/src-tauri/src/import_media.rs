use crate::project::{ProjectPackage, MEDIA_DIR, MANIFEST_FILENAME, TIMELINE_FILENAME};
use crate::state::EditorState;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const IMAGE_DURATION_SECS: f64 = 5.0;

pub fn clip_type_from_ext(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        "mov" | "mp4" | "m4v" => Some("video"),
        "mp3" | "wav" | "aac" | "m4a" => Some("audio"),
        "png" | "jpg" | "jpeg" | "tiff" | "heic" | "webp" => Some("image"),
        "json" | "lottie" => Some("lottie"),
        _ => None,
    }
}

pub fn import_paths(state: &mut EditorState, paths: &[String]) -> Result<Vec<String>, String> {
    let project_path = state.project_path.clone().ok_or("Open or create a project first")?;
    let root = PathBuf::from(&project_path);
    fs::create_dir_all(root.join(MEDIA_DIR)).map_err(|e| e.to_string())?;

    if state.manifest.is_none() {
        state.manifest = Some(json!({ "version": 2, "entries": [], "folders": [] }));
    }

    let mut imported = Vec::new();
    for path_str in paths {
        match import_one(&root, path_str) {
            Ok(entry) => {
                imported.push(entry.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string());
                if let Some(manifest) = state.manifest.as_mut() {
                    if let Some(entries) = manifest.get_mut("entries").and_then(|v| v.as_array_mut()) {
                        entries.push(entry);
                    }
                }
            }
            Err(e) => return Err(format!("{}: {e}", Path::new(path_str).file_name().and_then(|n| n.to_str()).unwrap_or(path_str))),
        }
    }

    persist_manifest(state)?;
    Ok(imported)
}

fn import_one(project_root: &Path, path_str: &str) -> Result<Value, String> {
    let source_path = PathBuf::from(path_str);
    if !source_path.exists() {
        return Err(format!("file not found: {path_str}"));
    }

    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or("missing file extension")?;
    let clip_type = clip_type_from_ext(ext).ok_or(format!("unsupported extension .{ext}"))?;

    let name = source_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled")
        .to_string();

    let duration = probe_duration(&source_path, clip_type)?;

    let source = if source_path.starts_with(project_root) {
        let rel = source_path
            .strip_prefix(project_root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        json!({ "project": { "relativePath": rel } })
    } else {
        json!({ "external": { "absolutePath": source_path.to_string_lossy() } })
    };

    Ok(json!({
        "id": Uuid::new_v4().to_string(),
        "name": name,
        "type": clip_type,
        "source": source,
        "duration": duration
    }))
}

fn probe_duration(path: &Path, clip_type: &str) -> Result<f64, String> {
    if clip_type == "image" {
        return Ok(IMAGE_DURATION_SECS);
    }
    if clip_type == "lottie" {
        return Ok(1.0);
    }

    let ffprobe = find_ffprobe()?;
    let output = std::process::Command::new(&ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            path.to_str().ok_or("invalid path")?,
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Ok(d) = s.parse::<f64>() {
            if d > 0.0 {
                return Ok(d);
            }
        }
    }
    Ok(1.0)
}

fn find_ffprobe() -> Result<PathBuf, String> {
    if let Ok(p) = which_bin("ffprobe") {
        return Ok(p);
    }
    if let Ok(ffmpeg) = crate::ffmpeg::find_ffmpeg() {
        let ffprobe = ffmpeg.with_file_name(
            ffmpeg
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.replacen("ffmpeg", "ffprobe", 1))
                .unwrap_or_else(|| "ffprobe.exe".into()),
        );
        if ffprobe.exists() {
            return Ok(ffprobe);
        }
    }
    Err("ffprobe not found (install FFmpeg)".into())
}

fn which_bin(name: &str) -> Result<PathBuf, String> {
    let output = std::process::Command::new("where")
        .arg(name)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("not in PATH".into());
    }
    let line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if line.is_empty() {
        return Err("empty".into());
    }
    Ok(PathBuf::from(line))
}

fn persist_manifest(state: &EditorState) -> Result<(), String> {
    let path = state.project_path.as_deref().ok_or("no project")?;
    let manifest = state.manifest.as_ref().ok_or("no manifest")?;
    let json = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    ProjectPackage::save(path, &serde_json::to_string_pretty(&state.timeline).map_err(|e| e.to_string())?, Some(&json))
        .map_err(|e| e.to_string())
}

pub fn create_project(name: &str) -> Result<ProjectPackage, String> {
    let safe_name = if name.trim().is_empty() {
        "Untitled Project".to_string()
    } else {
        name.trim().to_string()
    };
    let dir = crate::project::default_storage_directory();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut project_dir = PathBuf::from(&dir).join(&safe_name);
    if project_dir.exists() {
        project_dir = PathBuf::from(&dir).join(format!("{safe_name}-{}", &Uuid::new_v4().to_string()[..8]));
    }
    fs::create_dir_all(&project_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(project_dir.join(MEDIA_DIR)).map_err(|e| e.to_string())?;

    let timeline = json!({
        "fps": 30,
        "width": 1920,
        "height": 1080,
        "settingsConfigured": false,
        "tracks": []
    });
    let manifest = json!({ "version": 2, "entries": [], "folders": [] });

    fs::write(
        project_dir.join(TIMELINE_FILENAME),
        serde_json::to_string_pretty(&timeline).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        project_dir.join(MANIFEST_FILENAME),
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    ProjectPackage::load(project_dir.to_str().ok_or("invalid project path")?)
        .map_err(|e| e.to_string())
}

pub fn add_imported_to_timeline(state: &mut EditorState, media_ref: &str, start_frame: i64) -> Result<String, String> {
    let entry = state
        .manifest
        .as_ref()
        .and_then(|m| m.get("entries"))
        .and_then(|e| e.as_array())
        .and_then(|entries| entries.iter().find(|e| e.get("id").and_then(|v| v.as_str()) == Some(media_ref)))
        .cloned()
        .ok_or("media not found")?;

    let clip_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("video");
    let duration_secs = entry.get("duration").and_then(|v| v.as_f64()).unwrap_or(5.0);
    let fps = state.fps();
    let duration_frames = ((duration_secs * fps).round() as i64).max(1);

    state.push_undo();

    let tracks = state
        .timeline
        .get_mut("tracks")
        .and_then(|v| v.as_array_mut())
        .ok_or("no tracks")?;

    let track_index = tracks
        .iter()
        .position(|t| t.get("type").and_then(|v| v.as_str()) == Some(clip_type))
        .unwrap_or_else(|| {
            tracks.push(json!({
                "id": Uuid::new_v4().to_string(),
                "type": clip_type,
                "clips": []
            }));
            tracks.len() - 1
        });

    let clip_id = Uuid::new_v4().to_string();
    let clip = json!({
        "id": clip_id,
        "mediaRef": media_ref,
        "mediaType": clip_type,
        "sourceClipType": clip_type,
        "startFrame": start_frame,
        "durationFrames": duration_frames,
        "trimStartFrame": 0,
        "trimEndFrame": 0,
        "speed": 1.0,
        "volume": 1.0,
        "opacity": 1.0
    });

    if let Some(clips) = tracks[track_index].get_mut("clips").and_then(|v| v.as_array_mut()) {
        clips.push(clip);
    }

    persist_manifest(state)?;
    Ok(clip_id)
}
