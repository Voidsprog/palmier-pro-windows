use crate::project::ProjectPackage;
use serde_json::Value;
use std::sync::Mutex;

pub struct EditorState {
    pub project_path: Option<String>,
    pub timeline: Value,
    pub manifest: Option<Value>,
    undo_stack: Vec<Value>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            project_path: None,
            timeline: serde_json::json!({ "fps": 30, "width": 1920, "height": 1080, "tracks": [] }),
            manifest: None,
            undo_stack: Vec::new(),
        }
    }
}

impl EditorState {
    pub fn load_package(&mut self, package: ProjectPackage) {
        self.project_path = Some(package.path);
        self.timeline = package.timeline;
        self.manifest = package.manifest;
        self.undo_stack.clear();
    }

    pub fn push_undo(&mut self) {
        self.undo_stack.push(self.timeline.clone());
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            self.timeline = prev;
            true
        } else {
            false
        }
    }

    pub fn fps(&self) -> f64 {
        self.timeline.get("fps").and_then(|v| v.as_f64()).unwrap_or(30.0)
    }

    pub fn width(&self) -> i64 {
        self.timeline.get("width").and_then(|v| v.as_i64()).unwrap_or(1920)
    }

    pub fn height(&self) -> i64 {
        self.timeline.get("height").and_then(|v| v.as_i64()).unwrap_or(1080)
    }

    pub fn total_frames(&self) -> i64 {
        let mut max_frame = 0i64;
        if let Some(tracks) = self.timeline.get("tracks").and_then(|v| v.as_array()) {
            for track in tracks {
                if let Some(clips) = track.get("clips").and_then(|v| v.as_array()) {
                    for clip in clips {
                        let start = clip.get("startFrame").and_then(|v| v.as_i64()).unwrap_or(0);
                        let dur = clip.get("durationFrames").and_then(|v| v.as_i64()).unwrap_or(0);
                        max_frame = max_frame.max(start + dur);
                    }
                }
            }
        }
        max_frame
    }
}

pub type AppState = std::sync::Arc<Mutex<EditorState>>;
