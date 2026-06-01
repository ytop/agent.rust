use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Memory {
    pub learned_facts: Vec<String>,
    pub command_history: Vec<String>,
    pub preferences: HashMap<String, String>,
}

pub struct MemoryManager {
    file_path: PathBuf,
    memory: Memory,
}

fn get_home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("USERPROFILE").map(PathBuf::from))
        .ok()
}

impl MemoryManager {
    pub fn new() -> Self {
        let home = get_home_dir().unwrap_or_else(|| PathBuf::from("."));
        let dir_path = home.join(".config").join("agent-rust");
        let file_path = dir_path.join("memory.json");

        let mut manager = Self {
            file_path,
            memory: Memory::default(),
        };

        let _ = manager.load();
        manager
    }

    /// Load memory from the JSON file
    pub fn load(&mut self) -> Result<(), String> {
        if !self.file_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&self.file_path)
            .map_err(|e| format!("Failed to read memory file: {}", e))?;
        
        let memory: Memory = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse memory JSON: {}", e))?;
        
        self.memory = memory;
        Ok(())
    }

    /// Save current memory to the JSON file
    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let serialized = serde_json::to_string_pretty(&self.memory)
            .map_err(|e| format!("Failed to serialize memory JSON: {}", e))?;

        fs::write(&self.file_path, serialized)
            .map_err(|e| format!("Failed to write memory file: {}", e))?;

        Ok(())
    }

    pub fn get_facts(&self) -> &[String] {
        &self.memory.learned_facts
    }

    pub fn add_fact(&mut self, fact: String) -> Result<(), String> {
        if !self.memory.learned_facts.contains(&fact) {
            self.memory.learned_facts.push(fact);
            self.save()?;
        }
        Ok(())
    }

    pub fn remove_fact(&mut self, index: usize) -> Result<bool, String> {
        if index < self.memory.learned_facts.len() {
            self.memory.learned_facts.remove(index);
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn clear_facts(&mut self) -> Result<(), String> {
        self.memory.learned_facts.clear();
        self.save()
    }

    pub fn get_command_history(&self) -> &[String] {
        &self.memory.command_history
    }

    pub fn add_command(&mut self, command: String) -> Result<(), String> {
        // Keep last 100 commands to prevent bloat
        self.memory.command_history.push(command);
        if self.memory.command_history.len() > 100 {
            self.memory.command_history.remove(0);
        }
        self.save()
    }

    pub fn get_preference(&self, key: &str) -> Option<&String> {
        self.memory.preferences.get(key)
    }

    pub fn set_preference(&mut self, key: String, value: String) -> Result<(), String> {
        self.memory.preferences.insert(key, value);
        self.save()
    }
}
