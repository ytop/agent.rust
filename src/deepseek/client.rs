use std::pin::Pin;
use futures::Stream;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json;
use bytes::Bytes;

use crate::deepseek::types::{ChatCompletionRequest, ChatCompletionResponse, ChatCompletionChunk};

const DEFAULT_BASE_URL: &str = "https://api.deepseek.com/v1";

#[derive(Debug)]
pub enum ClientError {
    Http(reqwest::Error),
    Serialization(serde_json::Error),
    Api(String),
    Stream(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "HTTP error: {}", e),
            Self::Serialization(e) => write!(f, "Serialization error: {}", e),
            Self::Api(e) => write!(f, "API error: {}", e),
            Self::Stream(e) => write!(f, "Streaming error: {}", e),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<reqwest::Error> for ClientError {
    fn from(err: reqwest::Error) -> Self {
        ClientError::Http(err)
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(err: serde_json::Error) -> Self {
        ClientError::Serialization(err)
    }
}

pub struct DeepSeekClient {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl DeepSeekClient {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        let base_url = base_url
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
            .trim_end_matches('/')
            .to_string();

        Self {
            client: reqwest::Client::new(),
            base_url,
            api_key,
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .unwrap_or_else(|_| HeaderValue::from_static("")),
        );
        headers
    }

    pub async fn send_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ClientError> {
        let url = format!("{}/chat/completions", self.base_url);
        
        let response = self.client
            .post(&url)
            .headers(self.headers())
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ClientError::Api(format!(
                "API returned error status {}: {}",
                status, error_text
            )));
        }

        let body = response.json::<ChatCompletionResponse>().await?;
        Ok(body)
    }

    pub async fn send_chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, ClientError>> + Send>>, ClientError> {
        let url = format!("{}/chat/completions", self.base_url);
        
        let mut request = request;
        request.stream = Some(true);

        let response = self.client
            .post(&url)
            .headers(self.headers())
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ClientError::Api(format!(
                "API returned error status {}: {}",
                status, error_text
            )));
        }

        let bytes_stream = response.bytes_stream();
        
        struct StreamState {
            bytes_stream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
            buffer: Vec<u8>,
            pending_lines: Vec<String>,
        }

        let initial_state = StreamState {
            bytes_stream: Box::pin(bytes_stream),
            buffer: Vec::new(),
            pending_lines: Vec::new(),
        };

        let stream = futures::stream::unfold(initial_state, |mut state| async move {
            loop {
                // If we have lines ready, process the first one
                if !state.pending_lines.is_empty() {
                    let line = state.pending_lines.remove(0);
                    let line = line.trim();
                    
                    if line.is_empty() {
                        continue;
                    }
                    
                    if line == "data: [DONE]" {
                        return None; // Stream finished gracefully
                    }
                    
                    if let Some(data) = line.strip_prefix("data: ") {
                        match serde_json::from_str::<ChatCompletionChunk>(data) {
                            Ok(chunk) => {
                                return Some((Ok(chunk), state));
                            }
                            Err(e) => {
                                return Some((Err(ClientError::Serialization(e)), state));
                            }
                        }
                    }
                }

                // Retrieve more bytes from network
                match state.bytes_stream.next().await {
                    Some(Ok(bytes)) => {
                        state.buffer.extend_from_slice(&bytes);
                        
                        // Extract any complete lines from buffer
                        while let Some(pos) = state.buffer.iter().position(|&b| b == b'\n') {
                            let mut line_bytes = state.buffer.drain(..=pos).collect::<Vec<u8>>();
                            if line_bytes.last() == Some(&b'\n') {
                                line_bytes.pop();
                            }
                            if line_bytes.last() == Some(&b'\r') {
                                line_bytes.pop();
                            }
                            if let Ok(line_str) = String::from_utf8(line_bytes) {
                                state.pending_lines.push(line_str);
                            }
                        }
                    }
                    Some(Err(e)) => {
                        return Some((Err(ClientError::Http(e)), state));
                    }
                    None => {
                        // EOF reached
                        if !state.buffer.is_empty() {
                            if let Ok(line_str) = String::from_utf8(state.buffer.clone()) {
                                state.pending_lines.push(line_str);
                            }
                            state.buffer.clear();
                        }
                        
                        if state.pending_lines.is_empty() {
                            return None;
                        }
                    }
                }
            }
        });

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deepseek::types::{Message, Role, ChatCompletionRequest, Tool, FunctionDefinition};
    use serde_json::json;

    #[test]
    fn test_request_serialization() {
        let messages = vec![
            Message::system("You are a helper."),
            Message::user("Hello!"),
        ];
        
        let request = ChatCompletionRequest {
            model: "deepseek-chat".to_string(),
            messages,
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: None,
            stream: Some(false),
            tools: Some(vec![Tool {
                r#type: "function".to_string(),
                function: FunctionDefinition {
                    name: "run_command".to_string(),
                    description: "Runs a command".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "command": { "type": "string" }
                        },
                        "required": ["command"]
                    }),
                },
            }]),
            tool_choice: None,
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let value: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(value["model"], "deepseek-chat");
        assert_eq!(value["messages"][0]["role"], "system");
        assert_eq!(value["messages"][0]["content"], "You are a helper.");
        assert_eq!(value["messages"][1]["role"], "user");
        assert_eq!(value["messages"][1]["content"], "Hello!");
        assert_eq!(value["temperature"], 0.7);
        assert_eq!(value["max_tokens"], 100);
        assert_eq!(value["stream"], false);
        assert_eq!(value["tools"][0]["type"], "function");
        assert_eq!(value["tools"][0]["function"]["name"], "run_command");
    }

    #[test]
    fn test_response_deserialization() {
        let response_json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "deepseek-chat",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello there!"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 9,
                "completion_tokens": 12,
                "total_tokens": 21
            }
        }"#;

        let response: ChatCompletionResponse = serde_json::from_str(response_json).unwrap();
        assert_eq!(response.id, "chatcmpl-123");
        assert_eq!(response.model, "deepseek-chat");
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].message.role, Role::Assistant);
        assert_eq!(response.choices[0].message.content, Some("Hello there!".to_string()));
        assert_eq!(response.choices[0].finish_reason, Some("stop".to_string()));
        let usage = response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 9);
        assert_eq!(usage.completion_tokens, 12);
        assert_eq!(usage.total_tokens, 21);
    }

    #[test]
    fn test_chunk_deserialization() {
        let chunk_json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1677652288,
            "model": "deepseek-chat",
            "choices": [
                {
                    "index": 0,
                    "delta": {
                        "content": "Hello"
                    },
                    "finish_reason": null
                }
            ]
        }"#;

        let chunk: ChatCompletionChunk = serde_json::from_str(chunk_json).unwrap();
        assert_eq!(chunk.id, "chatcmpl-123");
        assert_eq!(chunk.choices.len(), 1);
        assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
        assert_eq!(chunk.choices[0].finish_reason, None);
    }
}
