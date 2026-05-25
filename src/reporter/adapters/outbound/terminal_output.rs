use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use console::{Color, Style};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::reporter::ports::outbound::OutputPort;
use crate::shared::responses::stage_event::{StageEvent, StageKind, StageStatus};

/// Messages shown during round 1 — three LLM calls: analysis → bodies → config.
const ROUND_ONE_MESSAGES: &[&str] = &[
    "Analysing security",
    "Analysing security",
    "Generating invariants",
    "Generating invariants",
    "Building test suite",
    "Building test suite",
    "Configuring fuzzer",
];

/// Messages shown during round N — single patch call.
const ROUND_N_MESSAGES: &[&str] = &[
    "Reviewing coverage gaps",
    "Reviewing coverage gaps",
    "Patching test suite",
    "Patching test suite",
    "Updating fuzzer config",
];

/// Per-contract spinner state.
struct ContractProgress {
    bar: ProgressBar,
    /// Set to `true` to stop the background verb-rotation task.
    cancel: Arc<AtomicBool>,
}

struct ProgressState {
    contracts: HashMap<String, ContractProgress>,
    fuzzer_bar: Option<ProgressBar>,
    multi: MultiProgress,
}

pub struct TerminalOutput {
    state: Arc<Mutex<ProgressState>>,
}

impl TerminalOutput {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ProgressState {
                contracts: HashMap::new(),
                fuzzer_bar: None,
                multi: MultiProgress::new(),
            })),
        }
    }
}

impl Default for TerminalOutput {
    fn default() -> Self {
        Self::new()
    }
}

// ── message helpers ───────────────────────────────────────────────────────────

fn contract_label(name: &str) -> String {
    // Truncate long names, then left-pad to a fixed width for alignment.
    let s = if name.len() > 20 { &name[..20] } else { name };
    format!("{:<22}", s)
}

fn spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{msg}{spinner}")
        .unwrap()
        .tick_strings(&["", ".", "..", "..."])
}

/// Plain style used when finishing a bar — no trailing spinner dots.
fn finish_style() -> ProgressStyle {
    ProgressStyle::with_template("{msg}").unwrap()
}

fn msg_active(label: &str, verb: &str) -> String {
    let diamond = Style::new().fg(Color::Color256(99)).bold();
    let name_st = Style::new().fg(Color::Color256(147)).bold();
    let verb_st = Style::new().fg(Color::Color256(75));
    format!(
        "  {}  {}  {}",
        diamond.apply_to("◆"),
        name_st.apply_to(label),
        verb_st.apply_to(verb),
    )
}

fn msg_writing(label: &str) -> String {
    let diamond = Style::new().fg(Color::Color256(99)).bold();
    let name_st = Style::new().fg(Color::Color256(147)).bold();
    let verb_st = Style::new().fg(Color::Color256(244));
    format!(
        "  {}  {}  {}",
        diamond.apply_to("◆"),
        name_st.apply_to(label),
        verb_st.apply_to("Writing files"),
    )
}

fn msg_done(label: &str, ok: bool) -> String {
    let (icon, icon_st, text_st, text) = if ok {
        (
            "✓",
            Style::new().fg(Color::Green).bold(),
            Style::new().fg(Color::Color256(245)),
            "Done",
        )
    } else {
        (
            "✗",
            Style::new().fg(Color::Red).bold(),
            Style::new().fg(Color::Red),
            "Failed",
        )
    };
    let name_st = Style::new().fg(Color::Color256(147)).bold();
    format!(
        "  {}  {}  {}",
        icon_st.apply_to(icon),
        name_st.apply_to(label),
        text_st.apply_to(text),
    )
}

fn msg_fuzzer_active() -> String {
    let bolt = Style::new().fg(Color::Color256(220)).bold();
    let txt = Style::new().fg(Color::Color256(75));
    format!("  {}  {}", bolt.apply_to("⚡"), txt.apply_to("Fuzzing all contracts"))
}

fn msg_fuzzer_done(ok: bool) -> String {
    if ok {
        let st = Style::new().fg(Color::Green).bold();
        format!("  {}  Fuzzer complete", st.apply_to("✓"))
    } else {
        let st = Style::new().fg(Color::Red).bold();
        format!("  {}  Fuzzer failed", st.apply_to("✗"))
    }
}

// ── OutputPort impl ───────────────────────────────────────────────────────────

#[async_trait]
impl OutputPort for TerminalOutput {
    async fn write(&self, output: &str) -> Result<()> {
        let state = self.state.clone();
        let guard = state.lock().expect("progress lock");
        // Use multi.println so spinners are not disturbed.
        guard.multi.println(output).ok();
        Ok(())
    }

    async fn write_progress(&self, output: &str) -> Result<()> {
        let state = self.state.clone();
        let guard = state.lock().expect("progress lock");
        guard.multi.println(output).ok();
        Ok(())
    }

    async fn handle_stage_event(&self, event: StageEvent) -> Result<()> {
        let state = self.state.clone();

        match (&event.stage, &event.status) {
            // ── LLM Started → new per-contract spinner + verb rotation ──────
            (StageKind::Llm, StageStatus::Started) => {
                let contract = match &event.contract_name {
                    Some(c) => c.clone(),
                    None => return Ok(()),
                };

                let multi = {
                    let g = state.lock().expect("progress lock");
                    g.multi.clone()
                };

                let spinner = multi.add(ProgressBar::new_spinner());
                spinner.set_style(spinner_style());
                // Slow tick: each dot frame lasts 600 ms → full cycle ≈ 2.4 s
                spinner.enable_steady_tick(Duration::from_millis(600));

                let label = contract_label(&contract);
                let messages: &'static [&'static str] = if event.round == 1 {
                    ROUND_ONE_MESSAGES
                } else {
                    ROUND_N_MESSAGES
                };
                spinner.set_message(msg_active(&label, messages[0]));

                // Background task: step through stage messages every 2.2 s
                let cancel = Arc::new(AtomicBool::new(false));
                let cancel_bg = cancel.clone();
                let bar_bg = spinner.clone();
                let label_bg = label.clone();
                tokio::spawn(async move {
                    let mut i = 0usize;
                    loop {
                        tokio::time::sleep(Duration::from_millis(2200)).await;
                        if cancel_bg.load(Ordering::Relaxed) {
                            break;
                        }
                        i += 1;
                        let msg = messages[i % messages.len()];
                        bar_bg.set_message(msg_active(&label_bg, msg));
                    }
                });

                let mut g = state.lock().expect("progress lock");
                g.contracts.insert(contract, ContractProgress { bar: spinner, cancel });
            }

            // ── LLM Done → stop verb rotation, show "Writing files" ─────────
            (StageKind::Llm, StageStatus::Finished) | (StageKind::Llm, StageStatus::Failed) => {
                let contract = match &event.contract_name {
                    Some(c) => c.clone(),
                    None => return Ok(()),
                };
                let mut g = state.lock().expect("progress lock");
                if let Some(cp) = g.contracts.get_mut(&contract) {
                    cp.cancel.store(true, Ordering::Relaxed);
                    let label = contract_label(&contract);
                    cp.bar.set_message(msg_writing(&label));
                }
            }

            // ── Executor Started → spinner already shows "Writing files" ────
            (StageKind::Executor, StageStatus::Started) => {}

            // ── Executor Done → finish the spinner ──────────────────────────
            (StageKind::Executor, StageStatus::Finished) => {
                let contract = match &event.contract_name {
                    Some(c) => c.clone(),
                    None => return Ok(()),
                };
                let mut g = state.lock().expect("progress lock");
                if let Some(cp) = g.contracts.remove(&contract) {
                    let label = contract_label(&contract);
                    cp.bar.set_style(finish_style());
                    cp.bar.finish_with_message(msg_done(&label, true));
                }
            }

            (StageKind::Executor, StageStatus::Failed) => {
                let contract = match &event.contract_name {
                    Some(c) => c.clone(),
                    None => return Ok(()),
                };
                let mut g = state.lock().expect("progress lock");
                if let Some(cp) = g.contracts.remove(&contract) {
                    let label = contract_label(&contract);
                    cp.bar.set_style(finish_style());
                    cp.bar.abandon_with_message(msg_done(&label, false));
                }
            }

            // ── Fuzzer Started → separator + single spinner at the bottom ────
            (StageKind::Fuzzer, StageStatus::Started) => {
                let multi = {
                    let g = state.lock().expect("progress lock");
                    g.multi.clone()
                };

                // Clear visual boundary between per-contract work and fuzzer phase.
                let sep_st = Style::new().fg(Color::Color256(240));
                let phase_st = Style::new().fg(Color::Color256(220)).bold();
                multi.println("").ok();
                multi
                    .println(
                        sep_st
                            .apply_to("  ────────────────────────────────────────")
                            .to_string(),
                    )
                    .ok();
                multi
                    .println(
                        format!(
                            "  {}  {}",
                            phase_st.apply_to("⚡"),
                            Style::new()
                                .fg(Color::Color256(75))
                                .bold()
                                .apply_to("All contracts ready — running Foundry fuzzer")
                        )
                    )
                    .ok();
                multi
                    .println(
                        sep_st
                            .apply_to("  ────────────────────────────────────────")
                            .to_string(),
                    )
                    .ok();

                let spinner = multi.add(ProgressBar::new_spinner());
                spinner.set_style(spinner_style());
                spinner.enable_steady_tick(Duration::from_millis(600));
                spinner.set_message(msg_fuzzer_active());

                let mut g = state.lock().expect("progress lock");
                g.fuzzer_bar = Some(spinner);
            }

            (StageKind::Fuzzer, StageStatus::Finished) => {
                let mut g = state.lock().expect("progress lock");
                if let Some(bar) = g.fuzzer_bar.take() {
                    bar.set_style(finish_style());
                    bar.finish_with_message(msg_fuzzer_done(true));
                }
                g.multi.println("").ok();
            }

            (StageKind::Fuzzer, StageStatus::Failed) => {
                let mut g = state.lock().expect("progress lock");
                if let Some(bar) = g.fuzzer_bar.take() {
                    bar.set_style(finish_style());
                    bar.abandon_with_message(msg_fuzzer_done(false));
                }
            }
        }

        Ok(())
    }
}
