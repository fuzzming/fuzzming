use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use console::{Color, Style};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::reporter::ports::outbound::OutputPort;
use crate::shared::responses::stage_event::{StageEvent, StageKind, StageStatus};

struct ProgressState {
    bars: HashMap<String, ProgressBar>,
    multi: MultiProgress,
    header_printed: bool,
}

pub struct TerminalOutput;

impl TerminalOutput {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TerminalOutput {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OutputPort for TerminalOutput {
    async fn write(&self, output: &str) -> Result<()> {
        println!("{}", output);
        Ok(())
    }

    async fn write_progress(&self, output: &str) -> Result<()> {
        println!("{}", output);
        Ok(())
    }

    async fn handle_stage_event(&self, event: StageEvent) -> Result<()> {
        let state = progress_state();
        let mut guard = state.lock().expect("progress lock");
        let key = stage_key(&event);

        match event.status {
            StageStatus::Started => {
                if !guard.header_printed {
                    print_progress_header(&guard.multi);
                    guard.header_printed = true;
                }
                let spinner = guard.multi.add(ProgressBar::new_spinner());
                spinner.set_style(
                    ProgressStyle::with_template("{msg}{spinner}")
                        .unwrap()
                        .tick_strings(&["", ".", "..", "..."]),
                );
                spinner.enable_steady_tick(Duration::from_millis(250));
                spinner.set_message(stage_message(&event, false));
                guard.bars.insert(key, spinner);
            }
            StageStatus::Finished => {
                if let Some(bar) = guard.bars.remove(&key) {
                    bar.finish_with_message(stage_message(&event, true));
                }
            }
            StageStatus::Failed => {
                if let Some(bar) = guard.bars.remove(&key) {
                    bar.abandon_with_message(stage_message(&event, true));
                }
            }
        }

        Ok(())
    }
}

fn progress_state() -> Arc<Mutex<ProgressState>> {
    static STATE: std::sync::OnceLock<Arc<Mutex<ProgressState>>> = std::sync::OnceLock::new();
    STATE
        .get_or_init(|| {
            Arc::new(Mutex::new(ProgressState {
                bars: HashMap::new(),
                multi: MultiProgress::new(),
                header_printed: false,
            }))
        })
        .clone()
}

fn stage_key(event: &StageEvent) -> String {
    let contract = event.contract_name.as_deref().unwrap_or("all");
    format!("{}:{}:{:?}", event.round, contract, event.stage)
}

fn stage_message(event: &StageEvent, finished: bool) -> String {
    let contract_style = Style::new().fg(Color::Color256(99)).bold();
    let stage_style = Style::new().fg(Color::Color256(75)).bold();
    let ok_style = Style::new().fg(Color::Green).bold();
    let failed_style = Style::new().fg(Color::Red).bold();
    let running_style = Style::new().fg(Color::Color256(245));

    let stage = match event.stage {
        StageKind::Llm => "LLM is thinking",
        StageKind::Executor => "Executor is running",
        StageKind::Fuzzer => "Fuzzer is running",
    };
    let contract = event.contract_name.as_deref().unwrap_or("all");
    let contract_cell = format!("{:<24}", contract);
    let stage_cell = format!("{:<18}", stage);

    let status = if finished {
        ok_style.apply_to("OK")
    } else {
        running_style.apply_to("RUNNING")
    };

    if matches!(event.status, StageStatus::Failed) {
        return format!(
            "{} | {} | {}",
            contract_style.apply_to(contract_cell),
            stage_style.apply_to(stage_cell),
            failed_style.apply_to("FAILED")
        );
    }

    format!(
        "{} | {} | {}",
        contract_style.apply_to(contract_cell),
        stage_style.apply_to(stage_cell),
        status
    )
}

fn print_progress_header(multi: &MultiProgress) {
    let header_style = Style::new().fg(Color::Color256(63)).bold();
    let muted = Style::new().fg(Color::Color256(245));
    let header = format!("{:<24} | {:<18} | STATUS", "CONTRACT", "STAGE");
    let divider = "----------------------------------------------";

    multi.println("").ok();
    multi
        .println(header_style.apply_to(header).to_string())
        .ok();
    multi.println(muted.apply_to(divider).to_string()).ok();
}
