# Implementation Plan: `agent-rust`

This document details the step-by-step implementation tasks required to build the `agent-rust` code agent.

---

## Phase 1: Project Setup and API Client
- [ ] **Task 1.1: Initialize Cargo Project**
  - Create standard binary crate `cargo init --bin`.
  - Add core dependencies to `Cargo.toml`:
    - `tokio` (async runtime)
    - `reqwest` (HTTP client with native-tls/rustls, json, stream support)
    - `serde` and `serde_json` (de/serialization)
    - `crossterm` and `ratatui` (TUI and terminal manipulation)
    - `rustyline` or `dialoguer` (for interactive shell prompt)
    - `futures` / `tokio-stream` (for processing streamed LLM chunks)
    - `clap` (for parsing command line arguments)
- [ ] **Task 1.2: DeepSeek API Client Module**
  - Design the `Client` struct in `src/deepseek/client.rs`.
  - Implement request and response schemas representing message history, roles (`system`, `user`, `assistant`, `tool`), functions, and tool calls.
  - Implement async streaming for chat completions (`chat_completions_stream`).
  - Add test harness to verify API payload serialization.
- [ ] **Task 1.3: Configuration and Environment Loaders**
  - Create `src/config.rs` to load `DEEPSEEK_API_KEY` from environment or configuration file.
  - Parse CLI parameters using `clap` (e.g. `--tui`, `--model`, `--max-tokens`).

---

## Phase 2: Context and Memory Management
- [ ] **Task 2.1: Context Manager Implementation**
  - Design `src/context/manager.rs`.
  - Implement adding/dropping files from active context.
  - Write standard file reader that formats active files into XML or Markdown blocks for the system prompt.
  - Build simple token estimator (char-based heuristic or basic BPE) to track remaining context budget.
  - Implement auto-truncation logic for conversation history, preserving the system prompt, files context, and latest messages.
- [ ] **Task 2.2: Memory Manager Implementation**
  - Design `src/memory/manager.rs` and the persistent schema.
  - Read/write `~/.config/agent-rust/memory.json` safely.
  - Implement functions to add, retrieve, and query learned facts.
  - Implement command history logger.

---

## Phase 3: Tool Execution Engine
- [ ] **Task 3.1: Tool Definitions & Traits**
  - Define a standard `Tool` trait in `src/tools/mod.rs` containing `name`, `description`, `parameters_schema`, and `execute`.
- [ ] **Task 3.2: Implement File Tools**
  - `view_file`: Read absolute/relative files, supporting pagination/line range limits.
  - `write_file`: Write full content safely (creating subdirectories if needed) with backup file rotation (`.bak`).
  - `patch_file`: Replace exact target strings with replacements, or implement diff patches.
  - `list_directory`: Read directories recursively.
  - `grep_search`: Implement rapid regex searching across files.
- [ ] **Task 3.3: Implement Command Executor Tool**
  - `run_command`: Run commands using `tokio::process::Command`.
  - Stream command output (`stdout` / `stderr`) in real time to the screen.
  - Implement timeout handling and exit code tracking.

---

## Phase 4: Core Agent Engine & Control Loop
- [ ] **Task 4.1: Engine Loop Orchestration**
  - Implement `src/engine/loop.rs`.
  - Define the main asynchronous loop coordinating user input -> context injection -> API call -> response parse -> tool execution -> feedback loop.
- [ ] **Task 4.2: Tool Call Handler**
  - Parse multiple parallel tool calls from the DeepSeek response.
  - Create confirmation mechanics to ask users before executing high-risk tools like `run_command` or destructive file writes.
  - Collect tool results and queue them back into the message history for subsequent LLM rounds.

---

## Phase 5: User Interface (Interactive REPL and TUI)
- [ ] **Task 5.1: Interactive REPL UI**
  - Build styled REPL using `rustyline` with history, syntax highlighting, and slash-command completion.
  - Format markdown stream outputs in real-time.
  - Implement interactive approval boxes for command execution.
- [ ] **Task 5.2: Ratatui TUI Interface**
  - Build split-screen layout using `ratatui`.
  - Pane 1: Conversation history (scrollable with styled text).
  - Pane 2: Current context files list & token usage gauges.
  - Pane 3: Interactive input field.
  - Pane 4: Tool output / compilation logs.
  - Set up keyboard event handling (`crossterm`) for navigation and input modes.

---

## Phase 6: System Integration, Testing & Polish
- [ ] **Task 6.1: End-to-End Simulation**
  - Verify complete workflow (User asks to write a file -> tool writes it -> user confirms -> cargo test runs -> compiler error feedback -> agent patches file).
- [ ] **Task 6.2: Error Handling & Resilience**
  - Catch connection errors, API timeouts, and missing keys.
  - Gracefully recover from failed commands without crashing the session.
- [ ] **Task 6.3: Codebase Documentation & Distribution**
  - Add help commands, configure default configurations, and test on multiple codebases.
