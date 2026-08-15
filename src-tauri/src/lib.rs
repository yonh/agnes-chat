use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{Emitter, State, Manager};

// ---------------------------------------------------------------------------
// Agnes API Constants
// Base URL: https://apihub.agnes-ai.com/v1
// ---------------------------------------------------------------------------

const AGNES_API_BASE: &str = "https://apihub.agnes-ai.com/v1";
const CHAT_MODEL: &str = "agnes-2.5-pro";
const IMAGE_MODEL: &str = "agnes-image-2.1-flash";
const VIDEO_MODEL: &str = "agnes-video-v2.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatParams {
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
}

fn default_temperature() -> f32 { 0.4 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatConfig {
    pub temperature: f32,
    pub max_tokens: u32,
    pub system_prompt: String,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            temperature: 0.4,
            max_tokens: 4096,
            system_prompt: "You are a helpful AI assistant powered by Agnes 2.5 Pro.".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
struct ChatRequest {
    model: &'static str,
    messages: Vec<ApiMessage>,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct StreamResponseLine {
    choices: Option<Vec<StreamChoice>>,
    #[serde(default)]
    error: Option<StreamError>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamError {
    message: Option<String>,
}

// ---------------------------------------------------------------------------
// Agnes Image Generation API Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
struct ImageRequest {
    model: &'static str,
    prompt: String,
    size: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_body: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ImageResponse {
    data: Vec<ImageResult>,
    error: Option<ImageError>,
}

#[derive(Debug, Deserialize)]
struct ImageResult {
    url: Option<String>,
    b64_json: Option<String>,
    revised_prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ImageError {
    message: Option<String>,
    #[serde(rename = "type")]
    error_type: Option<String>,
}

// ---------------------------------------------------------------------------
// App state (in-memory key store for demo — production should use secure store)
// ---------------------------------------------------------------------------

pub struct AppState {
    api_key: std::sync::Mutex<Option<String>>,
    chat_config: std::sync::Mutex<ChatConfig>,
}

impl Default for AppState {
    fn default() -> Self {
        let env_key = std::env::var("AGNES_API_KEY").ok();
        Self {
            api_key: std::sync::Mutex::new(env_key),
            chat_config: std::sync::Mutex::new(ChatConfig::default()),
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// Return the currently configured API key (for UI display purposes)
#[tauri::command]
async fn get_api_key(state: State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(state.api_key.lock().map_err(|e| e.to_string())?.clone())
}

/// Persist a new API key in app state
#[tauri::command]
async fn set_api_key(state: State<'_, AppState>, api_key: Option<String>) -> Result<(), String> {
    *state.api_key.lock().map_err(|e| e.to_string())? = api_key.filter(|s| !s.trim().is_empty());
    Ok(())
}

/// Return the current chat config (temperature, max_tokens, system_prompt)
#[tauri::command]
async fn get_chat_config(state: State<'_, AppState>) -> Result<ChatConfig, String> {
    state.chat_config.lock().map_err(|e| e.to_string()).map(|c| c.clone())
}

/// Save a new chat config
#[tauri::command]
async fn set_chat_config(
    state: State<'_, AppState>,
    config: ChatConfig,
) -> Result<(), String> {
    *state.chat_config.lock().map_err(|e| e.to_string())? = config;
    Ok(())
}

/// Resolve the API key from app state, erroring with a helpful message if missing.
fn resolve_api_key(state: &State<'_, AppState>) -> Result<String, String> {
    state
        .api_key
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?
        .clone()
        .ok_or_else(|| "未配置 API Key，请在设置中填写或通过环境变量 AGNES_API_KEY 提供".to_string())
}

/// Send chat history to Agnes and stream back response chunks as Tauri events
///
/// Frontend emits event `chat-stream-chunk` with payload `{ messageId, content, done?, error? }`
#[tauri::command]
async fn send_chat_message(
    window: tauri::Window,
    state: State<'_, AppState>,
    api_key: Option<String>,
    history: Vec<ApiMessage>,
    message_id: String,
    _stream: bool,
    params: Option<ChatParams>,
) -> Result<(), String> {
    // Resolve the API key: command arg > state > error
    let key = if let Some(ref k) = api_key {
        k.clone()
    } else {
        state
            .api_key
            .lock()
            .map_err(|e| format!("Lock poisoned: {}", e))?
            .clone()
            .ok_or_else(|| "未配置 API Key，请在设置中填写或通过环境变量 AGNES_API_KEY 提供".to_string())?
    };

    // Merge params with stored config (params take priority)
    let config = state
        .chat_config
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?
        .clone();

    let temperature = params.as_ref().map(|p| p.temperature).unwrap_or(config.temperature);
    let max_tokens = params
        .as_ref()
        .and_then(|p| p.max_tokens)
        .or(Some(config.max_tokens));
    let system_prompt = params
        .as_ref()
        .and_then(|p| p.system_prompt.clone())
        .or(Some(config.system_prompt.clone()));

    // Prepend system prompt if present and not already in history
    let mut messages = if let Some(sys) = system_prompt {
        let has_system = history.iter().any(|m| m.role == "system");
        if has_system {
            history
        } else {
            let mut prefixed = vec![ApiMessage {
                role: "system".to_string(),
                content: sys,
            }];
            prefixed.extend(history);
            prefixed
        }
    } else {
        history
    };

    let client = reqwest::Client::new();

    let request = ChatRequest {
        model: CHAT_MODEL,
        messages: std::mem::take(&mut messages),
        temperature,
        max_tokens,
        stream: true,
    };

    let resp = client
        .post(format!("{}/chat/completions", AGNES_API_BASE))
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("API 返回错误 {}: {}", status.as_u16(), body));
    }

    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                buffer.push_str(&text);

                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].to_string();
                    buffer = buffer[pos + 1..].to_string();
                    if let Some(rpos) = line.rfind('\r') {
                        parse_and_emit(&line[..rpos], &message_id, &window)?;
                    } else {
                        parse_and_emit(&line, &message_id, &window)?;
                    }
                }
            }
            Err(e) => {
                emit_chunk(&window, &message_id, "", true, Some(&format!("流读取错误: {}", e)))?;
                return Err(format!("流读取错误: {}", e));
            }
        }
    }

    // Finalize: if we never got a finish_reason, signal done explicitly
    emit_chunk(&window, &message_id, "", true, None)?;

    Ok(())
}

/// Parse a single SSE line and forward the content delta to the frontend
fn parse_and_emit(line: &str, message_id: &str, window: &tauri::Window) -> Result<(), String> {
    let trimmed = line.trim();

    // Data lines start with "data: "
    if !trimmed.starts_with("data:") && !trimmed.starts_with("data :") {
        return Ok(());
    }

    // Extract JSON portion after "data: " prefix
    let json_part = if trimmed.starts_with("data :") {
        &trimmed[6..]
    } else {
        &trimmed[5..]
    }
    .trim();

    if json_part == "[DONE]" {
        emit_chunk(window, message_id, "", true, None)?;
        return Ok(());
    }

    let parsed: StreamResponseLine =
        serde_json::from_str(json_part).map_err(|e| format!("JSON 解析错误: {}", e))?;

    if let Some(err) = parsed.error {
        let msg = err.message.unwrap_or_else(|| "未知错误".to_string());
        emit_chunk(window, message_id, "", true, Some(&msg))?;
        return Ok(());
    }

    if let Some(choices) = parsed.choices {
        for choice in choices {
            if let Some(content) = choice.delta.content {
                emit_chunk(window, message_id, &content, false, None)?;
            }
            if let Some(reason) = choice.finish_reason {
                if reason != "" {
                    emit_chunk(window, message_id, "", true, None)?;
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

/// Emit one streaming chunk event to the frontend
fn emit_chunk(
    window: &tauri::Window,
    message_id: &str,
    content: &str,
    done: bool,
    error: Option<&str>,
) -> Result<(), String> {
    let mut payload: HashMap<&str, serde_json::Value> = HashMap::new();
    payload.insert("messageId", serde_json::json!(message_id));
    payload.insert("content", serde_json::json!(content));
    payload.insert("done", serde_json::json!(done));
    if let Some(err) = error {
        payload.insert("error", serde_json::json!(err));
    }

    window
        .emit("chat-stream-chunk", &payload)
        .map_err(|e| format!("事件发送失败: {}", e))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Image Generation Command
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateImageArgs {
    pub prompt: String,
    pub size: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub input_images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageGenerationResult {
    pub url: Option<String>,
    pub b64_json: Option<String>,
    pub revised_prompt: Option<String>,
}

/// Generate an image using Agnes Image 2.1 Flash API
///
/// Returns a JSON string with the image URL or base64 data
#[tauri::command]
async fn generate_image(
    window: tauri::Window,
    state: State<'_, AppState>,
    args: GenerateImageArgs,
) -> Result<String, String> {
    use std::time::Instant;
    let start = Instant::now();

    let emit_progress = |msg: &str| {
        let _ = window.emit("image-progress", msg);
    };

    emit_progress("正在请求生图服务...");

    // Resolve the API key
    let key = state
        .api_key
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?
        .clone()
        .ok_or_else(|| "未配置 API Key，请在设置中填写或通过环境变量 AGNES_API_KEY 提供".to_string())?;

    // Client with explicit timeouts so generation can never hang forever
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(360))
        .build()
        .map_err(|e| format!("HTTP 客户端初始化失败: {}", e))?;

    // NOTE: Agnes 要求 response_format 放在 extra_body 内（顶层会 400），
    // image 输入同样放在 extra_body.image（数组）。URL 输出显式声明 url。
    let mut extra_body = serde_json::Map::new();
    extra_body.insert("response_format".to_string(), serde_json::json!("url"));

    if let Some(images) = &args.input_images {
        let preview: Vec<String> = images
            .iter()
            .map(|s| {
                if s.len() > 200 {
                    format!("{}...[{} bytes]", &s[..200], s.len())
                } else {
                    s.clone()
                }
            })
            .collect();
        eprintln!("[agnes] input_images (preview): {:?}", preview);
        extra_body.insert("image".to_string(), serde_json::to_value(images).map_err(|e| e.to_string())?);
    }

    let request = ImageRequest {
        model: IMAGE_MODEL,
        prompt: args.prompt.clone(),
        size: args.size.clone(),
        ratio: args.ratio.clone(),
        extra_body: if extra_body.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(extra_body))
        },
    };

    eprintln!(
        "[agnes] generate_image: size={} ratio={:?} images={:?} output_format={:?} prompt={:?}",
        args.size, args.ratio, args.input_images.is_some(), args.output_format, &args.prompt[..args.prompt.len().min(60)]
    );

    emit_progress("正在等待生成结果（1:1 高清图可能需要 1-3 分钟）...");

    let resp = client
        .post(format!("{}/images/generations", AGNES_API_BASE))
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {}", e))?;

    eprintln!("[agnes] API respond: status={} elapsed={:.1}s", resp.status().as_u16(), start.elapsed().as_secs_f32());

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("API 返回错误 {}: {}", status.as_u16(), body));
    }

    let response: ImageResponse = resp.json().await.map_err(|e| format!("JSON 解析错误: {}", e))?;

    if let Some(err) = response.error {
        let msg = err.message.unwrap_or_else(|| "未知错误".to_string());
        return Err(format!("生成失败: {}", msg));
    }

    if response.data.is_empty() {
        return Err("API 返回了空结果".to_string());
    }

    let result = &response.data[0];
    let url = result.url.clone();
    let mut b64_json = result.b64_json.clone();

    // If the API returned a raw base64 payload (no data: prefix), wrap it.
    // Otherwise download the original and produce a compressed preview so
    // the webview never has to decode multi-MB images (which can crash it).
    b64_json = Some(match b64_json {
        Some(b) if b.starts_with("data:") => b,
        Some(b) if b.starts_with('/') || !b.is_empty() && !b.starts_with("http") => {
            let mime = if b.starts_with("/9j/") { "image/jpeg" } else { "image/png" };
            format!("data:{};base64,{}", mime, b)
        }
        _ => {
            let u = url.as_deref().ok_or_else(|| "API 未返回图片 URL 或数据".to_string())?;
            eprintln!("[agnes] downloading original: {} elapsed={:.1}s", u, start.elapsed().as_secs_f32());
            emit_progress("生成完成，正在处理图片（下载/压缩）...");
            match download_compressed(&client, u).await {
                Ok(data) => data,
                Err(e) => {
                    // 压缩失败不能导致整体失败：返回空预览，前端将回退到原图 URL 显示
                    eprintln!("[agnes] preview compression failed, fallback to url: {}", e);
                    String::new()
                }
            }
        }
    });

    let output = ImageGenerationResult {
        url,
        b64_json,
        revised_prompt: result.revised_prompt.clone(),
    };

    let out = serde_json::to_string(&output).map_err(|e| format!("序列化错误: {}", e))?;
    eprintln!("[agnes] generate_image done: total={:.1}s payload={} bytes", start.elapsed().as_secs_f32(), out.len());
    Ok(out)
}

const PREVIEW_MAX_DIM: u32 = 1024;

/// Download an image, downscale it to at most PREVIEW_MAX_DIM px on the longest
/// side, re-encode as JPEG and return as `data:` URL. Never returns multi-MB data.
async fn download_compressed(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("图片下载失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("图片下载失败 {}: {}", resp.status().as_u16(), url));
    }

    let bytes = resp.bytes().await.map_err(|e| format!("图片读取失败: {}", e))?;
    eprintln!("[agnes]   downloaded {} bytes", bytes.len());

    // Decode the original
    let img = image::load_from_memory(&bytes).map_err(|e| format!("图片解码失败: {}", e))?;
    eprintln!(
        "[agnes]   decoded {}x{} format={:?}",
        img.width(),
        img.height(),
        img.color()
    );

    // Downscale to preview size
    let (w, h) = (img.width(), img.height());
    let img = if w.max(h) > PREVIEW_MAX_DIM {
        let scale = PREVIEW_MAX_DIM as f32 / w.max(h) as f32;
        img.resize(
            ((w as f32) * scale).max(1.0) as u32,
            ((h as f32) * scale).max(1.0) as u32,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    // Re-encode as JPEG
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
        .encode_image(&img)
        .map_err(|e| format!("图片压缩失败: {}", e))?;
    let out = out.into_inner();
    eprintln!("[agnes]   compressed to {}x{} -> {} bytes", img.width(), img.height(), out.len());

    Ok(format!("data:image/jpeg;base64,{}", base64_encode(&out)))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

// ---------------------------------------------------------------------------
// Video Generation Command (Agnes Video V2.0 — async task API)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VideoCreateArgs {
    pub prompt: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyframe_images: Option<Vec<String>>,
    #[serde(default = "default_resolution")]
    pub resolution: String,
    #[serde(default = "default_aspect")]
    pub aspect_ratio: String,
    #[serde(default = "default_num_frames")]
    pub num_frames: u32,
    #[serde(default = "default_frame_rate")]
    pub frame_rate: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

fn default_resolution() -> String {
    "720p".to_string()
}
fn default_aspect() -> String {
    "16:9".to_string()
}
fn default_num_frames() -> u32 {
    121
}
fn default_frame_rate() -> f32 {
    24.0
}

#[derive(Debug, Deserialize)]
struct VideoCreateResponse {
    #[serde(default)]
    video_id: Option<String>,
    #[serde(default)]
    task_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VideoResultResponse {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    progress: Option<u32>,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    seconds: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    metadata: Option<VideoMetadata>,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct VideoMetadata {
    #[serde(default)]
    url: Option<String>,
}

/// Map a resolution preset + aspect ratio to concrete width/height.
fn compute_size(resolution: &str, aspect: &str) -> (u32, u32) {
    let h: u32 = match resolution {
        "1080p" => 1080,
        "480p" => 480,
        _ => 720,
    };
    let ratio: f64 = match aspect {
        "9:16" => 9.0 / 16.0,
        "1:1" => 1.0,
        "4:3" => 4.0 / 3.0,
        "3:4" => 3.0 / 4.0,
        _ => 16.0 / 9.0,
    };
    let w = (h as f64 * ratio).round() as u32;
    (w, h)
}

/// Snap `num_frames` to the nearest `8n + 1` value, clamped to <= 441.
fn snap_num_frames(n: u32) -> u32 {
    let n = if n == 0 { 121 } else { n };
    let k = ((n as i32 - 1) / 8).clamp(0, 55);
    (8 * k + 1) as u32
}

/// Generate a video via Agnes Video V2.0.
///
/// Creates the async task and returns the `video_id` immediately. A background
/// task then polls for results and emits `video-progress` / `video-complete`
/// events to the frontend.
#[tauri::command]
async fn generate_video(
    window: tauri::Window,
    state: State<'_, AppState>,
    args: VideoCreateArgs,
) -> Result<String, String> {
    let key = resolve_api_key(&state)?;

    let mode = args.mode.clone().unwrap_or_else(|| "text-to-video".to_string());

    let (width, height) = compute_size(&args.resolution, &args.aspect_ratio);

    let mut body = serde_json::Map::new();
    body.insert("model".to_string(), serde_json::json!(VIDEO_MODEL));
    body.insert("prompt".to_string(), serde_json::json!(args.prompt));
    body.insert("width".to_string(), serde_json::json!(width));
    body.insert("height".to_string(), serde_json::json!(height));
    body.insert(
        "num_frames".to_string(),
        serde_json::json!(snap_num_frames(args.num_frames)),
    );
    body.insert("frame_rate".to_string(), serde_json::json!(args.frame_rate));

    if let Some(np) = &args.negative_prompt {
        if !np.trim().is_empty() {
            body.insert("negative_prompt".to_string(), serde_json::json!(np));
        }
    }
    if let Some(seed) = args.seed {
        body.insert("seed".to_string(), serde_json::json!(seed));
    }

    match mode.as_str() {
        "image-to-video" => {
            if let Some(img) = &args.image {
                body.insert("image".to_string(), serde_json::json!(img));
            }
        }
        "keyframes" => {
            let mut extra = serde_json::Map::new();
            if let Some(imgs) = &args.keyframe_images {
                extra.insert("image".to_string(), serde_json::json!(imgs));
            }
            extra.insert("mode".to_string(), serde_json::json!("keyframes"));
            body.insert("extra_body".to_string(), serde_json::Value::Object(extra));
        }
        _ => {}
    }

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/videos", AGNES_API_BASE))
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API 返回错误 {}: {}", status.as_u16(), text));
    }

    let created: VideoCreateResponse = resp
        .json()
        .await
        .map_err(|e| format!("JSON 解析错误: {}", e))?;

    let video_id = created
        .video_id
        .or(created.task_id)
        .ok_or_else(|| "创建任务未返回 video_id".to_string())?;

    // Spawn background polling; events drive the UI.
    let poll_window = window.clone();
    let poll_client = client;
    let poll_id = video_id.clone();
    tauri::async_runtime::spawn(async move {
        poll_video(poll_window, poll_client, poll_id, key).await;
    });

    Ok(video_id)
}

/// Poll the video result endpoint until completion / failure / timeout.
async fn poll_video(window: tauri::Window, client: reqwest::Client, video_id: String, key: String) {
    let base = format!("{}/agnesapi?video_id={}", AGNES_API_BASE, video_id);
    let auth = format!("Bearer {}", key);
    let mut attempts: u32 = 0;

    loop {
        attempts += 1;
        if attempts > 120 {
            emit_video_complete(&window, &video_id, None, None, None, Some("轮询超时，请稍后在历史中查看结果"), None);
            return;
        }

        let result = client
            .get(&base)
            .header("Authorization", &auth)
            .send()
            .await;

        if let Ok(resp) = result {
            if resp.status().is_success() {
                if let Ok(res) = resp.json::<VideoResultResponse>().await {
                    let status = res.status.clone().unwrap_or_default();
                    let progress = res.progress.unwrap_or(0);
                    emit_video_progress(&window, &video_id, &status, progress);

                    if res.error.is_some() {
                        emit_video_complete(&window, &video_id, None, None, None, Some("视频生成失败"), None);
                        return;
                    }
                    if status == "completed" {
                        let size = res.size.clone();
                        let seconds = res.seconds.clone();
                        // Capture the result URL (metadata.url or top-level url).
                        let mut remote = res
                            .metadata
                            .and_then(|m| m.url)
                            .or(res.url);
                        // The URL may take a moment to populate; retry a few times.
                        for _ in 0..4 {
                            if remote.is_some() {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            if let Ok(r) = client.get(&base).header("Authorization", &auth).send().await {
                                if let Ok(rr) = r.json::<VideoResultResponse>().await {
                                    remote = rr.metadata.and_then(|m| m.url).or(rr.url);
                                }
                            }
                        }
                        // Download into a playable data: URL so the webview can render it
                        // regardless of CDN auth/CORS. Fall back to the remote URL if download fails.
                        let playable = match remote.as_ref() {
                            Some(u) => match download_video_as_base64(&client, &auth, u).await {
                                Ok(data) => Some(data),
                                Err(_) => remote.clone(),
                            },
                            None => None,
                        };
                        emit_video_complete(
                            &window,
                            &video_id,
                            playable.as_deref(),
                            size.as_deref(),
                            seconds.as_deref(),
                            None,
                            remote.as_deref(),
                        );
                        return;
                    }
                    if status == "failed" {
                        emit_video_complete(&window, &video_id, None, None, None, Some("视频生成失败"), None);
                        return;
                    }
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}

fn emit_video_progress(window: &tauri::Window, video_id: &str, status: &str, progress: u32) {
    let payload = serde_json::json!({
        "videoId": video_id,
        "status": status,
        "progress": progress,
    });
    let _ = window.emit("video-progress", &payload);
}

fn emit_video_complete(
    window: &tauri::Window,
    video_id: &str,
    url: Option<&str>,
    size: Option<&str>,
    seconds: Option<&str>,
    error: Option<&str>,
    remote_url: Option<&str>,
) {
    let payload = serde_json::json!({
        "videoId": video_id,
        "url": url,
        "size": size,
        "seconds": seconds,
        "error": error,
        "remoteUrl": remote_url,
    });
    let _ = window.emit("video-complete", &payload);
}

/// Download a video URL and return it as a `data:` URL with base64 payload.
/// The API key is sent as a Bearer header in case the CDN requires auth.
async fn download_video_as_base64(client: &reqwest::Client, auth: &str, url: &str) -> Result<String, String> {
    let resp = client
        .get(url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("视频下载失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("视频下载失败 {}: {}", resp.status().as_u16(), url));
    }

    let bytes = resp.bytes().await.map_err(|e| format!("视频读取失败: {}", e))?;
    let mime = sniff_video_mime(&bytes);
    Ok(format!("data:{};base64,{}", mime, base64_encode(&bytes)))
}

/// Guess video MIME type from magic bytes (fallback to video/mp4).
fn sniff_video_mime(bytes: &[u8]) -> &'static str {
    // WebM starts with EBML header 0x1A 0x45 0xDF 0xA3
    if bytes.len() >= 4 && bytes[0] == 0x1A && bytes[1] == 0x45 && bytes[2] == 0xDF && bytes[3] == 0xA3 {
        "video/webm"
    } else {
        "video/mp4"
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            app.manage(AppState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_api_key,
            set_api_key,
            get_chat_config,
            set_chat_config,
            send_chat_message,
            generate_image,
            generate_video,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run agnes-chat app");
}
