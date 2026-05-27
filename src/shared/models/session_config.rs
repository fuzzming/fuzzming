use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Source language of the target project.
/// Controls which CodeGeneratorPort implementation is selected at composition time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Language {
    Solidity,
    // Rust,   // Reserved for future support.
    // Vyper,  // Reserved for future support.
    // Move,   // Reserved for future support.
}

/// Fuzzing framework to use.
/// Controls which ConfigWriterPort and TestRunnerPort implementations are selected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Fuzzer {
    Foundry,
    // Echidna,   // Reserved for future support.
    // Medusa,    // Reserved for future support.
    // CargoFuzz, // Reserved for future support.
}

/// Selects how much guidance the prompt includes.
/// `Concise`: 9 focused rules — for capable models (Claude, GPT-4+, Gemini).
/// `Guided`: 18 explicit rules — for open-source models that need more direction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum PromptMode {
    #[default]
    Concise,
    Guided,
}

impl PromptMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            PromptMode::Concise => "concise",
            PromptMode::Guided => "guided",
        }
    }
}

impl std::str::FromStr for PromptMode {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "concise" => Ok(PromptMode::Concise),
            "guided" => Ok(PromptMode::Guided),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub model: String,
    pub llm_key: String,
    pub language: Language,
    pub fuzzer: Fuzzer,
    /// Absolute path to the Foundry project root — all forge commands run here.
    pub workspace_root: PathBuf,
    /// Maximum tokens the LLM may generate per call. None means no restriction.
    pub max_tokens: Option<u32>,
    /// Per-call LLM timeout in seconds.
    pub llm_timeout_secs: u64,
    /// Stop a contract session after this many consecutive rounds with 100% coverage.
    pub full_coverage_rounds: u32,
    /// Controls prompt rule verbosity — set explicitly in fuzzming.config.
    pub prompt_mode: PromptMode,
}
