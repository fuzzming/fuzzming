use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Source language of the target project.
/// Controls which CodeGeneratorPort implementation is selected at composition time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Language {
    Solidity,
    // Rust,   — future
    // Vyper,  — future
    // Move,   — future
}

/// Fuzzing framework to use.
/// Controls which ConfigWriterPort and TestRunnerPort implementations are selected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Fuzzer {
    Foundry,
    // Echidna,   — future
    // Medusa,    — future
    // CargoFuzz, — future
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub model: String,
    pub llm_key: String,
    pub language: Language,
    pub fuzzer: Fuzzer,
    /// Absolute path to the Foundry project root — all forge commands run here.
    pub workspace_root: PathBuf,
}
