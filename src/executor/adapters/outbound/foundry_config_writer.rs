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

    let base = config.current_toml.as_deref().unwrap_or("");
    let needs_coverage = !base.contains(COVERAGE_HEADER);

    let mut toml = replace_or_append_section(base, FUZZMING_HEADER, &fuzzming_section);

    if needs_coverage {
        let coverage_section = build_coverage_section();
        toml = replace_or_append_section(&toml, COVERAGE_HEADER, &coverage_section);
    }

    writer.write_file(FOUNDRY_TOML_PATH, &toml).await
}

fn build_fuzzming_section(config: &FoundryConfig) -> String {
    let mut lines = vec![
        FUZZMING_HEADER.to_string(),
        format!("depth            = {}", config.depth),
        format!("runs             = {}", config.runs),
        format!("seed             = \"{}\"", config.seed),
        format!("max_test_rejects = {}", config.max_test_rejects),
        format!("dictionary_weight = {}", config.dictionary_weight),
    ];

    if !config.call_sequence_weights.is_empty() {
        lines.push(String::new());
        lines.push("[profile.fuzzming.invariant]".to_string());
        for (selector, weight) in &config.call_sequence_weights {
            lines.push(format!("\"{}\" = {}", selector, weight));
        }
    }

    lines.join("\n")
}

fn build_coverage_section() -> String {
    vec![
        COVERAGE_HEADER.to_string(),
        "depth = 50".to_string(),
        "runs  = 256".to_string(),
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
                format!("{}\n", new_section)
            } else {
                format!("{}\n\n{}\n", base, new_section)
            }
        }
        Some(start_idx) => {
            let end_idx = lines[start_idx + 1..]
                .iter()
                .position(|l| {
                    let t = l.trim();
                    t.starts_with('[') && !t.starts_with("[[")
                })
                .map(|rel| start_idx + 1 + rel)
                .unwrap_or(lines.len());

            let before = lines[..start_idx].join("\n");
            let after = lines[end_idx..].join("\n");
            let before_trimmed = before.trim_end();
            let after_trimmed = after.trim_start();

            match (before_trimmed.is_empty(), after_trimmed.is_empty()) {
                (true, true) => format!("{}\n", new_section),
                (true, false) => format!("{}\n\n{}\n", new_section, after_trimmed),
                (false, true) => format!("{}\n\n{}\n", before_trimmed, new_section),
                (false, false) => {
                    format!(
                        "{}\n\n{}\n\n{}\n",
                        before_trimmed, new_section, after_trimmed
                    )
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
