use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use indexmap::IndexMap;
use tokio::time::sleep;

use crate::generator::domain::generation_response::{GenerationResponse, GenerationResult};
use crate::orchestrator::adapters::inbound::Orchestrator;
use crate::orchestrator::use_cases::RunSessionUseCase;
use crate::reporter::adapters::inbound::Reporter;
use crate::reporter::adapters::outbound::TerminalOutput;
use crate::shared::models::{
    BodiesJson, BodiesMeta, BugInfo, ContractContext, CoverageContext, ExecutorInput,
    FoundryConfig, FuzzerConfigArtifact, GenerationUsage, HandlerBodies, InvariantTestBodies,
};
use crate::shared::ports::{
    ExecutorPort, FuzzerEnginePort, LlmEnginePort, OrchestratorPort, ReaderPort,
};
use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::{
    fuzz_report::{FuzzOutcome, FuzzReport},
    llm_signal::{LlmSignal, LlmStatus},
};

/// Mock LLM engine used by the demo composition.
pub struct MockLlmEngine;

#[async_trait]
impl LlmEnginePort for MockLlmEngine {
    async fn run(&self, signal: RoundSignal) -> Result<LlmSignal> {
        sleep(Duration::from_millis(1400)).await;
        Ok(LlmSignal {
            status: LlmStatus::Done,
            result: Some(GenerationResult {
                response: GenerationResponse::Full {
                    bodies: fake_bodies(&signal.contract_name, &signal.contract_path),
                    foundry_config: fake_foundry_config(),
                },
                usage: GenerationUsage {
                    calls: 1,
                    prompt_tokens: 4200,
                    completion_tokens: 1800,
                    total_tokens: 6000,
                    ..Default::default()
                },
            }),
            reason: None,
        })
    }
}

/// Mock executor used by the demo composition.
pub struct MockExecutor;

#[async_trait]
impl ExecutorPort for MockExecutor {
    async fn execute(&self, _input: ExecutorInput) -> Result<()> {
        sleep(Duration::from_millis(200)).await;
        Ok(())
    }
}

/// Mock fuzzer engine used by the demo composition.
pub struct MockFuzzerEngine;

#[async_trait]
impl FuzzerEnginePort for MockFuzzerEngine {
    async fn run(&self, signals: Vec<RoundSignal>) -> Result<Vec<FuzzReport>> {
        sleep(Duration::from_millis(1800)).await;
        Ok(signals
            .iter()
            .map(|s| scripted_report(&s.contract_name, s.round))
            .collect())
    }
}

fn scripted_report(contract_name: &str, round: u32) -> FuzzReport {
    match (contract_name, round) {
        ("TokenVault", 1) => bug_report(vec![BugInfo {
            invariant_name: "invariant_solvency".to_string(),
            call_sequence:
                "sender=0x1111...  calldata=withdraw(uint256) args=[1000000000000000000 [1e18]]"
                    .to_string(),
        }]),
        ("TokenVault", 2) => bug_report(vec![BugInfo {
            invariant_name: "invariant_noReentrancy".to_string(),
            call_sequence: concat!(
                "sender=0x2222...  calldata=deposit(uint256) args=[500000000000000000 [5e17]]\n",
                "sender=0x2222...  calldata=withdraw(uint256) args=[500000000000000000 [5e17]]"
            )
            .to_string(),
        }]),
        ("TokenVault", _) => bug_report(vec![BugInfo {
            invariant_name: "invariant_solvency".to_string(),
            call_sequence:
                "sender=0x3333...  calldata=ownerWithdraw(uint256) args=[9999999999 [9.999e9]]"
                    .to_string(),
        }]),
        ("StakingPool", 1) => pass_report("StakingPool"),
        ("StakingPool", _) => bug_report(vec![BugInfo {
            invariant_name: "invariant_rewardRateAccessControl".to_string(),
            call_sequence: "sender=0x4444...  calldata=setRewardRate(uint256) args=[99999]"
                .to_string(),
        }]),
        ("PriceOracle", _) => pass_report("PriceOracle"),
        _ => pass_report(contract_name),
    }
}

fn bug_report(bugs: Vec<BugInfo>) -> FuzzReport {
    FuzzReport {
        outcome: FuzzOutcome::Bug,
        bugs,
        lcov_path: None,
    }
}

fn pass_report(contract_name: &str) -> FuzzReport {
    FuzzReport {
        outcome: FuzzOutcome::Pass,
        bugs: vec![],
        lcov_path: Some(PathBuf::from(format!(
            ".fuzzming/{}/lcov.info",
            contract_name
        ))),
    }
}

/// Mock reader used by the demo composition.
pub struct MockReader;

#[async_trait]
impl ReaderPort for MockReader {
    async fn get_contract_context(
        &self,
        path: &str,
        _include_comments: bool,
    ) -> Result<ContractContext> {
        let name = std::path::Path::new(path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        Ok(ContractContext {
            source_code: format!(
                "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.0;\n\ncontract {} {{\n    // demo\n}}\n",
                name
            ),
        })
    }

    async fn get_fuzz_output(&self, _path: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn get_coverage_context(&self, _path: &str) -> Result<Option<CoverageContext>> {
        Ok(Some(CoverageContext {
            line_found: 80,
            line_hit: 80,
            branch_found: 20,
            branch_hit: 20,
            function_found: 10,
            function_hit: 10,
            gaps: vec![],
        }))
    }

    async fn get_existing_bodies(&self, _path: &str) -> Result<Option<BodiesJson>> {
        Ok(None)
    }

    async fn get_existing_config(&self, _path: &str) -> Result<Option<FuzzerConfigArtifact>> {
        Ok(None)
    }
}

fn fake_bodies(contract_name: &str, contract_path: &str) -> BodiesJson {
    BodiesJson {
        meta: BodiesMeta {
            contract: contract_name.to_string(),
            contract_path: contract_path.to_string(),
            solidity: "^0.8.0".to_string(),
            generated_at: "demo".to_string(),
        },
        handler: HandlerBodies {
            contract_name: format!("{}Handler", contract_name),
            imports: vec![],
            state_vars: vec![],
            ghost_vars: vec![],
            constructor_signature: "constructor() {}".to_string(),
            constructor_body: vec![],
            functions: IndexMap::new(),
            target_selectors: String::new(),
        },
        invariant_test: InvariantTestBodies {
            contract_name: format!("{}Test", contract_name),
            imports: vec![],
            state_vars: vec![],
            set_up_body: vec![],
            invariants: IndexMap::new(),
        },
    }
}

fn fake_foundry_config() -> FoundryConfig {
    FoundryConfig {
        depth: 100,
        runs: 256,
        seed: "0x1".to_string(),
        max_test_rejects: 65536,
        dictionary_weight: 40,
        call_sequence_weights: HashMap::new(),
        current_toml: None,
    }
}

/// Demo wiring that uses mock adapters instead of live engines.
pub struct DemoCompositionRoot;

impl DemoCompositionRoot {
    pub fn build() -> Box<dyn OrchestratorPort> {
        let output: Box<dyn crate::reporter::ports::outbound::OutputPort> =
            Box::new(TerminalOutput::new());
        let reporter = Box::new(Reporter::new(output));
        let run_session = Box::new(RunSessionUseCase::new(
            Box::new(MockLlmEngine),
            Box::new(MockFuzzerEngine),
            Box::new(MockExecutor),
            reporter,
            Box::new(MockReader),
        ));
        Box::new(Orchestrator::new(run_session))
    }
}
