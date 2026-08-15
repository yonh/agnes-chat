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
