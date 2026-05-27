use std::path::Path;

use crate::shared::models::CoverageContext;

/// Populate source_context for each coverage gap from nearby source lines (best-effort).
pub async fn enrich_coverage_context(coverage: &mut CoverageContext, workspace_root: &Path) {
    for gap in coverage.gaps.iter_mut() {
        if gap.file.is_empty() {
            continue;
        }

        let path = if Path::new(&gap.file).is_absolute() {
            Path::new(&gap.file).to_path_buf()
        } else {
            workspace_root.join(&gap.file)
        };

        let Ok(source) = tokio::fs::read_to_string(&path).await else {
            continue;
        };

        let lines: Vec<&str> = source.lines().collect();
        if lines.is_empty() {
            continue;
        }

        let idx = (gap.line as isize - 1).max(0) as usize;
        let start = idx.saturating_sub(1);
        let end = std::cmp::min(idx + 1, lines.len().saturating_sub(1));

        gap.source_context = lines
            .iter()
            .enumerate()
            .skip(start)
            .take(end - start + 1)
            .map(|(i, line)| format!("{}: {}", i + 1, line))
            .collect();
    }
}
