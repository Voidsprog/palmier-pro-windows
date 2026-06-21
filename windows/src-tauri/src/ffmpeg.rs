use std::path::{Path, PathBuf};
use std::process::Command;

pub fn find_ffmpeg() -> Result<PathBuf, String> {
    if let Ok(p) = which_ffmpeg("ffmpeg") {
        return Ok(p);
    }
    #[cfg(windows)]
    {
        for candidate in [
            r"C:\ffmpeg\bin\ffmpeg.exe",
            r"C:\Program Files\ffmpeg\bin\ffmpeg.exe",
        ] {
            let p = PathBuf::from(candidate);
            if p.exists() {
                return Ok(p);
            }
        }
    }
    Err("FFmpeg not found. Install FFmpeg and add it to PATH.".into())
}

fn which_ffmpeg(name: &str) -> Result<PathBuf, String> {
    let output = Command::new("where")
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

pub fn extract_frame(input: &Path, source_seconds: f64, width: i64, height: i64) -> Result<Vec<u8>, String> {
    let _guard = crate::preview::ffmpeg_guard();
    let ffmpeg = find_ffmpeg()?;
    let ss = format!("{source_seconds:.6}");

    let output = Command::new(&ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-ss",
            &ss,
            "-i",
            input.to_str().ok_or("invalid path")?,
            "-vframes",
            "1",
            "-vf",
            &format!("scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:black"),
            "-f",
            "image2pipe",
            "-vcodec",
            "mjpeg",
            "pipe:1",
        ])
        .output()
        .map_err(|e| format!("ffmpeg failed: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg error: {err}"));
    }
    Ok(output.stdout)
}

pub fn extract_image(input: &Path, width: i64, height: i64) -> Result<Vec<u8>, String> {
    let _guard = crate::preview::ffmpeg_guard();
    let ffmpeg = find_ffmpeg()?;
    let output = Command::new(&ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            input.to_str().ok_or("invalid path")?,
            "-vframes",
            "1",
            "-vf",
            &format!("scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:black"),
            "-f",
            "image2pipe",
            "-vcodec",
            "mjpeg",
            "pipe:1",
        ])
        .output()
        .map_err(|e| format!("ffmpeg failed: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg error: {err}"));
    }
    Ok(output.stdout)
}

pub fn run_ffmpeg(args: &[&str]) -> Result<(), String> {
    let _guard = crate::preview::ffmpeg_guard();
    let ffmpeg = find_ffmpeg()?;
    let output = Command::new(&ffmpeg)
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .args(args)
        .output()
        .map_err(|e| format!("ffmpeg failed: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg export error: {err}"));
    }
    Ok(())
}

pub fn source_seconds_for_clip(clip: &serde_json::Value, local_frame: i64, fps: f64) -> f64 {
    let trim_start = clip.get("trimStartFrame").and_then(|v| v.as_i64()).unwrap_or(0);
    let speed = clip.get("speed").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let source_frames = (local_frame as f64 * speed).round() as i64 + trim_start;
    source_frames as f64 / fps
}

pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp" | "tiff" | "heic"))
        .unwrap_or(false)
}
