mod edit;
mod export;
mod ffmpeg;
mod import_media;
mod mcp;
mod media;
mod preview;
mod project;
mod state;

use base64::{engine::general_purpose::STANDARD, Engine};
use export::export_timeline;
use preview::{preview_context_at_frame, render_preview_jpeg_cached, clear_preview_cache, PreviewContext};
use project::{ProjectPackage, ProjectSummary};
use serde_json::{json, Value};
use state::{AppState, EditorState};
use tauri::State;

fn lock_state(state: &AppState) -> Result<std::sync::MutexGuard<'_, EditorState>, String> {
    state.lock().map_err(|e| e.to_string())
}

fn lock_state_mut(state: &AppState) -> Result<std::sync::MutexGuard<'_, EditorState>, String> {
    state.lock().map_err(|e| e.to_string())
}

fn with_state<F, T>(state: State<'_, AppState>, f: F) -> Result<T, String>
where
    F: FnOnce(&EditorState) -> Result<T, String>,
{
    let guard = lock_state(state.inner())?;
    f(&guard)
}

fn with_state_mut<F, T>(state: State<'_, AppState>, f: F) -> Result<T, String>
where
    F: FnOnce(&mut EditorState) -> Result<T, String>,
{
    let mut guard = lock_state_mut(state.inner())?;
    f(&mut guard)
}

#[tauri::command]
fn open_project(path: String, state: State<'_, AppState>) -> Result<ProjectPackage, String> {
    let package = ProjectPackage::load(&path).map_err(|e| e.to_string())?;
    clear_preview_cache();
    with_state_mut(state, |s| {
        s.load_package(package.clone());
        Ok(())
    })?;
    Ok(package)
}

#[tauri::command]
fn save_project(
    path: String,
    timeline_json: String,
    manifest_json: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    ProjectPackage::save(&path, &timeline_json, manifest_json.as_deref())
        .map_err(|e| e.to_string())?;
    with_state_mut(state, |s| {
        s.timeline = serde_json::from_str(&timeline_json).map_err(|e| e.to_string())?;
        if let Some(m) = manifest_json {
            s.manifest = Some(serde_json::from_str(&m).map_err(|e| e.to_string())?);
        }
        s.project_path = Some(path);
        Ok(())
    })
}

#[tauri::command]
fn project_summary(path: String) -> Result<ProjectSummary, String> {
    ProjectPackage::summary(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn storage_directory() -> String {
    project::default_storage_directory()
}

#[tauri::command]
async fn render_preview_frame(frame: i64, state: State<'_, AppState>) -> Result<String, String> {
    let app = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = lock_state(&app)?;
        let jpeg = render_preview_jpeg_cached(&guard, frame)?;
        Ok(STANDARD.encode(jpeg))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_preview_context(frame: i64, state: State<'_, AppState>) -> Result<PreviewContext, String> {
    with_state(state, |s| preview_context_at_frame(s, frame))
}

#[tauri::command]
async fn export_video(output_path: String, codec: String, state: State<'_, AppState>) -> Result<(), String> {
    let app = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = lock_state(&app)?;
        export_timeline(&guard, std::path::Path::new(&output_path), &codec)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_timeline(state: State<'_, AppState>) -> Result<Value, String> {
    with_state(state, |s| Ok(export::timeline_summary_json(s)))
}

#[tauri::command]
fn move_clip_cmd(clip_id: String, start_frame: i64, state: State<'_, AppState>) -> Result<(), String> {
    with_state_mut(state, |s| {
        s.push_undo();
        edit::move_clip(&mut s.timeline, &clip_id, start_frame)?;
        clear_preview_cache();
        persist_state(s)
    })
}

#[tauri::command]
fn remove_clip_cmd(clip_id: String, state: State<'_, AppState>) -> Result<(), String> {
    with_state_mut(state, |s| {
        s.push_undo();
        edit::remove_clip(&mut s.timeline, &clip_id)?;
        clear_preview_cache();
        persist_state(s)
    })
}

#[tauri::command]
fn set_clip_properties_cmd(
    clip_id: String,
    properties: Value,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_state_mut(state, |s| {
        s.push_undo();
        edit::set_clip_properties(&mut s.timeline, &clip_id, &properties)?;
        clear_preview_cache();
        persist_state(s)
    })
}

#[tauri::command]
fn split_clip_cmd(clip_id: String, at_frame: i64, state: State<'_, AppState>) -> Result<String, String> {
    with_state_mut(state, |s| {
        s.push_undo();
        let new_id = edit::split_clip(&mut s.timeline, &clip_id, at_frame)?;
        clear_preview_cache();
        persist_state(s)?;
        Ok(new_id)
    })
}

#[tauri::command]
fn undo_cmd(state: State<'_, AppState>) -> Result<bool, String> {
    with_state_mut(state, |s| {
        let ok = s.undo();
        if ok {
            clear_preview_cache();
            persist_state(s)?;
        }
        Ok(ok)
    })
}

#[tauri::command]
fn mcp_status() -> bool {
    true
}

#[tauri::command]
fn create_project(name: String, state: State<'_, AppState>) -> Result<ProjectPackage, String> {
    let package = import_media::create_project(&name)?;
    clear_preview_cache();
    with_state_mut(state, |s| {
        s.load_package(package.clone());
        Ok(())
    })?;
    Ok(package)
}

#[tauri::command]
fn import_media_files(paths: Vec<String>, state: State<'_, AppState>) -> Result<Vec<String>, String> {
    with_state_mut(state, |s| {
        let ids = import_media::import_paths(s, &paths)?;
        clear_preview_cache();
        Ok(ids)
    })
}

#[tauri::command]
fn add_media_to_timeline(
    media_ref: String,
    start_frame: i64,
    state: State<'_, AppState>,
) -> Result<String, String> {
    with_state_mut(state, |s| {
        let id = import_media::add_imported_to_timeline(s, &media_ref, start_frame)?;
        clear_preview_cache();
        Ok(id)
    })
}

#[tauri::command]
fn list_media(state: State<'_, AppState>) -> Result<Value, String> {
    with_state(state, |s| {
        Ok(json!({
            "entries": crate::media::list_media(s.manifest.as_ref())
        }))
    })
}

fn persist_state(state: &EditorState) -> Result<(), String> {
    let path = state.project_path.as_deref().ok_or("no project open")?;
    let timeline = serde_json::to_string_pretty(&state.timeline).map_err(|e| e.to_string())?;
    let manifest = state
        .manifest
        .as_ref()
        .map(|m| serde_json::to_string_pretty(m).unwrap_or_default());
    ProjectPackage::save(path, &timeline, manifest.as_deref()).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let editor: AppState = std::sync::Arc::new(std::sync::Mutex::new(EditorState::default()));
    mcp::start_mcp_server(editor.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(editor)
        .invoke_handler(tauri::generate_handler![
            open_project,
            save_project,
            project_summary,
            storage_directory,
            render_preview_frame,
            get_preview_context,
            export_video,
            get_timeline,
            move_clip_cmd,
            remove_clip_cmd,
            set_clip_properties_cmd,
            split_clip_cmd,
            undo_cmd,
            mcp_status,
            create_project,
            import_media_files,
            add_media_to_timeline,
            list_media,
        ])
        .setup(|_| {
            project::ensure_storage_directory();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
