// Message role matching the Agnes Chat Completions API
export type Role = 'system' | 'user' | 'assistant';

export interface ChatMessage {
  id: string;
  role: Role;
  content: string;
}

// Event payload sent from Rust via Tauri events during streaming
export interface StreamChunkEvent {
  messageId: string;
  content: string;
  done?: boolean;
  error?: string;
}

export interface ChatConfig {
  temperature: number;
  maxTokens: number;
  systemPrompt: string;
}

// ---------------------------------------------------------------------------
// Image Generation Types
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Image Generation Types
// ---------------------------------------------------------------------------

export type ImageMode = 'text-to-image' | 'image-to-image';
export type ImageOutputFormat = 'url' | 'base64';

export interface ImageGenerationParams {
  prompt: string;
  size: string;       // '1K', '2K', '3K', '4K' or exact like '1024x768'
  ratio?: string;     // '1:1', '3:4', '4:3', '16:9', '9:16', '2:3', '3:2', '21:9'
  inputImages?: string[];  // URLs or Data URIs for img2img
  outputFormat?: ImageOutputFormat;
}

export interface GeneratedImage {
  id: string;
  prompt: string;
  url?: string;
  base64?: string;
  format: ImageOutputFormat;
  size: string;
  ratio: string;
  timestamp: number;
  elapsedMs?: number;
  revisedPrompt?: string;
}

export interface ImageProgressEvent {
  status: string;
  message: string;
}

export interface ImageGeneratedEvent {
  status: 'completed' | 'error';
  result?: string;   // JSON string with type + url/base64
  revisedPrompt?: string;
}

// Size options matching Agnes Image API
export const SIZE_OPTIONS = ['1K', '2K', '3K', '4K'] as const;
export const RATIO_OPTIONS = ['1:1', '3:4', '4:3', '16:9', '9:16', '2:3', '3:2', '21:9'] as const;

// ---------------------------------------------------------------------------
// Video Generation Types (Agnes Video V2.0)
// ---------------------------------------------------------------------------

export type VideoMode = 'text-to-video' | 'image-to-video' | 'keyframes';
export type VideoResolution = '480p' | '720p' | '1080p';
export type VideoAspectRatio = '16:9' | '9:16' | '1:1' | '4:3' | '3:4';

export interface VideoGenerationParams {
  prompt: string;
  mode: VideoMode;
  image?: string;            // 图生视频：单张公网图片 URL
  keyframeImages?: string[]; // 关键帧：多张公网图片 URL
  resolution: VideoResolution;
  aspectRatio: VideoAspectRatio;
  numFrames: number;         // 目标帧数，须满足 8n+1 且 <= 441
  frameRate: number;         // 帧率 1-60
  negativePrompt?: string;
  seed?: number;
}

export interface GeneratedVideo {
  id: string;                 // video_id
  prompt: string;
  mode: VideoMode;
  videoUrl?: string;
  remoteUrl?: string;        // 原始视频链接（用于浏览器打开）
  size?: string;
  seconds?: string;
  status: 'queued' | 'in_progress' | 'completed' | 'failed';
  progress: number;
  error?: string;
  timestamp: number;
}

export interface VideoProgressEvent {
  videoId: string;
  status: string;
  progress: number;
}

export interface VideoCompleteEvent {
  videoId: string;
  url?: string;
  remoteUrl?: string;
  size?: string;
  seconds?: string;
  error?: string;
}

export const VIDEO_RESOLUTION_OPTIONS = ['480p', '720p', '1080p'] as const;
export const VIDEO_ASPECT_OPTIONS = ['16:9', '9:16', '1:1', '4:3', '3:4'] as const;
export const VIDEO_DURATION_OPTIONS = [
  { label: '约 3 秒', numFrames: 81 },
  { label: '约 5 秒', numFrames: 121 },
  { label: '约 10 秒', numFrames: 241 },
  { label: '约 18 秒', numFrames: 441 },
] as const;
