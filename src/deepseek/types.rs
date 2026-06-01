use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: Some(content.into()),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(content.into()),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn assistant(content: Option<String>, tool_calls: Option<Vec<ToolCall>>) -> Self {
        Self {
            role: Role::Assistant,
            content,
            name: None,
            tool_call_id: None,
            tool_calls,
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(content.into()),
            name: None,
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String, // "function"
    pub function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON-encoded string
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Tool {
    pub r#type: String, // "function"
    pub function: FunctionDefinition,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Choice {
    pub index: usize,
    pub message: Message,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// Streaming structures
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChoiceChunk>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChoiceChunk {
    pub index: usize,
    pub delta: DeltaMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeltaMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<DeltaToolCall>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeltaToolCall {
    pub index: usize,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    
    pub function: DeltaFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeltaFunctionCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}
