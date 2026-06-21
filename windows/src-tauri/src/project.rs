use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const TIMELINE_FILENAME: &str = "project.json";
pub const MANIFEST_FILENAME: &str = "media.json";
#[allow(dead_code)]
pub const GENERATION_LOG_FILENAME: &str = "generation-log.json";
#[allow(dead_code)]
pub const THUMBNAIL_FILENAME: &str = "thumbnail.jpg";
#[allow(dead_code)]
pub const MEDIA_DIR: &str = "media";

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("project not found: {0}")]
    NotFound(String),
    #[error("invalid timeline JSON: {0}")]
    InvalidTimeline(String),
    #[error("invalid manifest JSON: {0}")]
    InvalidManifest(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPackage {
    pub path: String,
    pub timeline: serde_json::Value,
    pub manifest: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub path: String,
    pub name: String,
    pub fps: i64,
    pub width: i64,
    pub height: i64,
    pub track_count: usize,
    pub clip_count: usize,
    pub media_count: usize,
    pub total_frames: i64,
}

pub fn default_storage_directory() -> String {
    let docs = dirs_documents().unwrap_or_else(|| PathBuf::from("."));
    docs.join("Palmier Pro").to_string_lossy().into_owned()
}

pub fn ensure_storage_directory() {
    let path = default_storage_directory();
    let _ = fs::create_dir_all(path);
}

fn dirs_documents() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE")
            .ok()
            .map(|home| PathBuf::from(home).join("Documents"))
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join("Documents"))
    }
}

fn package_root(path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_dir() {
        p.to_path_buf()
    } else {
        p.parent().unwrap_or(p).to_path_buf()
    }
}

fn read_json_file(root: &Path, filename: &str) -> Result<Option<serde_json::Value>, ProjectError> {
    let file = root.join(filename);
    if !file.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&file)?;
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|e| ProjectError::InvalidTimeline(e.to_string()))
}

impl ProjectPackage {
    pub fn load(path: &str) -> Result<Self, ProjectError> {
        let root = package_root(path);
        if !root.exists() {
            return Err(ProjectError::NotFound(path.to_string()));
        }

        let timeline_path = root.join(TIMELINE_FILENAME);
        if !timeline_path.exists() {
            return Err(ProjectError::NotFound(format!(
                "missing {TIMELINE_FILENAME} in {}",
                root.display()
            )));
        }

        let timeline_raw = fs::read_to_string(&timeline_path)?;
        let timeline: serde_json::Value = serde_json::from_str(&timeline_raw)
            .map_err(|e| ProjectError::InvalidTimeline(e.to_string()))?;

        let manifest = match read_json_file(&root, MANIFEST_FILENAME)? {
            Some(v) => Some(v),
            None => None,
        };

        Ok(Self {
            path: root.to_string_lossy().into_owned(),
            timeline,
            manifest,
        })
    }

    pub fn save(path: &str, timeline_json: &str, manifest_json: Option<&str>) -> Result<(), ProjectError> {
        let root = package_root(path);
        fs::create_dir_all(&root)?;

        let _: serde_json::Value = serde_json::from_str(timeline_json)
            .map_err(|e| ProjectError::InvalidTimeline(e.to_string()))?;
        fs::write(root.join(TIMELINE_FILENAME), timeline_json)?;

        if let Some(manifest) = manifest_json {
            let _: serde_json::Value = serde_json::from_str(manifest)
                .map_err(|e| ProjectError::InvalidManifest(e.to_string()))?;
            fs::write(root.join(MANIFEST_FILENAME), manifest)?;
        }

        Ok(())
    }

    pub fn summary(path: &str) -> Result<ProjectSummary, ProjectError> {
        let package = Self::load(path)?;
        let timeline = &package.timeline;

        let fps = timeline.get("fps").and_then(|v| v.as_i64()).unwrap_or(30);
        let width = timeline.get("width").and_then(|v| v.as_i64()).unwrap_or(1920);
        let height = timeline.get("height").and_then(|v| v.as_i64()).unwrap_or(1080);

        let tracks = timeline
            .get("tracks")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let track_count = tracks.len();
        let mut clip_count = 0usize;
        let mut total_frames = 0i64;

        for track in &tracks {
            if let Some(clips) = track.get("clips").and_then(|v| v.as_array()) {
                clip_count += clips.len();
                for clip in clips {
                    if let Some(end) = clip.get("endFrame").and_then(|v| v.as_i64()) {
                        total_frames = total_frames.max(end);
                    } else if let (Some(start), Some(duration)) = (
                        clip.get("startFrame").and_then(|v| v.as_i64()),
                        clip.get("durationFrames").and_then(|v| v.as_i64()),
                    ) {
                        total_frames = total_frames.max(start + duration);
                    }
                }
            }
        }

        let media_count = package
            .manifest
            .as_ref()
            .and_then(|m| m.get("entries"))
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);

        let name = Path::new(&package.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled Project")
            .trim_end_matches(".palmier")
            .to_string();

        Ok(ProjectSummary {
            path: package.path,
            name,
            fps,
            width,
            height,
            track_count,
            clip_count,
            media_count,
            total_frames,
        })
    }
}
