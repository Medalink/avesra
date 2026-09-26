//! Incremental OpenAI-compatible SSE decoding, not backend completion proof.
use avesra_contracts::{ErrorCode, planner};
use serde::Deserialize;
use uuid::Uuid;
const MAX_EVENT: usize = 65_536;
const MAX_TOTAL: usize = 1_048_576;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
    #[serde(default)]
    system_fingerprint: Option<String>,
    #[serde(default)]
    service_tier: Option<String>,
    #[serde(default)]
    prompt_token_ids: Option<serde_json::Value>,
    #[serde(default)]
    prompt_text: Option<serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Usage {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    #[serde(default)]
    prompt_tokens_details: Option<serde_json::Value>,
    #[serde(default)]
    completion_tokens_details: Option<serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Choice {
    index: u64,
    delta: Delta,
    finish_reason: Option<String>,
    #[serde(default)]
    logprobs: Option<serde_json::Value>,
    #[serde(default)]
    token_ids: Option<serde_json::Value>,
    #[serde(default)]
    stop_reason: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Delta {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default, alias = "reasoning")]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<serde_json::Value>,
    #[serde(default)]
    function_call: Option<serde_json::Value>,
}
/// This observation is only a well-formed upstream stream terminal. Deployment
/// qualification must establish its relationship to actual engine completion.
pub struct CompletedStream {
    request: Uuid,
    model: String,
    response: Result<String, ErrorCode>,
}
impl CompletedStream {
    pub fn request(&self) -> Uuid {
        self.request
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn response(self) -> Result<planner::Response, ErrorCode> {
        let value: planner::Response =
            serde_json::from_str(&self.response?).map_err(|_| ErrorCode::Malformed)?;
        value.validate()?;
        Ok(value)
    }
    pub fn classification(self) -> Result<avesra_contracts::directedness::Category, ErrorCode> {
        let value: avesra_contracts::directedness::Classification =
            serde_json::from_str(&self.response?).map_err(|_| ErrorCode::Malformed)?;
        Ok(value.category)
    }
}
pub struct Parser {
    request: Uuid,
    model: String,
    id: Option<String>,
    line: Vec<u8>,
    data: String,
    answer: String,
    total: usize,
    events: usize,
    reasoning_bytes: usize,
    finish: Option<String>,
    done: bool,
    failed: bool,
}
impl Parser {
    pub fn new(request: Uuid, model: &str) -> Result<Self, ErrorCode> {
        if request.is_nil()
            || model.is_empty()
            || model.len() > 128
            || !model
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'-' | b'_' | b'.'))
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(Self {
            request,
            model: model.into(),
            id: None,
            line: Vec::new(),
            data: String::new(),
            answer: String::new(),
            total: 0,
            events: 0,
            reasoning_bytes: 0,
            finish: None,
            done: false,
            failed: false,
        })
    }
    /// An error poisons the parser permanently; a later DONE cannot repair it.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), ErrorCode> {
        if self.failed {
            return Err(ErrorCode::Stale);
        }
        let result = self.consume(bytes);
        if result.is_err() {
            self.failed = true;
            self.line.clear();
            self.data.clear();
            self.answer.clear();
        }
        result
    }
    fn consume(&mut self, bytes: &[u8]) -> Result<(), ErrorCode> {
        self.total = self
            .total
            .checked_add(bytes.len())
            .ok_or(ErrorCode::TooLarge)?;
        if self.total > MAX_TOTAL || bytes.len() > MAX_EVENT {
            return Err(ErrorCode::TooLarge);
        }
        for byte in bytes {
            if *byte == b'\n' {
                let mut line = std::mem::take(&mut self.line);
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                let text = std::str::from_utf8(&line).map_err(|_| ErrorCode::Malformed)?;
                if text.is_empty() {
                    self.event()?;
                } else if text.starts_with(':') { /* bounded keepalive only */
                } else if let Some(data) = text.strip_prefix("data:") {
                    if self.done {
                        return Err(ErrorCode::Malformed);
                    }
                    let data = data.strip_prefix(' ').unwrap_or(data);
                    if self.data.len() + data.len() + 1 > MAX_EVENT {
                        return Err(ErrorCode::TooLarge);
                    }
                    if !self.data.is_empty() {
                        self.data.push('\n');
                    }
                    self.data.push_str(data);
                } else {
                    return Err(ErrorCode::Malformed);
                }
            } else {
                if self.line.len() >= MAX_EVENT {
                    return Err(ErrorCode::TooLarge);
                }
                self.line.push(*byte);
            }
        }
        Ok(())
    }
    fn event(&mut self) -> Result<(), ErrorCode> {
        if self.data.is_empty() {
            return Ok(());
        }
        let data = std::mem::take(&mut self.data);
        self.events = self.events.checked_add(1).ok_or(ErrorCode::TooLarge)?;
        if self.events > 4096 {
            return Err(ErrorCode::TooLarge);
        }
        if data == "[DONE]" {
            if self.done || self.finish.is_none() || self.id.is_none() {
                return Err(ErrorCode::Malformed);
            }
            self.done = true;
            return Ok(());
        }
        if self.done {
            return Err(ErrorCode::Malformed);
        }
        let event: Event = serde_json::from_str(&data).map_err(|_| ErrorCode::Malformed)?;
        if event.object != "chat.completion.chunk"
            || event.prompt_token_ids.is_some()
            || event.prompt_text.is_some()
            || event.model != self.model
            || event.id.is_empty()
            || event.id.len() > 128
            || !event
                .id
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'-' | b'_' | b'.'))
            || event.created > avesra_contracts::browser::MAX_SAFE_COUNTER
            || event
                .system_fingerprint
                .as_ref()
                .is_some_and(|v| v.len() > 256 || v.chars().any(char::is_control))
            || event
                .service_tier
                .as_ref()
                .is_some_and(|v| v.len() > 32 || v.chars().any(char::is_control))
        {
            return Err(ErrorCode::Malformed);
        }
        if self.id.as_ref().is_some_and(|id| *id != event.id) {
            return Err(ErrorCode::Stale);
        }
        self.id = Some(event.id);
        if let Some(usage) = &event.usage {
            if usage.prompt_tokens > 1_000_000
                || usage.completion_tokens > 1_000_000
                || usage.total_tokens
                    != usage
                        .prompt_tokens
                        .checked_add(usage.completion_tokens)
                        .ok_or(ErrorCode::Malformed)?
            {
                return Err(ErrorCode::Malformed);
            }
            for details in [
                &usage.prompt_tokens_details,
                &usage.completion_tokens_details,
            ]
            .into_iter()
            .flatten()
            {
                let object = details.as_object().ok_or(ErrorCode::Malformed)?;
                if object.len() > 16
                    || object.iter().any(|(key, value)| {
                        key.len() > 64 || value.as_u64().is_none_or(|v| v > 1_000_000)
                    })
                {
                    return Err(ErrorCode::Malformed);
                }
            }
        }
        if event.choices.is_empty() {
            return if event.usage.is_some() && self.finish.is_some() {
                Ok(())
            } else {
                Err(ErrorCode::Malformed)
            };
        }
        if event.choices.len() != 1 || self.finish.is_some() {
            return Err(ErrorCode::Malformed);
        }
        let choice = event
            .choices
            .into_iter()
            .next()
            .ok_or(ErrorCode::Malformed)?;
        if choice.index != 0
            || choice.logprobs.is_some()
            || choice.token_ids.is_some()
            || choice.stop_reason.is_some_and(|token| {
                !matches!(token, 248046 | 248044) || choice.finish_reason.as_deref() != Some("stop")
            })
            || choice.delta.role.as_ref().is_some_and(|v| v != "assistant")
            || choice.delta.tool_calls.is_some()
            || choice.delta.function_call.is_some()
        {
            return Err(ErrorCode::Unsupported);
        }
        if let Some(reasoning) = choice.delta.reasoning_content {
            self.reasoning_bytes = self
                .reasoning_bytes
                .checked_add(reasoning.len())
                .ok_or(ErrorCode::TooLarge)?;
            if self.reasoning_bytes > 65_536
                || reasoning
                    .chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
            {
                return Err(ErrorCode::TooLarge);
            }
            // Model reasoning is discarded; never emitted or retained as history.
        }
        if let Some(text) = choice.delta.content {
            if self.answer.len() + text.len() > 65_536 {
                return Err(ErrorCode::TooLarge);
            }
            self.answer.push_str(&text);
        }
        if let Some(finish) = choice.finish_reason {
            if !matches!(finish.as_str(), "stop" | "length" | "content_filter") {
                return Err(ErrorCode::Unsupported);
            }
            self.finish = Some(finish);
        }
        Ok(())
    }
    /// Call only at upstream EOF; a local timeout is not a complete stream.
    pub fn finish(self) -> Result<CompletedStream, ErrorCode> {
        if self.failed || !self.done || !self.data.is_empty() || !self.line.is_empty() {
            return Err(ErrorCode::Unavailable);
        }
        let response = if self.finish.as_deref() != Some("stop") {
            Err(ErrorCode::Unavailable)
        } else {
            Ok(self.answer)
        };
        Ok(CompletedStream {
            request: self.request,
            model: self.model,
            response,
        })
    }
}
