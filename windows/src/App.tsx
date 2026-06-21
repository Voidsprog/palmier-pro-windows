import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Clip, MediaManifestEntry, ProjectPackage, ProjectSummary, Track } from "./types/project";
import {
  addMediaToTimeline,
  createProject,
  exportVideo,
  formatTimecode,
  getMcpStatus,
  getProjectSummary,
  getStorageDirectory,
  getTimeline,
  importMediaFiles,
  listMedia,
  MCP_URL,
  moveClip,
  openProject,
  pickExportPath,
  pickMediaFiles,
  pickProjectDirectory,
  removeClip,
  setClipProperties,
  splitClip,
  undo,
} from "./lib/tauri-api";
import { PreviewPlayer } from "./components/PreviewPlayer";
import { usePlayback } from "./hooks/usePlayback";

function clipClass(type: string): string {
  if (type === "audio") return "audio";
  if (type === "image" || type === "text" || type === "lottie") return "image";
  return "video";
}

interface TimelineEditorProps {
  tracks: Track[];
  fps: number;
  totalFrames: number;
  playhead: number;
  selectedClipId: string | null;
  onSelectClip: (id: string | null) => void;
  onPlayheadChange: (frame: number) => void;
  onMoveClip: (clipId: string, startFrame: number) => void;
}

function TimelineEditor({
  tracks,
  fps,
  totalFrames,
  playhead,
  selectedClipId,
  onSelectClip,
  onPlayheadChange,
  onMoveClip,
}: TimelineEditorProps) {
  const span = Math.max(totalFrames, 1);
  const laneRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ clipId: string; startX: number; origStart: number; pendingStart?: number } | null>(null);

  const frameFromEvent = (clientX: number, rect: DOMRect) => {
    const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    return Math.round(ratio * span);
  };

  const onLaneClick = (e: React.MouseEvent) => {
    if (!laneRef.current) return;
    const rect = laneRef.current.getBoundingClientRect();
    onPlayheadChange(frameFromEvent(e.clientX, rect));
  };

  const onClipMouseDown = (e: React.MouseEvent, clip: Clip) => {
    e.stopPropagation();
    onSelectClip(clip.id);
    if (!laneRef.current) return;
    dragRef.current = { clipId: clip.id, startX: e.clientX, origStart: clip.startFrame };
  };

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!dragRef.current || !laneRef.current) return;
      const rect = laneRef.current.getBoundingClientRect();
      const deltaPx = e.clientX - dragRef.current.startX;
      const deltaFrames = Math.round((deltaPx / rect.width) * span);
      dragRef.current.pendingStart = Math.max(0, dragRef.current.origStart + deltaFrames);
    };
    const onUp = () => {
      if (!dragRef.current) return;
      const { clipId, pendingStart, origStart } = dragRef.current;
      dragRef.current = null;
      onMoveClip(clipId, pendingStart ?? origStart);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [onMoveClip, span]);

  const playheadPct = (playhead / span) * 100;

  return (
    <div className="timeline-editor">
      <div className="timeline-ruler" ref={laneRef} onClick={onLaneClick}>
        <div className="playhead" style={{ left: `${playheadPct}%` }} />
      </div>
      {tracks.map((track) => (
        <div className="track-row" key={track.id}>
          <div className="track-label">{track.type}</div>
          <div className="track-lane" onClick={onLaneClick}>
            {track.clips.map((clip) => {
              const left = (clip.startFrame / span) * 100;
              const width = Math.max((clip.durationFrames / span) * 100, 1.5);
              const selected = clip.id === selectedClipId;
              return (
                <div
                  key={clip.id}
                  className={`clip-block ${clipClass(clip.mediaType ?? track.type)}${selected ? " selected" : ""}`}
                  style={{ left: `${left}%`, width: `${width}%` }}
                  title={clip.mediaRef}
                  onMouseDown={(e) => onClipMouseDown(e, clip)}
                >
                  {clip.mediaRef.slice(0, 12)}
                </div>
              );
            })}
            <div className="playhead" style={{ left: `${playheadPct}%` }} />
          </div>
        </div>
      ))}
      <div className="timecode">
        {formatTimecode(playhead, fps)} / {formatTimecode(totalFrames, fps)}
      </div>
    </div>
  );
}

function findClip(tracks: Track[], clipId: string): Clip | null {
  for (const track of tracks) {
    const clip = track.clips.find((c) => c.id === clipId);
    if (clip) return clip;
  }
  return null;
}

export default function App() {
  const [project, setProject] = useState<ProjectPackage | null>(null);
  const [summary, setSummary] = useState<ProjectSummary | null>(null);
  const [storageDir, setStorageDir] = useState("");
  const [loading, setLoading] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [playhead, setPlayhead] = useState(0);
  const [selectedClipId, setSelectedClipId] = useState<string | null>(null);
  const [mcpRunning, setMcpRunning] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [mediaEntries, setMediaEntries] = useState<MediaManifestEntry[]>([]);
  const [selectedMediaId, setSelectedMediaId] = useState<string | null>(null);

  const tracks = project?.timeline.tracks ?? [];
  const fps = project?.timeline.fps ?? summary?.fps ?? 30;
  const totalFrames = useMemo(() => {
    if (summary?.totalFrames) return summary.totalFrames;
    return tracks.reduce((max, track) => {
      const trackEnd = track.clips.reduce((m, clip) => Math.max(m, clip.startFrame + clip.durationFrames), 0);
      return Math.max(max, trackEnd);
    }, 0);
  }, [summary, tracks]);

  const selectedClip = selectedClipId ? findClip(tracks, selectedClipId) : null;

  const refreshTimeline = useCallback(async () => {
    if (!project) return;
    const tl = await getTimeline();
    setProject((p) => (p ? { ...p, timeline: { ...p.timeline, tracks: tl.tracks } } : p));
    const info = await getProjectSummary(project.path);
    setSummary(info);
  }, [project]);

  const refreshMedia = useCallback(async () => {
    try {
      const media = await listMedia();
      setMediaEntries(media.entries ?? []);
      if (project) {
        setProject((p) => (p ? { ...p, manifest: { version: 2, entries: media.entries, folders: p.manifest?.folders ?? [] } } : p));
      }
    } catch {
      setMediaEntries([]);
    }
  }, [project]);

  useEffect(() => {
    getStorageDirectory().then(setStorageDir).catch(() => undefined);
    getMcpStatus().then(setMcpRunning).catch(() => undefined);
  }, []);

  usePlayback(playing, fps, totalFrames, playhead, setPlayhead, setPlaying);

  const handleNewProject = useCallback(async () => {
    setError(null);
    setLoading(true);
    try {
      const loaded = await createProject("Untitled Project");
      const info = await getProjectSummary(loaded.path);
      setProject(loaded);
      setSummary(info);
      setMediaEntries([]);
      setPlayhead(0);
      setSelectedClipId(null);
      setSelectedMediaId(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const handleImportMedia = useCallback(async () => {
    if (!project) {
      setError("Open or create a project first.");
      return;
    }
    const paths = await pickMediaFiles();
    if (paths.length === 0) return;
    setError(null);
    setLoading(true);
    try {
      await importMediaFiles(paths);
      await refreshMedia();
      const info = await getProjectSummary(project.path);
      setSummary(info);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [project, refreshMedia]);

  const handleAddMediaToTimeline = useCallback(async () => {
    if (!selectedMediaId) return;
    try {
      await addMediaToTimeline(selectedMediaId, playhead);
      await refreshTimeline();
      if (project) {
        const info = await getProjectSummary(project.path);
        setSummary(info);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [selectedMediaId, playhead, project, refreshTimeline]);

  const handleOpen = useCallback(async () => {
    setError(null);
    const path = await pickProjectDirectory();
    if (!path) return;

    setLoading(true);
    try {
      const loaded = await openProject(path);
      const info = await getProjectSummary(path);
      setProject(loaded);
      setSummary(info);
      setPlayhead(0);
      setSelectedClipId(null);
      const media = await listMedia();
      setMediaEntries(media.entries ?? []);
    } catch (e) {
      setProject(null);
      setSummary(null);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const handleMoveClip = useCallback(
    async (clipId: string, startFrame: number) => {
      try {
        await moveClip(clipId, startFrame);
        await refreshTimeline();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [refreshTimeline],
  );

  const handleExport = useCallback(
    async (codec: "h264" | "h265") => {
      const path = await pickExportPath(codec);
      if (!path) return;
      setExporting(true);
      setError(null);
      try {
        await exportVideo(path, codec);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setExporting(false);
      }
    },
    [],
  );

  const handleRemoveClip = async () => {
    if (!selectedClipId) return;
    try {
      await removeClip(selectedClipId);
      setSelectedClipId(null);
      await refreshTimeline();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleSplitClip = async () => {
    if (!selectedClipId) return;
    try {
      await splitClip(selectedClipId, playhead);
      await refreshTimeline();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleUndo = async () => {
    try {
      const ok = await undo();
      if (ok) await refreshTimeline();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const updateClipProp = async (key: string, value: number) => {
    if (!selectedClipId) return;
    try {
      await setClipProperties(selectedClipId, { [key]: value });
      await refreshTimeline();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="app-shell">
      <header className="app-header">
        <div>
          <div className="app-title">Palmier Pro</div>
          <div className="app-subtitle">Windows · Tauri · FFmpeg</div>
        </div>
        <div className="header-actions">
          <button className="secondary-button" onClick={handleNewProject} disabled={loading}>
            New project
          </button>
          <button className="secondary-button" onClick={handleImportMedia} disabled={!project || loading}>
            Import media
          </button>
          <button className="secondary-button" onClick={handleUndo} disabled={!project}>
            Undo
          </button>
          <button className="secondary-button" onClick={() => handleExport("h264")} disabled={!project || exporting}>
            Export H.264
          </button>
          <button className="secondary-button" onClick={() => handleExport("h265")} disabled={!project || exporting}>
            Export H.265
          </button>
          <button className="primary-button" onClick={handleOpen} disabled={loading}>
            {loading ? "Opening…" : "Open project"}
          </button>
        </div>
      </header>

      {!project ? (
        <main className="preview-canvas">
          <div className="welcome-card">
            <h1>Palmier Pro</h1>
            <p>
              Editor de vídeo Windows com preview FFmpeg, edição de timeline, export H.264/H.265 e servidor MCP em{" "}
              <code>{MCP_URL}</code>.
            </p>
            <button className="primary-button" onClick={handleOpen} disabled={loading}>
              Open project
            </button>
            <button className="secondary-button" onClick={handleNewProject} disabled={loading}>
              New project
            </button>
            {error && <div className="error-banner">{error}</div>}
          </div>
        </main>
      ) : (
        <main className="app-main">
          <aside className="panel">
            <div className="panel-header">Media</div>
            <div className="panel-body">
              <button className="secondary-button" onClick={handleImportMedia} disabled={loading} style={{ width: "100%", marginBottom: "var(--spacing-md)" }}>
                Import…
              </button>
              {mediaEntries.length === 0 ? (
                <p className="muted">No media. Import video, audio, or image files.</p>
              ) : (
                <ul className="media-list">
                  {mediaEntries.map((entry) => (
                    <li
                      key={entry.id}
                      className={entry.id === selectedMediaId ? "media-item selected" : "media-item"}
                      onClick={() => setSelectedMediaId(entry.id)}
                    >
                      <span className="media-type">{entry.type}</span>
                      <span className="media-name">{entry.name}</span>
                    </li>
                  ))}
                </ul>
              )}
              {selectedMediaId && (
                <button className="primary-button" onClick={handleAddMediaToTimeline} style={{ width: "100%", marginTop: "var(--spacing-md)" }}>
                  Add to timeline at playhead
                </button>
              )}
              <ul className="meta-list" style={{ marginTop: "var(--spacing-xl)" }}>
                <li>
                  <span className="meta-label">Name</span>
                  <span className="meta-value">{summary?.name}</span>
                </li>
                <li>
                  <span className="meta-label">Resolution</span>
                  <span className="meta-value">
                    {summary?.width}×{summary?.height}
                  </span>
                </li>
                <li>
                  <span className="meta-label">FPS</span>
                  <span className="meta-value">{summary?.fps}</span>
                </li>
                <li>
                  <span className="meta-label">Clips</span>
                  <span className="meta-value">{summary?.clipCount}</span>
                </li>
              </ul>
              <div className="transport">
                <button className="secondary-button" onClick={() => setPlaying((p) => !p)}>
                  {playing ? "Pause" : "Play"}
                </button>
                <input
                  type="range"
                  min={0}
                  max={Math.max(totalFrames - 1, 0)}
                  value={playhead}
                  onChange={(e) => setPlayhead(Number(e.target.value))}
                  className="scrubber"
                />
              </div>
            </div>
          </aside>

          <section className="center-stage">
            <div className="preview-canvas">
              <PreviewPlayer playhead={playhead} playing={playing} projectOpen={!!project} />
            </div>
            <div className="timeline-bar">
              <TimelineEditor
                tracks={tracks}
                fps={fps}
                totalFrames={totalFrames}
                playhead={playhead}
                selectedClipId={selectedClipId}
                onSelectClip={setSelectedClipId}
                onPlayheadChange={setPlayhead}
                onMoveClip={handleMoveClip}
              />
            </div>
          </section>

          <aside className="panel">
            <div className="panel-header">Inspector</div>
            <div className="panel-body">
              {selectedClip ? (
                <div className="inspector-form">
                  <label>
                    Start frame
                    <input
                      type="number"
                      value={selectedClip.startFrame}
                      onChange={(e) => updateClipProp("startFrame", Number(e.target.value))}
                    />
                  </label>
                  <label>
                    Duration
                    <input
                      type="number"
                      value={selectedClip.durationFrames}
                      onChange={(e) => updateClipProp("durationFrames", Number(e.target.value))}
                    />
                  </label>
                  <label>
                    Volume
                    <input
                      type="range"
                      min={0}
                      max={1}
                      step={0.01}
                      value={selectedClip.volume ?? 1}
                      onChange={(e) => updateClipProp("volume", Number(e.target.value))}
                    />
                  </label>
                  <label>
                    Opacity
                    <input
                      type="range"
                      min={0}
                      max={1}
                      step={0.01}
                      value={selectedClip.opacity ?? 1}
                      onChange={(e) => updateClipProp("opacity", Number(e.target.value))}
                    />
                  </label>
                  <div className="inspector-actions">
                    <button className="secondary-button" onClick={handleSplitClip}>
                      Split at playhead
                    </button>
                    <button className="danger-button" onClick={handleRemoveClip}>
                      Remove clip
                    </button>
                  </div>
                </div>
              ) : (
                <p className="muted">Select a clip on the timeline.</p>
              )}
            </div>
          </aside>
        </main>
      )}

      <footer className="status-bar">
        <span>{project ? summary?.path : "No project open"}</span>
        <span className={mcpRunning ? "mcp-on" : "mcp-off"}>
          MCP {mcpRunning ? `on · ${MCP_URL}` : "off"}
        </span>
        <span>{exporting ? "Exporting…" : storageDir ? `Storage: ${storageDir}` : ""}</span>
      </footer>
      {error && project && <div className="toast-error">{error}</div>}
    </div>
  );
}
