import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { PreviewContext } from "../types/project";
import { getPreviewContext, renderPreviewFrame } from "../lib/tauri-api";

interface PreviewPlayerProps {
  playhead: number;
  playing: boolean;
  projectOpen: boolean;
}

export function PreviewPlayer({ playhead, playing, projectOpen }: PreviewPlayerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [ctx, setCtx] = useState<PreviewContext | null>(null);
  const [previewBusy, setPreviewBusy] = useState(false);
  const requestId = useRef(0);
  const debounceRef = useRef<number | null>(null);

  const useNative = playing && ctx?.canUseNativeVideo && ctx.mediaPath;

  useEffect(() => {
    if (!projectOpen) {
      setPreviewUrl(null);
      setCtx(null);
      return;
    }

    let cancelled = false;
    getPreviewContext(playhead)
      .then((c) => {
        if (!cancelled) setCtx(c);
      })
      .catch(() => {
        if (!cancelled) setCtx(null);
      });

    return () => {
      cancelled = true;
    };
  }, [projectOpen, playhead]);

  useEffect(() => {
    if (!projectOpen || useNative) return;

    if (debounceRef.current) window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(() => {
      const id = ++requestId.current;
      setPreviewBusy(true);
      renderPreviewFrame(playhead)
        .then((b64) => {
          if (id !== requestId.current) return;
          setPreviewUrl(`data:image/jpeg;base64,${b64}`);
        })
        .catch(() => {
          if (id !== requestId.current) return;
        })
        .finally(() => {
          if (id === requestId.current) setPreviewBusy(false);
        });
    }, playing ? 0 : 120);

    return () => {
      if (debounceRef.current) window.clearTimeout(debounceRef.current);
    };
  }, [projectOpen, playhead, playing, useNative]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video || !useNative || !ctx?.mediaPath) return;

    const src = convertFileSrc(ctx.mediaPath);
    if (video.src !== src) {
      video.src = src;
      video.load();
    }

    const target = ctx.sourceSeconds / Math.max(ctx.speed, 0.001);
    if (Math.abs(video.currentTime - target) > 0.05) {
      video.currentTime = target;
    }

    if (playing) {
      video.playbackRate = Math.max(ctx.speed, 0.1);
      void video.play().catch(() => undefined);
    } else {
      video.pause();
    }
  }, [ctx, playing, useNative]);

  if (useNative && ctx?.mediaPath) {
    return (
      <video
        ref={videoRef}
        className="preview-image"
        muted
        playsInline
        preload="auto"
      />
    );
  }

  if (previewUrl) {
    return <img src={previewUrl} alt="Preview" className="preview-image" />;
  }

  return (
    <div className="preview-placeholder">
      {previewBusy ? "Rendering preview…" : "No preview at playhead"}
    </div>
  );
}
