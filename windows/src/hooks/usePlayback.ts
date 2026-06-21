import { useEffect, useRef } from "react";

export function usePlayback(
  playing: boolean,
  fps: number,
  totalFrames: number,
  playhead: number,
  setPlayhead: (frame: number) => void,
  setPlaying: (playing: boolean) => void,
) {
  const rafRef = useRef<number | null>(null);
  const lastTimeRef = useRef<number | null>(null);
  const playheadRef = useRef(playhead);

  playheadRef.current = playhead;

  useEffect(() => {
    if (!playing) {
      lastTimeRef.current = null;
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
      return;
    }

    const frameDuration = 1000 / Math.max(fps, 1);

    const tick = (now: number) => {
      if (lastTimeRef.current === null) {
        lastTimeRef.current = now;
      }
      const elapsed = now - lastTimeRef.current;
      if (elapsed >= frameDuration) {
        const steps = Math.floor(elapsed / frameDuration);
        lastTimeRef.current += steps * frameDuration;
        const next = playheadRef.current + steps;
        if (next >= totalFrames - 1) {
          setPlayhead(Math.max(totalFrames - 1, 0));
          setPlaying(false);
          return;
        }
        setPlayhead(next);
      }
      rafRef.current = requestAnimationFrame(tick);
    };

    rafRef.current = requestAnimationFrame(tick);
    return () => {
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
    };
  }, [playing, fps, totalFrames, setPlayhead, setPlaying]);
}
