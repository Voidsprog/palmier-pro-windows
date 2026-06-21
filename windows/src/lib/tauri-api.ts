import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { MediaManifestEntry, PreviewContext, ProjectPackage, ProjectSummary, Timeline } from "../types/project";

export async function pickProjectDirectory(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Open Palmier project",
  });
  if (selected === null || Array.isArray(selected)) return null;
  return selected;
}

export async function pickMediaFiles(): Promise<string[]> {
  const selected = await open({
    multiple: true,
    title: "Import media",
    filters: [
      {
        name: "Media",
        extensions: ["mp4", "mov", "m4v", "mp3", "wav", "aac", "m4a", "png", "jpg", "jpeg", "webp", "tiff", "heic"],
      },
    ],
  });
  if (selected === null) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function pickExportPath(codec: "h264" | "h265"): Promise<string | null> {
  const selected = await save({
    title: "Export video",
    defaultPath: `export.${codec === "h265" ? "mp4" : "mp4"}`,
    filters: [{ name: "MP4 video", extensions: ["mp4"] }],
  });
  return selected;
}

export async function openProject(path: string): Promise<ProjectPackage> {
  return invoke<ProjectPackage>("open_project", { path });
}

export async function saveProject(
  path: string,
  timelineJson: string,
  manifestJson?: string,
): Promise<void> {
  await invoke("save_project", {
    path,
    timelineJson,
    manifestJson: manifestJson ?? null,
  });
}

export async function getProjectSummary(path: string): Promise<ProjectSummary> {
  return invoke<ProjectSummary>("project_summary", { path });
}

export async function getStorageDirectory(): Promise<string> {
  return invoke<string>("storage_directory");
}

export async function renderPreviewFrame(frame: number): Promise<string> {
  return invoke<string>("render_preview_frame", { frame });
}

export async function getPreviewContext(frame: number): Promise<PreviewContext> {
  return invoke<PreviewContext>("get_preview_context", { frame });
}

export async function exportVideo(outputPath: string, codec: "h264" | "h265"): Promise<void> {
  await invoke("export_video", { outputPath, codec });
}

export async function getTimeline(): Promise<Timeline & { totalFrames: number }> {
  return invoke("get_timeline");
}

export async function moveClip(clipId: string, startFrame: number): Promise<void> {
  await invoke("move_clip_cmd", { clipId, startFrame });
}

export async function removeClip(clipId: string): Promise<void> {
  await invoke("remove_clip_cmd", { clipId });
}

export async function setClipProperties(
  clipId: string,
  properties: Record<string, unknown>,
): Promise<void> {
  await invoke("set_clip_properties_cmd", { clipId, properties });
}

export async function splitClip(clipId: string, atFrame: number): Promise<string> {
  return invoke<string>("split_clip_cmd", { clipId, atFrame });
}

export async function undo(): Promise<boolean> {
  return invoke<boolean>("undo_cmd");
}

export async function getMcpStatus(): Promise<boolean> {
  return invoke<boolean>("mcp_status");
}

export async function createProject(name = "Untitled Project"): Promise<ProjectPackage> {
  return invoke<ProjectPackage>("create_project", { name });
}

export async function importMediaFiles(paths: string[]): Promise<string[]> {
  return invoke<string[]>("import_media_files", { paths });
}

export async function listMedia(): Promise<{ entries: MediaManifestEntry[] }> {
  return invoke("list_media");
}

export async function addMediaToTimeline(mediaRef: string, startFrame: number): Promise<string> {
  return invoke<string>("add_media_to_timeline", { mediaRef, startFrame });
}

export function formatTimecode(frames: number, fps: number): string {
  if (fps <= 0) return "00:00:00:00";
  const totalSeconds = Math.floor(frames / fps);
  const ff = frames % fps;
  const ss = totalSeconds % 60;
  const mm = Math.floor(totalSeconds / 60) % 60;
  const hh = Math.floor(totalSeconds / 3600);
  const pad = (n: number, len = 2) => String(n).padStart(len, "0");
  return `${pad(hh)}:${pad(mm)}:${pad(ss)}:${pad(ff)}`;
}

export const MCP_URL = "http://127.0.0.1:19789/mcp";
