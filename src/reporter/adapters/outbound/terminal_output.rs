use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::Notify;

use anyhow::Result;
use async_trait::async_trait;
use console::{Color, Style};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::reporter::ports::outbound::OutputPort;
use crate::shared::responses::stage_event::{FuzzerRoundSummary, StageEvent, StageKind, StageStatus};

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

/// Messages cycled on the fuzzer spinner while forge is running.
const FUZZER_MESSAGES: &[&str] = &[
    "Running invariant tests",
    "Running invariant tests",
    "Scanning for vulnerabilities",
    "Scanning for vulnerabilities",
    "Analysing call sequences",
    "Analysing call sequences",
    "Exploring edge cases",
];

struct ContractProgress {
    bar: ProgressBar,
    cancel: Arc<Notify>,
}

struct FuzzerProgress {
    bar: ProgressBar,
    cancel: Arc<Notify>,
}

struct ProgressState {
    contracts: HashMap<String, ContractProgress>,
    fuzzer: Option<FuzzerProgress>,
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
                fuzzer: None,
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

fn contract_label(name: &str) -> String {
    let s = if name.len() > 20 { &name[..20] } else { name };
    format!("{s:<22}")
}

fn spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{msg}{spinner}")
        .unwrap()
        .tick_strings(&["", ".", "..", "..."])
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

fn msg_fuzzer_active(verb: &str) -> String {
    let bolt = Style::new().fg(Color::Color256(220)).bold();
    let txt = Style::new().fg(Color::Color256(75));
    format!("  {}  {}", bolt.apply_to("⚡"), txt.apply_to(verb))
}

fn msg_fuzzer_done(ok: bool, summary: Option<&FuzzerRoundSummary>) -> String {
    let (icon, icon_st) = if ok {
        ("✓", Style::new().fg(Color::Green).bold())
    } else {
        ("✗", Style::new().fg(Color::Red).bold())
    };
    let label = if ok { "Fuzzer complete" } else { "Fuzzer failed" };
    let base = format!("  {}  {}", icon_st.apply_to(icon), label);
    match summary {
        Some(s) => {
            let muted = Style::new().fg(Color::Color256(245));
            let bug_st = if s.bugs > 0 {
                Style::new().fg(Color::Red)
            } else {
                Style::new().fg(Color::Green)
            };
            let compile_part = if s.compile_errors > 0 {
                let err_st = Style::new().fg(Color::Color256(208));
                format!(
                    "  {}",
                    err_st.apply_to(format!(
                        "{} compile error{}",
                        s.compile_errors,
                        if s.compile_errors == 1 { "" } else { "s" }
                    ))
                )
            } else {
                String::new()
            };
            format!(
                "{}  {}  {}{}",
                base,
                bug_st.apply_to(format!("{} bug{}", s.bugs, if s.bugs == 1 { "" } else { "s" })),
                muted.apply_to(format!("{} passed", s.passed)),
                compile_part,
            )
        }
        None => base,
    }
}

#[async_trait]
impl OutputPort for TerminalOutput {
    async fn write(&self, output: &str) -> Result<()> {
        let state = self.state.clone();
        let guard = state.lock().expect("progress lock");
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
                spinner.enable_steady_tick(Duration::from_millis(600));

                let label = contract_label(&contract);
                let messages: &'static [&'static str] = if event.round == 1 {
                    ROUND_ONE_MESSAGES
                } else {
                    ROUND_N_MESSAGES
                };
                spinner.set_message(msg_active(&label, messages[0]));

                let cancel = Arc::new(Notify::new());
                let cancel_bg = cancel.clone();
                let bar_bg = spinner.clone();
                let label_bg = label.clone();
                tokio::spawn(async move {
                    let mut i = 0usize;
                    loop {
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_millis(2200)) => {
                                i += 1;
                                bar_bg.set_message(msg_active(&label_bg, messages[i % messages.len()]));
                            }
                            _ = cancel_bg.notified() => break,
                        }
                    }
                });

                let mut g = state.lock().expect("progress lock");
                g.contracts.insert(contract, ContractProgress { bar: spinner, cancel });
            }

            (StageKind::Llm, StageStatus::Finished) | (StageKind::Llm, StageStatus::Failed) => {
                let contract = match &event.contract_name {
                    Some(c) => c.clone(),
                    None => return Ok(()),
                };
                let mut g = state.lock().expect("progress lock");
                if let Some(cp) = g.contracts.get_mut(&contract) {
                    cp.cancel.notify_one();
                    let label = contract_label(&contract);
                    cp.bar.set_message(msg_writing(&label));
                }
            }

            (StageKind::Executor, StageStatus::Started) => {}

            (StageKind::Executor, StageStatus::Finished) => {
                let contract = match &event.contract_name {
                    Some(c) => c.clone(),
                    None => return Ok(()),
                };
                let mut g = state.lock().expect("progress lock");
                if let Some(cp) = g.contracts.remove(&contract) {
                    let label = contract_label(&contract);
                    let msg = msg_done(&label, true);
                    cp.bar.finish_and_clear();
                    g.multi.println(msg).ok();
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
                    let msg = msg_done(&label, false);
                    cp.bar.finish_and_clear();
                    g.multi.println(msg).ok();
                }
            }

            (StageKind::Fuzzer, StageStatus::Started) => {
                let multi = {
                    let g = state.lock().expect("progress lock");
                    g.multi.clone()
                };

                multi.println("").ok();

                let spinner = multi.add(ProgressBar::new_spinner());
                spinner.set_style(spinner_style());
                spinner.enable_steady_tick(Duration::from_millis(600));
                spinner.set_message(msg_fuzzer_active(FUZZER_MESSAGES[0]));

                let cancel = Arc::new(Notify::new());
                let cancel_bg = cancel.clone();
                let bar_bg = spinner.clone();
                tokio::spawn(async move {
                    let mut i = 0usize;
                    loop {
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_millis(2200)) => {
                                i += 1;
                                bar_bg.set_message(msg_fuzzer_active(FUZZER_MESSAGES[i % FUZZER_MESSAGES.len()]));
                            }
                            _ = cancel_bg.notified() => break,
                        }
                    }
                });

                let mut g = state.lock().expect("progress lock");
                g.fuzzer = Some(FuzzerProgress { bar: spinner, cancel });
            }

            (StageKind::Fuzzer, StageStatus::Finished) => {
                let summary = event.fuzzer_summary.as_ref();
                let msg = msg_fuzzer_done(true, summary);
                let mut g = state.lock().expect("progress lock");
                if let Some(fp) = g.fuzzer.take() {
                    fp.cancel.notify_one();
                    fp.bar.finish_and_clear();
                    g.multi.println(msg).ok();
                }
                g.multi.println("").ok();
            }

            (StageKind::Fuzzer, StageStatus::Failed) => {
                let summary = event.fuzzer_summary.as_ref();
                let msg = msg_fuzzer_done(false, summary);
                let mut g = state.lock().expect("progress lock");
                if let Some(fp) = g.fuzzer.take() {
                    fp.cancel.notify_one();
                    fp.bar.finish_and_clear();
                    g.multi.println(msg).ok();
                }
                g.multi.println("").ok();
            }

            (StageKind::ContractDone, _) | (StageKind::SessionDone, _) => {}
        }

        Ok(())
    }
}
