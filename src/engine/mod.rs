use std::collections::HashMap;
use std::io::{self, Write};

use futures_util::StreamExt;

use crate::config::Config;
use crate::deepseek::client::DeepSeekClient;
use crate::deepseek::types::{
    ChatCompletionRequest, Message, Tool as ApiTool, FunctionDefinition, ToolCall, FunctionCall
};
use crate::context::ContextManager;
use crate::memory::MemoryManager;
use crate::tools::Tool;
use crate::tools::file_tools::{ViewFileTool, WriteFileTool, PatchFileTool, ListDirectoryTool, GrepSearchTool};
use crate::tools::cmd_tool::RunCommandTool;

pub struct Agent {
    client: DeepSeekClient,
    pub context_manager: ContextManager,
    pub memory_manager: MemoryManager,
    tools: HashMap<String, Box<dyn Tool>>,
    model: String,
    max_tokens: u32,
}

impl Agent {
    pub fn new(config: Config) -> Self {
        let client = DeepSeekClient::new(config.api_key, config.base_url);
        let context_manager = ContextManager::new(64000); // 64K context window
        let memory_manager = MemoryManager::new();
        
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();
        tools.insert("view_file".to_string(), Box::new(ViewFileTool));
        tools.insert("write_file".to_string(), Box::new(WriteFileTool));
        tools.insert("patch_file".to_string(), Box::new(PatchFileTool));
        tools.insert("list_directory".to_string(), Box::new(ListDirectoryTool));
        tools.insert("grep_search".to_string(), Box::new(GrepSearchTool));
        tools.insert("run_command".to_string(), Box::new(RunCommandTool));

        Self {
            client,
            context_manager,
            memory_manager,
            tools,
            model: config.model,
            max_tokens: config.max_tokens,
        }
    }

    /// Build the complete system prompt injecting instructions and long-term memory facts
    fn build_system_prompt(&self) -> String {
        let mut prompt = String::new();
        prompt.push_str(
            "You are agent-rust, an advanced local software development agent. \
            You help the user write, modify, debug, and test code on their system.\n\n\
            You have access to local file system and execution tools. \
            When editing code, ALWAYS check if there are compilation errors or test failures by running the tests.\n\n"
        );

        // Inject learned facts from memory
        let facts = self.memory_manager.get_facts();
        if !facts.is_empty() {
            prompt.push_str("Here are facts about the project and preferences you learned in past sessions:\n");
            for fact in facts {
                prompt.push_str(&format!("- {}\n", fact));
            }
            prompt.push('\n');
        }

        prompt.push_str(
            "Guidelines:\n\
            1. Verify your edits immediately using local tools like `cargo test` or `cargo check` if applicable.\n\
            2. When using `patch_file`, ensure you provide an exact match of the target block, including spaces and newlines.\n\
            3. Answer clearly, directly, and keep text explanations minimal if code can speak for itself.\n"
        );

        prompt
    }

    /// Formulate API-compatible tool definitions from local tool schemas
    fn get_api_tools(&self) -> Vec<ApiTool> {
        self.tools.values().map(|tool| ApiTool {
            r#type: "function".to_string(),
            function: FunctionDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters_schema(),
            },
        }).collect()
    }

    /// Prompts user to confirm high-risk tool actions (e.g., executing commands)
    fn get_user_confirmation(&self, tool_name: &str, args: &str) -> bool {
        println!("\n🛡️  [TOOL APPROVAL REQUIRED] The agent wants to execute: `{}`", tool_name);
        println!("Arguments: {}", args);
        print!("Approve execution? [y/N]: ");
        let _ = io::stdout().flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let trimmed = input.trim().to_lowercase();
            trimmed == "y" || trimmed == "yes"
        } else {
            false
        }
    }

    /// Process a single turn in the agent-user conversation
    pub async fn run_turn(&mut self, user_input: &str) -> Result<(), String> {
        // Log user command in history if it's a code request
        if !user_input.starts_with('/') {
            let _ = self.memory_manager.add_command(user_input.to_string());
        }

        // Add user message to history
        self.context_manager.add_message(Message::user(user_input));

        loop {
            // Re-compile system prompt with latest memory and file contexts
            let system_prompt = self.build_system_prompt();
            let files_prompt = self.context_manager.build_files_prompt().unwrap_or_default();
            
            let mut combined_system = system_prompt.clone();
            if !files_prompt.is_empty() {
                combined_system.push_str(&files_prompt);
            }

            // Truncate history to stay within budget
            self.context_manager.truncate_history(&combined_system, "");

            // Assemble message payload
            let mut messages = vec![Message::system(combined_system)];
            messages.extend(self.context_manager.get_chat_history().iter().cloned());

            // Build request
            let request = ChatCompletionRequest {
                model: self.model.clone(),
                messages,
                temperature: Some(0.2), // Low temperature for precise code edits
                max_tokens: Some(self.max_tokens),
                top_p: None,
                stream: Some(true),
                tools: Some(self.get_api_tools()),
                tool_choice: None,
            };

            print!("\nagent-rust > ");
            let _ = io::stdout().flush();

            // Send request and stream response
            let mut stream = self.client.send_chat_completion_stream(request).await
                .map_err(|e| format!("DeepSeek Client error: {}", e))?;

            let mut assistant_text = String::new();
            let mut tool_calls_builder: HashMap<usize, (Option<String>, Option<String>, String)> = HashMap::new();

            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result.map_err(|e| format!("Error in response stream: {}", e))?;
                
                if chunk.choices.is_empty() {
                    continue;
                }
                
                let choice = &chunk.choices[0];
                
                // 1. Text content delta
                if let Some(ref text) = choice.delta.content {
                    print!("{}", text);
                    let _ = io::stdout().flush();
                    assistant_text.push_str(text);
                }

                // 2. Tool calls delta
                if let Some(ref tool_calls) = choice.delta.tool_calls {
                    for tc in tool_calls {
                        let idx = tc.index;
                        let entry = tool_calls_builder.entry(idx).or_insert((None, None, String::new()));
                        
                        if let Some(ref id) = tc.id {
                            entry.0 = Some(id.clone());
                        }

                        if let Some(ref func) = tc.function.name {
                            entry.1 = Some(func.clone());
                        }
                        if let Some(ref args) = tc.function.arguments {
                            entry.2.push_str(args);
                        }
                    }
                }
            }

            println!(); // Add newline after streaming ends

            // Reconstruct tool calls if any were parsed
            let mut final_tool_calls = Vec::new();
            for (_idx, (id_opt, name_opt, args)) in tool_calls_builder {
                if let (Some(id), Some(name)) = (id_opt, name_opt) {
                    final_tool_calls.push(ToolCall {
                        id,
                        r#type: "function".to_string(),
                        function: FunctionCall {
                            name,
                            arguments: args,
                        },
                    });
                }
            }

            // Save assistant response to history
            let assistant_msg = Message::assistant(
                if assistant_text.is_empty() { None } else { Some(assistant_text.clone()) },
                if final_tool_calls.is_empty() { None } else { Some(final_tool_calls.clone()) }
            );
            self.context_manager.add_message(assistant_msg);

            // If no tool calls, this conversation turn is complete!
            if final_tool_calls.is_empty() {
                break;
            }

            // Execute tool calls and feed back outputs
            for tool_call in final_tool_calls {
                let tool_name = &tool_call.function.name;
                let tool_args = &tool_call.function.arguments;
                
                println!("⚙️  [Executing Tool] `{}`", tool_name);

                let is_high_risk = tool_name == "run_command" || tool_name == "write_file";
                let confirmed = if is_high_risk {
                    self.get_user_confirmation(tool_name, tool_args)
                } else {
                    true
                };

                let tool_result = if !confirmed {
                    println!("❌  Tool execution cancelled by user.");
                    "Error: Execution cancelled by user.".to_string()
                } else {
                    match self.tools.get(tool_name) {
                        Some(tool) => match tool.execute(tool_args).await {
                            Ok(output) => {
                                println!("✅  Tool executed successfully.");
                                output
                            }
                            Err(err) => {
                                println!("⚠️  Tool executed with error: {}", err);
                                format!("Error: {}", err)
                            }
                        },
                        None => {
                            println!("⚠️  Unknown tool: {}", tool_name);
                            format!("Error: Unknown tool: {}", tool_name)
                        }
                    }
                };

                // Add tool result to chat history
                self.context_manager.add_message(Message::tool(tool_call.id, tool_result));
            }

            // Loop continues: we'll call the LLM again with the tool results included in the message history.
        }

        Ok(())
    }
}
