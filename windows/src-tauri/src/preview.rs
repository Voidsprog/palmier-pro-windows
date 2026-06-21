use crate::edit::clips_at_frame;
use crate::ffmpeg::{extract_frame, extract_image, is_image_path, source_seconds_for_clip};
use crate::media::media_file_path;
use crate::state::EditorState;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

const CACHE_MAX: usize = 180;

static FFMPEG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static FRAME_CACHE: OnceLock<Mutex<FrameCache>> = OnceLock::new();

struct FrameCache {
    map: HashMap<String, Vec<u8>>,
    order: Vec<String>,
}

impl FrameCache {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            order: Vec::new(),
        }
    }

    fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.map.get(key).cloned()
    }

    fn insert(&mut self, key: String, value: Vec<u8>) {
        if self.map.contains_key(&key) {
            self.order.retain(|k| k != &key);
        }
        self.order.push(key.clone());
        self.map.insert(key, value);
        while self.order.len() > CACHE_MAX {
            if let Some(old) = self.order.first().cloned() {
                self.order.remove(0);
                self.map.remove(&old);
            }
        }
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

pub fn ffmpeg_guard() -> std::sync::MutexGuard<'static, ()> {
    FFMPEG_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

pub fn cache_key(media_ref: &str, source_frame: i64, w: i64, h: i64) -> String {
    format!("{media_ref}:{source_frame}:{w}x{h}")
}

pub fn get_cached(key: &str) -> Option<Vec<u8>> {
    FRAME_CACHE
        .get_or_init(|| Mutex::new(FrameCache::new()))
        .lock()
        .ok()?
        .get(key)
}

pub fn put_cache(key: String, data: Vec<u8>) {
    if let Ok(mut cache) = FRAME_CACHE
        .get_or_init(|| Mutex::new(FrameCache::new()))
        .lock()
    {
        cache.insert(key, data);
    }
}

pub fn clear_preview_cache() {
    if let Ok(mut cache) = FRAME_CACHE
        .get_or_init(|| Mutex::new(FrameCache::new()))
        .lock()
    {
        cache.clear();
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewContext {
    pub frame: i64,
    pub can_use_native_video: bool,
    pub media_path: Option<String>,
    pub source_seconds: f64,
    pub speed: f64,
    pub clip_start_frame: i64,
    pub clip_duration_frames: i64,
    pub is_image: bool,
    pub media_ref: Option<String>,
}

pub fn preview_context_at_frame(state: &EditorState, frame: i64) -> Result<PreviewContext, String> {
    let project_path = state.project_path.as_deref().ok_or("no project open")?;
    let root = std::path::Path::new(project_path);
    let fps = state.fps();

    let active = clips_at_frame(&state.timeline, frame);
    if active.len() != 1 {
        return Ok(PreviewContext {
            frame,
            can_use_native_video: false,
            media_path: None,
            source_seconds: 0.0,
            speed: 1.0,
            clip_start_frame: 0,
            clip_duration_frames: 0,
            is_image: false,
            media_ref: None,
        });
    }

    let (_track_idx, clip) = active[0].clone();
    let media_ref = clip
        .get("mediaRef")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let media_path = media_file_path(root, state.manifest.as_ref(), &media_ref);
    let start = clip.get("startFrame").and_then(|v| v.as_i64()).unwrap_or(0);
    let duration = clip.get("durationFrames").and_then(|v| v.as_i64()).unwrap_or(0);
    let speed = clip.get("speed").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let local = frame - start;
    let source_seconds = source_seconds_for_clip(&clip, local, fps);

    let media_type = clip
        .get("mediaType")
        .and_then(|v| v.as_str())
        .unwrap_or("video");

    let is_image = media_type == "image"
        || media_path
            .as_ref()
            .map(|p| is_image_path(p))
            .unwrap_or(false);

    let can_use_native_video = media_type == "video"
        && !is_image
        && media_path.as_ref().map(|p| p.exists()).unwrap_or(false);

    Ok(PreviewContext {
        frame,
        can_use_native_video,
        media_path: media_path.map(|p| p.to_string_lossy().into_owned()),
        source_seconds,
        speed,
        clip_start_frame: start,
        clip_duration_frames: duration,
        is_image,
        media_ref: Some(media_ref),
    })
}

pub fn render_preview_jpeg_cached(state: &EditorState, frame: i64) -> Result<Vec<u8>, String> {
    let ctx = preview_context_at_frame(state, frame)?;
    let w = state.width();
    let h = state.height();

    if let Some(media_ref) = &ctx.media_ref {
        let source_frame = (ctx.source_seconds * state.fps()).round() as i64;
        let key = cache_key(media_ref, source_frame, w, h);
        if let Some(cached) = get_cached(&key) {
            return Ok(cached);
        }
        let jpeg = render_preview_jpeg(state, frame)?;
        put_cache(key, jpeg.clone());
        return Ok(jpeg);
    }

    render_preview_jpeg(state, frame)
}

pub fn render_preview_jpeg(state: &EditorState, frame: i64) -> Result<Vec<u8>, String> {
    let project_path = state.project_path.as_deref().ok_or("no project open")?;
    let root = Path::new(project_path);
    let fps = state.fps();
    let w = state.width();
    let h = state.height();

    let active = clips_at_frame(&state.timeline, frame);
    if active.is_empty() {
        return render_black(w, h);
    }

    let (_track_idx, clip) = active.last().cloned().ok_or("no clip")?;
    let media_ref = clip.get("mediaRef").and_then(|v| v.as_str()).ok_or("no mediaRef")?;
    let media_path = media_file_path(root, state.manifest.as_ref(), media_ref)
        .ok_or_else(|| format!("media not found: {media_ref}"))?;

    let start = clip.get("startFrame").and_then(|v| v.as_i64()).unwrap_or(0);
    let local = frame - start;

    if is_image_path(&media_path) {
        extract_image(&media_path, w, h)
    } else {
        let secs = source_seconds_for_clip(&clip, local, fps);
        extract_frame(&media_path, secs, w, h)
    }
}

fn render_black(w: i64, h: i64) -> Result<Vec<u8>, String> {
    let _guard = ffmpeg_guard();
    let ffmpeg_path = crate::ffmpeg::find_ffmpeg()?;
    let output = std::process::Command::new(&ffmpeg_path)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            &format!("color=c=black:s={w}x{h}:d=0.04"),
            "-vframes",
            "1",
            "-f",
            "image2pipe",
            "-vcodec",
            "mjpeg",
            "pipe:1",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    Ok(output.stdout)
}
