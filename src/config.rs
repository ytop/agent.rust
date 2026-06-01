use clap::Parser;
use std::env;
use std::fs;
use std::path::PathBuf;

const DEFAULT_MODEL: &str = "deepseek-chat";
const DEFAULT_MAX_TOKENS: u32 = 4096;

#[derive(Parser, Debug, Clone)]
#[command(name = "agent-rust")]
#[command(author = "Dana Oshu")]
#[command(version = "0.1.0")]
#[command(about = "Rust-based code agent integrated with DeepSeek", long_about = None)]
pub struct CliArgs {
    /// Use Ratatui TUI interactive dashboard interface (default is line-based REPL)
    #[arg(short, long)]
    pub tui: bool,

    /// Specify the DeepSeek model to use
    #[arg(short, long, default_value = DEFAULT_MODEL)]
    pub model: String,

    /// Maximum completion tokens
    #[arg(long, default_value_t = DEFAULT_MAX_TOKENS)]
    pub max_tokens: u32,

    /// Custom DeepSeek API Base URL
    #[arg(short, long)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub max_tokens: u32,
    pub tui: bool,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        let args = CliArgs::parse();
        
        // Find DEEPSEEK_API_KEY from environment or from ~/.config/agent-rust/config.toml
        let api_key = match env::var("DEEPSEEK_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                // Try reading from ~/.config/agent-rust/config.toml or ~/.config/agent-rust/key
                if let Some(config_key) = Self::read_key_from_config() {
                    config_key
                } else {
                    return Err(
                        "Error: DEEPSEEK_API_KEY environment variable not set and no key found in ~/.config/agent-rust/config".to_string()
                    );
                }
            }
        };

        Ok(Self {
            api_key,
            base_url: args.base_url,
            model: args.model,
            max_tokens: args.max_tokens,
            tui: args.tui,
        })
    }
}

// Let's implement read_key_from_config safely using std::env:
fn get_home_dir() -> Option<PathBuf> {
    env::var("HOME")
        .map(PathBuf::from)
        .or_else(|_| env::var("USERPROFILE").map(PathBuf::from))
        .ok()
}

impl Config {
    fn read_key_from_config() -> Option<String> {
        let home = get_home_dir()?;
        let config_path = home.join(".config").join("agent-rust").join("key");
        if config_path.exists() {
            if let Ok(key) = fs::read_to_string(config_path) {
                let trimmed = key.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }
        None
    }
}
