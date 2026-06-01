use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use crate::deepseek::types::Message;

pub struct ContextManager {
    active_files: HashSet<PathBuf>,
    chat_history: Vec<Message>,
    max_context_tokens: usize,
}

impl ContextManager {
    pub fn new(max_context_tokens: usize) -> Self {
        Self {
            active_files: HashSet::new(),
            chat_history: Vec::new(),
            max_context_tokens,
        }
    }

    /// Add a file to active context. Returns true if it was added, false if already present.
    pub fn add_file(&mut self, path: PathBuf) -> bool {
        self.active_files.insert(path)
    }

    /// Remove a file from active context. Returns true if it was removed, false if not present.
    pub fn drop_file(&mut self, path: &Path) -> bool {
        self.active_files.remove(path)
    }

    pub fn get_active_files(&self) -> &HashSet<PathBuf> {
        &self.active_files
    }

    pub fn clear_files(&mut self) {
        self.active_files.clear();
    }

    /// Estimates token count based on standard heuristic (approx. 3 characters per token for code/text)
    pub fn estimate_tokens(text: &str) -> usize {
        text.chars().count() / 3
    }

    /// Estimates tokens for a single Message structure
    pub fn estimate_message_tokens(msg: &Message) -> usize {
        let mut tokens = 0;
        if let Some(ref content) = msg.content {
            tokens += Self::estimate_tokens(content);
        }
        if let Some(ref tool_calls) = msg.tool_calls {
            for tc in tool_calls {
                tokens += Self::estimate_tokens(&tc.function.name);
                tokens += Self::estimate_tokens(&tc.function.arguments);
            }
        }
        if let Some(ref tool_call_id) = msg.tool_call_id {
            tokens += Self::estimate_tokens(tool_call_id);
        }
        // Message metadata overhead
        tokens + 4
    }

    /// Formats all active files into an XML container block for injection into LLM system prompt
    pub fn build_files_prompt(&self) -> Result<String, std::io::Error> {
        if self.active_files.is_empty() {
            return Ok(String::new());
        }

        let mut prompt = String::new();
        prompt.push_str("\n<active_files_context>\n");
        prompt.push_str("Here are the contents of the files currently added to your context:\n\n");

        for file_path in &self.active_files {
            let relative_path = file_path.to_string_lossy().to_string();
            
            match fs::read_to_string(file_path) {
                Ok(content) => {
                    prompt.push_str(&format!("<file path=\"{}\">\n", relative_path));
                    prompt.push_str(&content);
                    if !content.ends_with('\n') {
                        prompt.push('\n');
                    }
                    prompt.push_str("</file>\n\n");
                }
                Err(e) => {
                    prompt.push_str(&format!("<file path=\"{}\" error=\"Failed to read file: {}\" />\n\n", relative_path, e));
                }
            }
        }

        prompt.push_str("</active_files_context>\n");
        Ok(prompt)
    }

    pub fn add_message(&mut self, message: Message) {
        self.chat_history.push(message);
    }

    pub fn get_chat_history(&self) -> &[Message] {
        &self.chat_history
    }

    pub fn mut_chat_history(&mut self) -> &mut Vec<Message> {
        &mut self.chat_history
    }

    pub fn clear_chat(&mut self) {
        self.chat_history.clear();
    }

    /// Calculates total tokens of current chat history plus the base prompts
    pub fn calculate_total_tokens(&self, system_prompt: &str, files_prompt: &str) -> usize {
        let base_tokens = Self::estimate_tokens(system_prompt) + Self::estimate_tokens(files_prompt);
        let history_tokens: usize = self.chat_history.iter().map(Self::estimate_message_tokens).sum();
        base_tokens + history_tokens
    }

    /// Truncates conversation history if total tokens exceed `max_context_tokens`.
    /// Preserves system prompts, files context, and the latest messages.
    /// Returns the number of removed messages.
    pub fn truncate_history(&mut self, system_prompt: &str, files_prompt: &str) -> usize {
        let base_tokens = Self::estimate_tokens(system_prompt) + Self::estimate_tokens(files_prompt);
        
        // If system + files already exceed/equal limit, we can't fit any history
        if base_tokens >= self.max_context_tokens {
            let removed = self.chat_history.len();
            self.chat_history.clear();
            return removed;
        }

        let allowed_history_tokens = self.max_context_tokens - base_tokens;
        let mut history_tokens = 0;
        let mut keep_count = 0;

        // Iterate backwards from the latest message to count how many we can retain
        for msg in self.chat_history.iter().rev() {
            let msg_tokens = Self::estimate_message_tokens(msg);
            if history_tokens + msg_tokens > allowed_history_tokens {
                break;
            }
            history_tokens += msg_tokens;
            keep_count += 1;
        }

        if keep_count < self.chat_history.len() {
            let remove_count = self.chat_history.len() - keep_count;
            // Drain elements from the front of the vector
            self.chat_history.drain(0..remove_count);
            remove_count
        } else {
            0
        }
    }
}
