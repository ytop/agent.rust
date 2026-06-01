use std::fs;
use std::path::{Path, PathBuf};
use std::future::Future;
use std::pin::Pin;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::tools::Tool;

// Helper to check if a directory/file should be ignored
fn is_ignored(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    name.starts_with('.') && name != ".env" 
        || name == "target" 
        || name == "node_modules"
}

// Helper to collect all files in a directory recursively
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if is_ignored(dir) {
        return Ok(());
    }

    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_files(&path, files)?;
            } else if !is_ignored(&path) {
                files.push(path);
            }
        }
    }
    Ok(())
}

// 1. VIEW FILE TOOL
pub struct ViewFileTool;

#[derive(Deserialize)]
struct ViewFileArgs {
    path: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
}

impl Tool for ViewFileTool {
    fn name(&self) -> &str {
        "view_file"
    }

    fn description(&self) -> &str {
        "Read absolute or relative files, supporting pagination/line range limits."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the file to view." },
                "start_line": { "type": "integer", "description": "Optional 1-indexed start line number to view (inclusive)." },
                "end_line": { "type": "integer", "description": "Optional 1-indexed end line number to view (inclusive)." }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, arguments: &str) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> {
        let arguments = arguments.to_string();
        Box::pin(async move {
            let args: ViewFileArgs = serde_json::from_str(&arguments)
                .map_err(|e| format!("Failed to parse arguments: {}", e))?;

            let file_path = PathBuf::from(&args.path);
            if !file_path.exists() {
                return Err(format!("File does not exist: {}", args.path));
            }

            let content = fs::read_to_string(&file_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();

            let start = args.start_line.unwrap_or(1);
            let end = args.end_line.unwrap_or(total_lines);

            if start == 0 || start > total_lines {
                return Err(format!("Invalid start_line: {}. Total lines: {}", start, total_lines));
            }

            let end = std::cmp::min(end, total_lines);
            if end < start {
                return Err(format!("end_line ({}) cannot be less than start_line ({})", end, start));
            }

            let mut output = String::new();
            output.push_str(&format!("File: {} (Lines {} - {} of {})\n\n", args.path, start, end, total_lines));
            
            for idx in (start - 1)..end {
                output.push_str(&format!("{:5}: {}\n", idx + 1, lines[idx]));
            }

            Ok(output)
        })
    }
}

// 2. WRITE FILE TOOL
pub struct WriteFileTool;

#[derive(Deserialize)]
struct WriteFileArgs {
    path: String,
    content: String,
}

impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write full content safely, creating subdirectories if needed. Automatically backups existing files."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to write the file to." },
                "content": { "type": "string", "description": "The exact and full content of the file." }
            },
            "required": ["path", "content"]
        })
    }

    fn execute(&self, arguments: &str) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> {
        let arguments = arguments.to_string();
        Box::pin(async move {
            let args: WriteFileArgs = serde_json::from_str(&arguments)
                .map_err(|e| format!("Failed to parse arguments: {}", e))?;

            let file_path = PathBuf::from(&args.path);

            // Create parent directories if missing
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent directories: {}", e))?;
            }

            // Backup existing file
            if file_path.exists() {
                let mut backup_path = file_path.clone();
                backup_path.set_extension("bak");
                fs::copy(&file_path, &backup_path)
                    .map_err(|e| format!("Failed to create backup at {}: {}", backup_path.to_string_lossy(), e))?;
            }

            // Write the file
            fs::write(&file_path, &args.content)
                .map_err(|e| format!("Failed to write file: {}", e))?;

            Ok(format!("Successfully wrote file: {}", args.path))
        })
    }
}

// 3. PATCH FILE TOOL
pub struct PatchFileTool;

#[derive(Deserialize)]
struct PatchFileArgs {
    path: String,
    target: String,
    replacement: String,
}

impl Tool for PatchFileTool {
    fn name(&self) -> &str {
        "patch_file"
    }

    fn description(&self) -> &str {
        "Replace an exact target block of text inside a file with a replacement. Backs up the file first."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the file to edit." },
                "target": { "type": "string", "description": "The exact block of code/text to search for and replace." },
                "replacement": { "type": "string", "description": "The new block of code/text to replace the target block." }
            },
            "required": ["path", "target", "replacement"]
        })
    }

    fn execute(&self, arguments: &str) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> {
        let arguments = arguments.to_string();
        Box::pin(async move {
            let args: PatchFileArgs = serde_json::from_str(&arguments)
                .map_err(|e| format!("Failed to parse arguments: {}", e))?;

            let file_path = PathBuf::from(&args.path);
            if !file_path.exists() {
                return Err(format!("File does not exist: {}", args.path));
            }

            let content = fs::read_to_string(&file_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            // Find occurrences of the target
            let occurrences: Vec<_> = content.match_indices(&args.target).collect();
            if occurrences.is_empty() {
                return Err("The target content was not found exactly as specified. Please check spacing, indentation, and newlines.".to_string());
            }
            if occurrences.len() > 1 {
                return Err(format!(
                    "The target content matches {} occurrences in the file. To prevent incorrect edits, please narrow down the target block.",
                    occurrences.len()
                ));
            }

            // Perform the patch
            let patched_content = content.replacen(&args.target, &args.replacement, 1);

            // Backup existing file
            let mut backup_path = file_path.clone();
            backup_path.set_extension("bak");
            fs::copy(&file_path, &backup_path)
                .map_err(|e| format!("Failed to create backup at {}: {}", backup_path.to_string_lossy(), e))?;

            // Write new content
            fs::write(&file_path, patched_content)
                .map_err(|e| format!("Failed to write patched file: {}", e))?;

            Ok(format!("Successfully patched file: {}", args.path))
        })
    }
}

// 4. LIST DIRECTORY TOOL
pub struct ListDirectoryTool;

#[derive(Deserialize)]
struct ListDirectoryArgs {
    path: Option<String>,
}

impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List files in a directory, displaying file sizes and directory states."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Optional path to list. Defaults to the current directory '.'." }
            }
        })
    }

    fn execute(&self, arguments: &str) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> {
        let arguments = arguments.to_string();
        Box::pin(async move {
            let args: ListDirectoryArgs = serde_json::from_str(&arguments)
                .map_err(|e| format!("Failed to parse arguments: {}", e))?;

            let target_path = PathBuf::from(args.path.unwrap_or_else(|| ".".to_string()));
            if !target_path.exists() {
                return Err(format!("Directory does not exist: {}", target_path.to_string_lossy()));
            }
            if !target_path.is_dir() {
                return Err(format!("Path is not a directory: {}", target_path.to_string_lossy()));
            }

            let mut output = String::new();
            output.push_str(&format!("Directory list for: {}\n\n", target_path.to_string_lossy()));

            let entries = fs::read_dir(&target_path)
                .map_err(|e| format!("Failed to read directory: {}", e))?;

            let mut dir_entries = Vec::new();
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if !is_ignored(&path) {
                        dir_entries.push(path);
                    }
                }
            }

            // Sort entries: directories first, then files alphabetically
            dir_entries.sort_by(|a, b| {
                let a_is_dir = a.is_dir();
                let b_is_dir = b.is_dir();
                if a_is_dir != b_is_dir {
                    b_is_dir.cmp(&a_is_dir)
                } else {
                    a.file_name().cmp(&b.file_name())
                }
            });

            for path in dir_entries {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if path.is_dir() {
                    output.push_str(&format!("[DIR]  {}/\n", name));
                } else {
                    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    output.push_str(&format!("{:5}  {} ({} bytes)\n", "[FILE]", name, size));
                }
            }

            Ok(output)
        })
    }
}

// 5. GREP SEARCH TOOL
pub struct GrepSearchTool;

#[derive(Deserialize)]
struct GrepSearchArgs {
    query: String,
    path: Option<String>,
}

impl Tool for GrepSearchTool {
    fn name(&self) -> &str {
        "grep_search"
    }

    fn description(&self) -> &str {
        "Search for text matches recursively across files in a directory."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "The exact text query to search for." },
                "path": { "type": "string", "description": "Optional directory path to search. Defaults to current directory '.'." }
            },
            "required": ["query"]
        })
    }

    fn execute(&self, arguments: &str) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> {
        let arguments = arguments.to_string();
        Box::pin(async move {
            let args: GrepSearchArgs = serde_json::from_str(&arguments)
                .map_err(|e| format!("Failed to parse arguments: {}", e))?;

            let search_dir = PathBuf::from(args.path.unwrap_or_else(|| ".".to_string()));
            if !search_dir.exists() {
                return Err(format!("Directory does not exist: {}", search_dir.to_string_lossy()));
            }

            let mut files = Vec::new();
            collect_files(&search_dir, &mut files)
                .map_err(|e| format!("Failed to read files recursively: {}", e))?;

            let mut output = String::new();
            output.push_str(&format!("Search results for '{}' in {}:\n\n", args.query, search_dir.to_string_lossy()));

            let mut match_count = 0;
            
            for file in files {
                let content = match fs::read_to_string(&file) {
                    Ok(c) => c,
                    Err(_) => continue, // Skip binary or unreadable files
                };

                let relative_path = file.to_string_lossy().to_string();

                for (idx, line) in content.lines().enumerate() {
                    if line.contains(&args.query) {
                        match_count += 1;
                        output.push_str(&format!("{}:{}: {}\n", relative_path, idx + 1, line.trim()));
                        
                        if match_count >= 100 {
                            output.push_str("\n[Truncated: over 100 matches found]");
                            return Ok(output);
                        }
                    }
                }
            }

            if match_count == 0 {
                output.push_str("No matches found.");
            } else {
                output.push_str(&format!("\nTotal matches found: {}", match_count));
            }

            Ok(output)
        })
    }
}
