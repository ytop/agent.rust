use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use serde::Deserialize;
use serde_json::{json, Value};
use crate::tools::Tool;

pub struct RunCommandTool;

#[derive(Deserialize)]
struct RunCommandArgs {
    command: String,
}

impl Tool for RunCommandTool {
    fn name(&self) -> &str {
        "run_command"
    }

    fn description(&self) -> &str {
        "Execute a shell command locally. Real-time streaming of stdout/stderr is printed directly. High safety verification required."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The exact shell command to execute (e.g., 'cargo test', 'git status')." }
            },
            "required": ["command"]
        })
    }

    fn execute(&self, arguments: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>> {
        let arguments = arguments.to_string();
        Box::pin(async move {
            let args: RunCommandArgs = serde_json::from_str(&arguments)
                .map_err(|e| format!("Failed to parse arguments: {}", e))?;

            let mut child = Command::new("sh")
                .arg("-c")
                .arg(&args.command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to spawn command process: {}", e))?;

            let stdout = child.stdout.take().ok_or("Failed to open stdout pipe")?;
            let stderr = child.stderr.take().ok_or("Failed to open stderr pipe")?;

            let mut stdout_reader = BufReader::new(stdout).lines();
            let mut stderr_reader = BufReader::new(stderr).lines();

            let mut output_log = String::new();

            // Separate tasks or loop to stream in real-time
            let stdout_handle = tokio::spawn(async move {
                let mut buffer = String::new();
                let mut count = 0;
                while let Ok(Some(line)) = stdout_reader.next_line().await {
                    println!("{}", line);
                    buffer.push_str(&line);
                    buffer.push('\n');
                    count += 1;
                    if count > 1000 {
                        // Safety check to prevent infinite log loops
                        break;
                    }
                }
                buffer
            });

            let stderr_handle = tokio::spawn(async move {
                let mut buffer = String::new();
                let mut count = 0;
                while let Ok(Some(line)) = stderr_reader.next_line().await {
                    eprintln!("{}", line);
                    buffer.push_str(&line);
                    buffer.push('\n');
                    count += 1;
                    if count > 1000 {
                        break;
                    }
                }
                buffer
            });

            // Set timeout to 120 seconds
            let wait_result = timeout(Duration::from_secs(120), child.wait()).await;

            match wait_result {
                Ok(Ok(status)) => {
                    let stdout_content = stdout_handle.await.unwrap_or_default();
                    let stderr_content = stderr_handle.await.unwrap_or_default();

                    output_log.push_str(&stdout_content);
                    output_log.push_str(&stderr_content);

                    let exit_code = status.code().unwrap_or(-1);
                    
                    let summary = format!(
                        "Command exited with code: {}\n\nOutput Log:\n{}",
                        exit_code, output_log
                    );
                    
                    Ok(summary)
                }
                Ok(Err(e)) => Err(format!("Command execution failed: {}", e)),
                Err(_) => {
                    // Kill process if timed out
                    let _ = child.kill().await;
                    Err("Command execution timed out after 120 seconds.".to_string())
                }
            }
        })
    }
}
