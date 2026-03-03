use async_trait::async_trait;
use ferroclaw_core::{FerroError, Message, MessageContent, Role, ToolCall};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_stream::Stream;
use tracing::{debug, warn};

use crate::provider::{LlmChunk, LlmProvider, LlmResponse};

pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
            model,
        }
    }
}

// ── Internal request/response types ──────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAiMessage>,
    tools: &'a [Value],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    /// For role=tool
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    function: OpenAiFunction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAiFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: OpenAiMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ChatStreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize, Debug)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<StreamToolCallDelta>>,
}

#[derive(Deserialize, Debug, Clone)]
struct StreamToolCallDelta {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    function: Option<StreamFunctionDelta>,
}

#[derive(Deserialize, Debug, Clone)]
struct StreamFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

// ── Conversion helpers ────────────────────────────────────────────────────────

fn to_openai_messages(messages: &[Message]) -> Vec<OpenAiMessage> {
    let mut result = Vec::new();
    for msg in messages {
        match (&msg.role, &msg.content) {
            (Role::System, MessageContent::Text(t)) => {
                result.push(OpenAiMessage {
                    role: "system".into(),
                    content: Some(t.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
            }
            (Role::User, MessageContent::Text(t)) => {
                result.push(OpenAiMessage {
                    role: "user".into(),
                    content: Some(t.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
            }
            (Role::Assistant, MessageContent::Text(t)) => {
                result.push(OpenAiMessage {
                    role: "assistant".into(),
                    content: Some(t.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
            }
            (Role::Assistant, MessageContent::ToolCalls(calls)) => {
                let tool_calls = calls
                    .iter()
                    .map(|c| OpenAiToolCall {
                        id: c.id.clone(),
                        kind: "function".into(),
                        function: OpenAiFunction {
                            name: c.name.clone(),
                            arguments: c.input.to_string(),
                        },
                    })
                    .collect();
                result.push(OpenAiMessage {
                    role: "assistant".into(),
                    content: None,
                    tool_calls: Some(tool_calls),
                    tool_call_id: None,
                    name: None,
                });
            }
            (Role::Tool, MessageContent::ToolResults(results)) => {
                for r in results {
                    result.push(OpenAiMessage {
                        role: "tool".into(),
                        content: Some(r.output.clone()),
                        tool_calls: None,
                        tool_call_id: Some(r.tool_call_id.clone()),
                        name: Some(r.tool_name.clone()),
                    });
                }
            }
            _ => {
                warn!("Skipping unsupported message role/content combination");
            }
        }
    }
    result
}

fn parse_finish_tool_calls(calls: Vec<OpenAiToolCall>) -> Vec<ToolCall> {
    calls
        .into_iter()
        .map(|c| {
            let input = serde_json::from_str(&c.function.arguments).unwrap_or(json!({}));
            ToolCall {
                id: c.id,
                name: c.function.name,
                input,
            }
        })
        .collect()
}

// ── LlmProvider impl ──────────────────────────────────────────────────────────

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn model_name(&self) -> &str {
        &self.model
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: &[Value],
    ) -> Result<LlmResponse, FerroError> {
        let req_body = ChatRequest {
            model: &self.model,
            messages: to_openai_messages(messages),
            tools,
            stream: false,
            max_tokens: Some(4096),
        };

        debug!(model = %self.model, "sending completion request");

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| FerroError::LlmProvider(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(FerroError::LlmProvider(format!(
                "HTTP {status}: {body}"
            )));
        }

        let data: ChatResponse = resp
            .json()
            .await
            .map_err(|e| FerroError::LlmProvider(e.to_string()))?;

        let choice = data
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| FerroError::LlmProvider("empty choices".into()))?;

        let stop_reason = choice.finish_reason.clone();
        let tool_calls = choice
            .message
            .tool_calls
            .map(parse_finish_tool_calls)
            .unwrap_or_default();

        Ok(LlmResponse {
            text: choice.message.content,
            tool_calls,
            stop_reason,
        })
    }

    fn complete_stream<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [Value],
    ) -> impl Stream<Item = Result<LlmChunk, FerroError>> + Send + 'a {
        async_stream::stream! {
            let req_body = ChatRequest {
                model: &self.model,
                messages: to_openai_messages(messages),
                tools,
                stream: true,
                max_tokens: Some(4096),
            };

            let resp = self
                .client
                .post(format!("{}/chat/completions", self.base_url))
                .bearer_auth(&self.api_key)
                .json(&req_body)
                .send()
                .await;

            let resp = match resp {
                Ok(r) => r,
                Err(e) => {
                    yield Err(FerroError::LlmProvider(e.to_string()));
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                yield Err(FerroError::LlmProvider(format!("HTTP {status}: {body}")));
                return;
            }

            let mut byte_stream = resp.bytes_stream();
            // Accumulate partial tool call deltas by index
            let mut tool_call_acc: std::collections::HashMap<usize, (String, String, String)> =
                std::collections::HashMap::new();

            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk {
                    Ok(b) => b,
                    Err(e) => {
                        yield Err(FerroError::LlmProvider(e.to_string()));
                        return;
                    }
                };

                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    let line = line.trim();
                    if line == "data: [DONE]" {
                        // Flush accumulated tool calls
                        if !tool_call_acc.is_empty() {
                            let mut calls: Vec<_> = tool_call_acc.drain().collect();
                            calls.sort_by_key(|(idx, _)| *idx);
                            let tool_calls = calls
                                .into_iter()
                                .map(|(_, (id, name, args))| {
                                    let input = serde_json::from_str(&args).unwrap_or(json!({}));
                                    ToolCall { id, name, input }
                                })
                                .collect::<Vec<_>>();
                            yield Ok(LlmChunk::ToolCalls(tool_calls));
                        }
                        yield Ok(LlmChunk::Done(None));
                        return;
                    }
                    if let Some(data) = line.strip_prefix("data: ") {
                        match serde_json::from_str::<ChatStreamChunk>(data) {
                            Ok(chunk) => {
                                for choice in chunk.choices {
                                    if let Some(text) = choice.delta.content {
                                        if !text.is_empty() {
                                            yield Ok(LlmChunk::Text(text));
                                        }
                                    }
                                    if let Some(tc_deltas) = choice.delta.tool_calls {
                                        for delta in tc_deltas {
                                            let entry = tool_call_acc
                                                .entry(delta.index)
                                                .or_insert_with(|| (String::new(), String::new(), String::new()));
                                            if let Some(id) = delta.id {
                                                entry.0 = id;
                                            }
                                            if let Some(func) = delta.function {
                                                if let Some(name) = func.name {
                                                    entry.1 = name;
                                                }
                                                if let Some(args) = func.arguments {
                                                    entry.2.push_str(&args);
                                                }
                                            }
                                        }
                                    }
                                    if let Some(reason) = choice.finish_reason {
                                        if reason == "tool_calls" {
                                            // Will be flushed at [DONE]
                                        } else {
                                            yield Ok(LlmChunk::Done(Some(reason)));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                debug!("SSE parse error (may be ok): {e}");
                            }
                        }
                    }
                }
            }
        }
    }
}
