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
    input_images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_format: Option<String>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
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
    state: State<'_, AppState>,
    args: GenerateImageArgs,
) -> Result<String, String> {
    // Resolve the API key
    let key = state
        .api_key
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?
        .clone()
        .ok_or_else(|| "未配置 API Key，请在设置中填写或通过环境变量 AGNES_API_KEY 提供".to_string())?;

    // Build the request
    let client = reqwest::Client::new();
    
    // Build extra_body for image input and response format
    let mut extra_body = serde_json::Map::new();
    
    if let Some(images) = &args.input_images {
        extra_body.insert("image".to_string(), serde_json::to_value(images).map_err(|e| e.to_string())?);
    }
    
    if let Some(ref format) = args.output_format {
        extra_body.insert("response_format".to_string(), serde_json::json!(format));
    }

    let request = ImageRequest {
        model: IMAGE_MODEL,
        prompt: args.prompt,
        size: args.size,
        ratio: args.ratio,
        input_images: args.input_images,
        output_format: args.output_format,
        extra_body: if extra_body.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(extra_body))
        },
    };

    let resp = client
        .post(format!("{}/images/generations", AGNES_API_BASE))
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

    let response: ImageResponse = resp.json().await.map_err(|e| format!("JSON 解析错误: {}", e))?;

    if let Some(err) = response.error {
        let msg = err.message.unwrap_or_else(|| "未知错误".to_string());
        return Err(format!("生成失败: {}", msg));
    }

    if response.data.is_empty() {
        return Err("API 返回了空结果".to_string());
    }

    let result = &response.data[0];
    let output = ImageGenerationResult {
        url: result.url.clone(),
        b64_json: result.b64_json.clone(),
        revised_prompt: result.revised_prompt.clone(),
    };

    serde_json::to_string(&output).map_err(|e| format!("序列化错误: {}", e))
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
        ])
        .run(tauri::generate_context!())
        .expect("failed to run agnes-chat app");
}
