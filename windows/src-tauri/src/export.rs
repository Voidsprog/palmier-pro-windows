use crate::ffmpeg::{is_image_path, run_ffmpeg};
use crate::media::media_file_path;
use crate::state::EditorState;
use serde_json::Value;
use std::path::{Path, PathBuf};

struct ExportClip {    path: PathBuf,
    start_sec: f64,
    end_sec: f64,
    trim_start_sec: f64,
    speed: f64,
    is_image: bool,
}

pub fn export_timeline(
    state: &EditorState,
    output: &Path,
    codec: &str,
) -> Result<(), String> {
    let project_path = state.project_path.as_deref().ok_or("no project open")?;
    let root = Path::new(project_path);
    let fps = state.fps();
    let w = state.width();
    let h = state.height();
    let total_frames = state.total_frames().max(1);
    let duration_sec = total_frames as f64 / fps;

    let mut layers: Vec<ExportClip> = Vec::new();
    let tracks = state
        .timeline
        .get("tracks")
        .and_then(|v| v.as_array())
        .ok_or("no tracks")?;

    for track in tracks {
        if track.get("hidden").and_then(|v| v.as_bool()) == Some(true) {
            continue;
        }
        let track_type = track.get("type").and_then(|v| v.as_str()).unwrap_or("video");
        if track_type != "video" && track_type != "image" {
            continue;
        }
        let Some(clips) = track.get("clips").and_then(|v| v.as_array()) else {
            continue;
        };
        for clip in clips {
            let media_ref = clip.get("mediaRef").and_then(|v| v.as_str()).unwrap_or("");
            let Some(path) = media_file_path(root, state.manifest.as_ref(), media_ref) else {
                continue;
            };
            let start = clip.get("startFrame").and_then(|v| v.as_i64()).unwrap_or(0);
            let dur = clip.get("durationFrames").and_then(|v| v.as_i64()).unwrap_or(0);
            let trim_start = clip.get("trimStartFrame").and_then(|v| v.as_i64()).unwrap_or(0);
            let speed = clip.get("speed").and_then(|v| v.as_f64()).unwrap_or(1.0);

            let is_image = is_image_path(&path) || track_type == "image";
            layers.push(ExportClip {
                path,
                start_sec: start as f64 / fps,
                end_sec: (start + dur) as f64 / fps,
                trim_start_sec: trim_start as f64 / fps,
                speed,
                is_image,
            });
        }
    }

    if layers.is_empty() {
        return Err("no exportable clips".into());
    }

    let vcodec = match codec {
        "h265" | "hevc" => "libx265",
        _ => "libx264",
    };

    if layers.len() == 1 {
        export_single_layer(&layers[0], output, w, h, fps, duration_sec, vcodec)?;
    } else {
        export_multi_layer(&layers, output, w, h, fps, duration_sec, vcodec)?;
    }

    Ok(())
}

fn export_single_layer(
    layer: &ExportClip,
    output: &Path,
    w: i64,
    h: i64,
    fps: f64,
    duration_sec: f64,
    vcodec: &str,
) -> Result<(), String> {
    let out = output.to_str().ok_or("invalid output path")?;
    if layer.is_image {
        let filter = format!(
            "loop=loop=-1:size=1:start=0,trim=duration={duration_sec},fps={fps},scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:black"
        );
        run_ffmpeg(&[
            "-loop",
            "1",
            "-i",
            layer.path.to_str().unwrap(),
            "-vf",
            &filter,
            "-c:v",
            vcodec,
            "-pix_fmt",
            "yuv420p",
            "-t",
            &format!("{duration_sec:.6}"),
            "-y",
            out,
        ])
    } else {
        let dur = layer.end_sec - layer.start_sec;
        run_ffmpeg(&[
            "-ss",
            &format!("{:.6}", layer.trim_start_sec),
            "-i",
            layer.path.to_str().unwrap(),
            "-t",
            &format!("{dur:.6}"),
            "-vf",
            &format!("setpts=PTS/{},scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:black,fps={fps}", layer.speed),
            "-c:v",
            vcodec,
            "-pix_fmt",
            "yuv420p",
            "-y",
            out,
        ])
    }
}

fn export_multi_layer(
    layers: &[ExportClip],
    output: &Path,
    w: i64,
    h: i64,
    fps: f64,
    duration_sec: f64,
    vcodec: &str,
) -> Result<(), String> {
    let mut args: Vec<String> = Vec::new();
    args.push("-f".into());
    args.push("lavfi".into());
    args.push("-i".into());
    args.push(format!("color=c=black:s={w}x{h}:r={fps}:d={duration_sec:.6}"));

    for layer in layers {
        if layer.is_image {
            args.push("-loop".into());
            args.push("1".into());
        }
        args.push("-i".into());
        args.push(layer.path.to_string_lossy().into_owned());
    }

    let mut filter = String::new();
    let mut last = "[0:v]".to_string();

    for (i, layer) in layers.iter().enumerate() {
        let input_idx = i + 1;
        let dur = layer.end_sec - layer.start_sec;
        let label = format!("[v{i}]");
        if layer.is_image {
            filter.push_str(&format!(
                "[{input_idx}:v]loop=loop=-1:size=1,fps={fps},trim=duration={dur:.6},setpts=PTS-STARTPTS+{:.6}/TB,scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:black{label};",
                layer.start_sec, label = label
            ));
        } else {
            filter.push_str(&format!(
                "[{input_idx}:v]trim=start={:.6}:duration={dur:.6},setpts=PTS-STARTPTS,setpts=PTS/{},scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:black,setpts=PTS-STARTPTS+{:.6}/TB{label};",
                layer.trim_start_sec, layer.speed, layer.start_sec, label = label
            ));
        }
        let out = format!("[o{i}]");
        filter.push_str(&format!(
            "{last}{label}overlay=enable='between(t,{:.6},{:.6})'{out};",
            layer.start_sec, layer.end_sec, out = out
        ));
        last = out;
    }

    filter.push_str(&format!("{last}format=yuv420p[out]"));

    let out = output.to_str().ok_or("invalid output")?;
    let duration_arg = format!("{duration_sec:.6}");
    let mut flat: Vec<&str> = args.iter().map(String::as_str).collect();
    flat.extend([
        "-filter_complex",
        &filter,
        "-map",
        "[out]",
        "-c:v",
        vcodec,
        "-pix_fmt",
        "yuv420p",
        "-t",
        &duration_arg,
        "-y",
        out,
    ]);

    run_ffmpeg(&flat)
}

pub fn timeline_summary_json(state: &EditorState) -> Value {
    serde_json::json!({
        "fps": state.fps(),
        "width": state.width(),
        "height": state.height(),
        "totalFrames": state.total_frames(),
        "tracks": state.timeline.get("tracks").cloned().unwrap_or(serde_json::json!([])),
        "canGenerate": false,
        "platform": "windows"
    })
}
