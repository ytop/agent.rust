use std::io;
use std::path::PathBuf;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};

use crate::engine::Agent;

pub struct TuiApp {
    input: String,
    chat_history: Vec<(String, bool)>, // (text, is_user)
    system_logs: Vec<String>,
    input_mode: bool,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            chat_history: vec![
                ("Welcome to agent-rust TUI dashboard!".to_string(), false),
                ("Type your prompt in the input box below and press Enter.".to_string(), false),
            ],
            system_logs: vec!["System initialized.".to_string()],
            input_mode: true,
        }
    }
}

pub async fn run_tui(mut agent: Agent) -> Result<(), String> {
    // Setup terminal
    enable_raw_mode().map_err(|e| format!("Failed to enable raw mode: {}", e))?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)
        .map_err(|e| format!("Failed to enter alternate screen: {}", e))?;
    
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .map_err(|e| format!("Failed to initialize terminal: {}", e))?;

    let mut app = TuiApp::new();

    // Populate TUI with loaded facts
    for fact in agent.memory_manager.get_facts() {
        app.system_logs.push(format!("Memory: {}", fact));
    }

    loop {
        // Render
        let active_files: Vec<String> = agent.context_manager.get_active_files()
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
            
        terminal.draw(|f| {
            // Main layout split horizontally (Left / Right)
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(60), // Chat and Input
                    Constraint::Percentage(40), // Context and Logs
                ])
                .split(f.size());

            // Left Layout (Split vertically: Chat History / User Input Box)
            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(3),
                ])
                .split(main_chunks[0]);

            // Right Layout (Split vertically: Active Files / Status Logs)
            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Percentage(60),
                ])
                .split(main_chunks[1]);

            // 1. Render Chat History
            let chat_content: Vec<Line> = app.chat_history
                .iter()
                .map(|(msg, is_user)| {
                    let speaker = if *is_user { "User: " } else { "Agent: " };
                    let speaker_style = if *is_user {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    };
                    
                    Line::from(vec![
                        Span::styled(speaker, speaker_style),
                        Span::raw(msg),
                    ])
                })
                .collect();

            let chat_paragraph = Paragraph::new(chat_content)
                .block(Block::default().borders(Borders::ALL).title(" Conversation History "))
                .wrap(Wrap { trim: true });
            f.render_widget(chat_paragraph, left_chunks[0]);

            // 2. Render User Input Box
            let input_style = if app.input_mode {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            
            let input_block = Block::default()
                .borders(Borders::ALL)
                .title(" User Input (Press Esc to quit TUI) ")
                .border_style(input_style);
                
            let input_paragraph = Paragraph::new(app.input.as_str())
                .block(input_block);
            f.render_widget(input_paragraph, left_chunks[1]);

            // 3. Render Active Files Context List
            let files_content: Vec<Line> = if active_files.is_empty() {
                vec![Line::from(Span::styled("No files added to context. Use /add <file> in REPL.", Style::default().fg(Color::DarkGray)))]
            } else {
                active_files
                    .iter()
                    .map(|file| Line::from(vec![
                        Span::styled(" * ", Style::default().fg(Color::Blue)),
                        Span::raw(file)
                    ]))
                    .collect()
            };

            let files_paragraph = Paragraph::new(files_content)
                .block(Block::default().borders(Borders::ALL).title(" Context Files "))
                .wrap(Wrap { trim: true });
            f.render_widget(files_paragraph, right_chunks[0]);

            // 4. Render System Logs & Memory Status
            let logs_content: Vec<Line> = app.system_logs
                .iter()
                .map(|log| Line::from(vec![
                    Span::styled(" > ", Style::default().fg(Color::LightRed)),
                    Span::raw(log)
                ]))
                .collect();

            let logs_paragraph = Paragraph::new(logs_content)
                .block(Block::default().borders(Borders::ALL).title(" System Logs & Learned Memory "))
                .wrap(Wrap { trim: true });
            f.render_widget(logs_paragraph, right_chunks[1]);

        }).map_err(|e| format!("Failed to draw TUI frame: {}", e))?;

        // Handle terminal key events
        if event::poll(std::time::Duration::from_millis(100)).map_err(|e| format!("Event poll error: {}", e))? {
            if let Event::Key(key) = event::read().map_err(|e| format!("Event read error: {}", e))? {
                if key.code == KeyCode::Esc {
                    // Clean quit TUI
                    break;
                }

                if app.input_mode {
                    match key.code {
                        KeyCode::Enter => {
                            let input_text = app.input.drain(..).collect::<String>();
                            let trimmed = input_text.trim();
                            if !trimmed.is_empty() {
                                app.chat_history.push((trimmed.to_string(), true));
                                app.system_logs.push(format!("Processing request: {}", trimmed));

                                // Handle TUI slash commands synchronously or mock
                                if trimmed.starts_with('/') {
                                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                                    let cmd = parts[0];
                                    match cmd {
                                        "/add" => {
                                            if parts.len() < 2 {
                                                app.chat_history.push(("Usage: /add <file_path>".to_string(), false));
                                            } else {
                                                let p = PathBuf::from(parts[1]);
                                                if !p.exists() {
                                                    app.chat_history.push((format!("Error: File not found: {}", parts[1]), false));
                                                } else {
                                                    if agent.context_manager.add_file(p) {
                                                        app.chat_history.push((format!("Added {} to context.", parts[1]), false));
                                                        app.system_logs.push(format!("Context added: {}", parts[1]));
                                                    }
                                                }
                                            }
                                        }
                                        "/drop" => {
                                            if parts.len() < 2 {
                                                app.chat_history.push(("Usage: /drop <file_path>".to_string(), false));
                                            } else {
                                                let p = PathBuf::from(parts[1]);
                                                if agent.context_manager.drop_file(&p) {
                                                    app.chat_history.push((format!("Removed {} from context.", parts[1]), false));
                                                    app.system_logs.push(format!("Context removed: {}", parts[1]));
                                                }
                                            }
                                        }
                                        "/clear" => {
                                            agent.context_manager.clear_chat();
                                            app.chat_history.truncate(2);
                                            app.system_logs.push("Context chat cleared.".to_string());
                                        }
                                        _ => {
                                            app.chat_history.push((format!("Unknown command: {}", cmd), false));
                                        }
                                    }
                                } else {
                                    // Run synchronously for simple interaction, or let the user know to use REPL for full deep streaming loop.
                                    app.chat_history.push(("Agent TUI is in monitor mode. Please run in standard REPL mode (default) to execute deep streaming tool loops.".to_string(), false));
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode().map_err(|e| format!("Failed to disable raw mode: {}", e))?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .map_err(|e| format!("Failed to leave alternate screen: {}", e))?;
    terminal.show_cursor().map_err(|e| format!("Failed to show cursor: {}", e))?;

    println!("TUI alternate screen closed.");
    Ok(())
}
