export type ClipType = "video" | "audio" | "image" | "text" | "lottie";

export type Interpolation = "linear" | "hold" | "smooth";

export interface Keyframe<T> {
  frame: number;
  value: T;
  interpolationOut?: Interpolation;
}

export interface KeyframeTrack<T> {
  keyframes: Keyframe<T>[];
}

export interface AnimPair {
  a: number;
  b: number;
}

export interface Transform {
  centerX?: number;
  centerY?: number;
  width?: number;
  height?: number;
  rotation?: number;
  flipHorizontal?: boolean;
  flipVertical?: boolean;
}

export interface Crop {
  left?: number;
  top?: number;
  right?: number;
  bottom?: number;
}

export interface TextStyle {
  fontName?: string;
  fontSize?: number;
  fontScale?: number;
  color?: { r: number; g: number; b: number; a: number };
  alignment?: "left" | "center" | "right";
}

export interface Clip {
  id: string;
  mediaRef: string;
  mediaType?: ClipType;
  sourceClipType?: ClipType;
  startFrame: number;
  durationFrames: number;
  trimStartFrame?: number;
  trimEndFrame?: number;
  speed?: number;
  volume?: number;
  fadeInFrames?: number;
  fadeOutFrames?: number;
  opacity?: number;
  transform?: Transform;
  crop?: Crop;
  linkGroupId?: string | null;
  captionGroupId?: string | null;
  textContent?: string | null;
  textStyle?: TextStyle | null;
  opacityTrack?: KeyframeTrack<number> | null;
  positionTrack?: KeyframeTrack<AnimPair> | null;
  scaleTrack?: KeyframeTrack<AnimPair> | null;
  rotationTrack?: KeyframeTrack<number> | null;
  cropTrack?: KeyframeTrack<Crop> | null;
  volumeTrack?: KeyframeTrack<number> | null;
}

export interface Track {
  id: string;
  type: ClipType;
  muted?: boolean;
  hidden?: boolean;
  syncLocked?: boolean;
  clips: Clip[];
}

export interface Timeline {
  fps?: number;
  width?: number;
  height?: number;
  settingsConfigured?: boolean;
  tracks: Track[];
}

export type MediaSource =
  | { external: { absolutePath: string } }
  | { project: { relativePath: string } };

export interface MediaManifestEntry {
  id: string;
  name: string;
  type: ClipType;
  source: MediaSource;
  duration: number;
  folderId?: string | null;
}

export interface MediaFolder {
  id: string;
  name: string;
  parentFolderId?: string | null;
}

export interface MediaManifest {
  version?: number;
  entries: MediaManifestEntry[];
  folders?: MediaFolder[];
}

export interface ProjectPackage {
  path: string;
  timeline: Timeline;
  manifest: MediaManifest | null;
}

export interface ProjectSummary {
  path: string;
  name: string;
  fps: number;
  width: number;
  height: number;
  trackCount: number;
  clipCount: number;
  mediaCount: number;
  totalFrames: number;
}

export interface PreviewContext {
  frame: number;
  canUseNativeVideo: boolean;
  mediaPath: string | null;
  sourceSeconds: number;
  speed: number;
  clipStartFrame: number;
  clipDurationFrames: number;
  isImage: boolean;
  mediaRef: string | null;
}

export const PROJECT_FILES = {
  timeline: "project.json",
  manifest: "media.json",
  generationLog: "generation-log.json",
  thumbnail: "thumbnail.jpg",
  mediaDir: "media",
  extension: "palmier",
} as const;
