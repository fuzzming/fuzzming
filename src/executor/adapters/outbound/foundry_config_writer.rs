use crate::executor::adapters::outbound::FileSystemWriter;
use crate::executor::ports::outbound::ConfigWriterPort;
use crate::shared::models::{FoundryConfig, FuzzerConfigArtifact};
use anyhow::Result;
use async_trait::async_trait;

pub struct FoundryConfigWriter;

#[async_trait]
impl ConfigWriterPort for FoundryConfigWriter {
    async fn write(&self, config: &FuzzerConfigArtifact, writer: &FileSystemWriter) -> Result<()> {
        match config {
            FuzzerConfigArtifact::Foundry(c) => write_foundry_config(c, writer).await,
        }
    }
}

const FOUNDRY_TOML_PATH: &str = "foundry.toml";
const COVERAGE_HEADER: &str = "[profile.coverage]";
const FUZZMING_HEADER: &str = "[profile.fuzzming]";

pub async fn write_foundry_config(config: &FoundryConfig, writer: &FileSystemWriter) -> Result<()> {
    let fuzzming_section = build_fuzzming_section(config);

    // Read existing foundry.toml directly so we preserve [profile.default] and other sections.
    // current_toml in the config struct is populated only in tests; in production it is always None.
    let existing_on_disk = tokio::fs::read_to_string(writer.base_path().join(FOUNDRY_TOML_PATH))
        .await
        .unwrap_or_default();
    let base = if !existing_on_disk.is_empty() {
        existing_on_disk.as_str()
    } else {
        config.current_toml.as_deref().unwrap_or("")
    };
    let needs_coverage = !base.contains(COVERAGE_HEADER);

    let mut toml = replace_or_append_section(base, FUZZMING_HEADER, &fuzzming_section);

    if needs_coverage {
        let coverage_section = build_coverage_section();
        toml = replace_or_append_section(&toml, COVERAGE_HEADER, &coverage_section);
    }

    writer.write_file(FOUNDRY_TOML_PATH, &toml).await
}

const MAX_RUNS: u32 = 1000;
const MAX_DEPTH: u32 = 500;

fn build_fuzzming_section(config: &FoundryConfig) -> String {
    [
        FUZZMING_HEADER.to_string(),
        String::new(),
        "[profile.fuzzming.invariant]".to_string(),
        format!("runs             = {}", config.runs.min(MAX_RUNS)),
        format!("depth            = {}", config.depth.min(MAX_DEPTH)),
        format!("seed             = \"{}\"", config.seed),
        format!("max_test_rejects = {}", config.max_test_rejects),
        format!("dictionary_weight = {}", config.dictionary_weight),
    ]
    .join("\n")
}

fn build_coverage_section() -> String {
    [
        COVERAGE_HEADER.to_string(),
        String::new(),
        "[profile.coverage.invariant]".to_string(),
        "runs  = 256".to_string(),
        "depth = 50".to_string(),
    ]
    .join("\n")
}

fn replace_or_append_section(toml: &str, header: &str, new_section: &str) -> String {
    let lines: Vec<&str> = toml.lines().collect();
    let start = lines.iter().position(|l| l.trim() == header);

    match start {
        None => {
            let base = toml.trim_end();
            if base.is_empty() {
                format!("{new_section}\n")
            } else {
                format!("{base}\n\n{new_section}\n")
            }
        }
        Some(start_idx) => {
            // Extract bare section name, e.g. "[profile.fuzzming]" → "profile.fuzzming".
            // Any header that starts with "[{section_name}." is a sub-table of this section
            // and must be included in the block being replaced — otherwise TOML ends up with
            // duplicate keys on subsequent writes.
            let section_name = header.trim_start_matches('[').trim_end_matches(']');
            let subsection_prefix = format!("[{section_name}.");
            let end_idx = lines[start_idx + 1..]
                .iter()
                .position(|l| {
                    let t = l.trim();
                    t.starts_with('[') && !t.starts_with("[[") && !t.starts_with(&subsection_prefix)
                })
                .map(|rel| start_idx + 1 + rel)
                .unwrap_or(lines.len());

            let before = lines[..start_idx].join("\n");
            let after = lines[end_idx..].join("\n");
            let before_trimmed = before.trim_end();
            let after_trimmed = after.trim_start();

            match (before_trimmed.is_empty(), after_trimmed.is_empty()) {
                (true, true) => format!("{new_section}\n"),
                (true, false) => format!("{new_section}\n\n{after_trimmed}\n"),
                (false, true) => format!("{before_trimmed}\n\n{new_section}\n"),
                (false, false) => {
                    format!("{before_trimmed}\n\n{new_section}\n\n{after_trimmed}\n")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_when_absent() {
        let toml = "[profile.default]\nsolver_timeout = 10000\n";
        let result = replace_or_append_section(
            toml,
            "[profile.fuzzming]",
            "[profile.fuzzming]\nruns = 1000",
        );
        assert!(result.contains("[profile.default]"));
        assert!(result.contains("[profile.fuzzming]"));
        assert!(result.contains("runs = 1000"));
    }

    #[test]
    fn replaces_existing_section() {
        let toml = "[profile.default]\nsolver_timeout = 10000\n\n[profile.fuzzming]\nruns = 500\n";
        let result = replace_or_append_section(
            toml,
            "[profile.fuzzming]",
            "[profile.fuzzming]\nruns = 1000",
        );
        assert!(result.contains("[profile.default]"));
        assert!(result.contains("runs = 1000"));
        assert!(!result.contains("runs = 500"));
    }

    #[test]
    fn preserves_section_after_replaced_one() {
        let toml = "[profile.fuzzming]\nruns = 500\n\n[profile.coverage]\ndepth = 50\n";
        let result = replace_or_append_section(
            toml,
            "[profile.fuzzming]",
            "[profile.fuzzming]\nruns = 1000",
        );
        assert!(result.contains("runs = 1000"));
        assert!(result.contains("[profile.coverage]"));
    }

    #[test]
    fn empty_base_produces_clean_output() {
        let result =
            replace_or_append_section("", "[profile.fuzzming]", "[profile.fuzzming]\nruns = 1000");
        assert_eq!(result, "[profile.fuzzming]\nruns = 1000\n");
    }
}
