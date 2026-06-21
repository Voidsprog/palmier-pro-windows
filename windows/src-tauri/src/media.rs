use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn resolve_media_path(project_root: &Path, source: &Value) -> Option<PathBuf> {
    if let Some(ext) = source.get("external") {
        let path = ext.get("absolutePath")?.as_str()?;
        return Some(PathBuf::from(path));
    }
    if let Some(proj) = source.get("project") {
        let rel = proj.get("relativePath")?.as_str()?;
        return Some(project_root.join(rel));
    }
    None
}

pub fn find_media_entry(manifest: Option<&Value>, media_ref: &str) -> Option<Value> {
    let entries = manifest?.get("entries")?.as_array()?;
    entries
        .iter()
        .find(|e| e.get("id").and_then(|v| v.as_str()) == Some(media_ref))
        .cloned()
}

pub fn media_file_path(project_root: &Path, manifest: Option<&Value>, media_ref: &str) -> Option<PathBuf> {
    let entry = find_media_entry(manifest, media_ref)?;
    let source = entry.get("source")?;
    let path = resolve_media_path(project_root, source)?;
    if path.exists() { Some(path) } else { None }
}

pub fn list_media(manifest: Option<&Value>) -> Vec<Value> {
    manifest
        .and_then(|m| m.get("entries"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}
