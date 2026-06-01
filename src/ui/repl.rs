use std::path::PathBuf;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use crate::engine::Agent;

pub async fn run_repl(mut agent: Agent) -> Result<(), String> {
    println!("====================================================");
    println!("     agent-rust : Local AI Software Engineer");
    println!("====================================================");
    println!("Commands:");
    println!("  /add <file>      Add file to context");
    println!("  /drop <file>     Remove file from context");
    println!("  /clear           Clear conversation history");
    println!("  /memory          Show learned project memory facts");
    println!("  /exit            Exit the session");
    println!("====================================================");

    let mut rl = DefaultEditor::new().map_err(|e| format!("Failed to initialize REPL: {}", e))?;
    let _ = rl.load_history("history.txt"); // Load command history file

    loop {
        let readline = rl.readline("\nagent-rust > ");
        match readline {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(trimmed);

                // Handle slash commands
                if trimmed.starts_with('/') {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    let command = parts[0];

                    match command {
                        "/exit" => {
                            println!("Goodbye!");
                            break;
                        }
                        "/clear" => {
                            agent.context_manager.clear_chat();
                            println!("🧹 Conversation history cleared.");
                        }
                        "/memory" => {
                            println!("\n🧠 [Learned Facts Memory]:");
                            let facts = agent.memory_manager.get_facts();
                            if facts.is_empty() {
                                println!("No facts learned yet.");
                            } else {
                                for (idx, fact) in facts.iter().enumerate() {
                                    println!("  [{}] {}", idx, fact);
                                }
                            }
                        }
                        "/add" => {
                            if parts.len() < 2 {
                                println!("Usage: /add <file_path>");
                                continue;
                            }
                            let path = PathBuf::from(parts[1]);
                            if !path.exists() {
                                println!("Error: File not found: {}", parts[1]);
                            } else {
                                if agent.context_manager.add_file(path.clone()) {
                                    println!("✅ Added {} to context.", parts[1]);
                                } else {
                                    println!("File already in context.");
                                }
                            }
                        }
                        "/drop" => {
                            if parts.len() < 2 {
                                println!("Usage: /drop <file_path>");
                                continue;
                            }
                            let path = PathBuf::from(parts[1]);
                            if agent.context_manager.drop_file(&path) {
                                println!("❌ Removed {} from context.", parts[1]);
                            } else {
                                println!("File was not in context.");
                            }
                        }
                        _ => {
                            println!("Unknown command: {}. Type /exit, /clear, /memory, /add, or /drop.", command);
                        }
                    }
                } else {
                    // Send to agent turn
                    if let Err(e) = agent.run_turn(trimmed).await {
                        println!("\n⚠️ Error executing turn: {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("Session interrupted.");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("EOF reached. Goodbye!");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    let _ = rl.save_history("history.txt");
    Ok(())
}
