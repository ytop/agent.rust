pub mod config;
pub mod deepseek;
pub mod context;
pub mod memory;
pub mod tools;
pub mod engine;
pub mod ui;

#[tokio::main]
async fn main() {
    // Load config
    let config = match config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    // Initialize agent
    let is_tui = config.tui;
    let agent = engine::Agent::new(config);

    if is_tui {
        if let Err(e) = ui::tui::run_tui(agent).await {
            eprintln!("TUI Error: {}", e);
            std::process::exit(1);
        }
    } else {
        if let Err(e) = ui::repl::run_repl(agent).await {
            eprintln!("REPL Error: {}", e);
            std::process::exit(1);
        }
    }
}
