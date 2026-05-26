use std::sync::Arc;

use crate::executor::adapters::inbound::Executor;
use crate::executor::adapters::outbound::{FileSystemWriter, FoundryConfigWriter, SolidityGenerator};
use crate::executor::use_cases::ExecuteUseCase;
use crate::fuzzer::adapters::inbound::Fuzzer as FuzzerAdapter;
use crate::fuzzer::adapters::outbound::{FileSystemFuzzerOutput, ForgeRunner};
use crate::fuzzer::use_cases::RunFuzzerUseCase;
use crate::generator::adapters::inbound::Generator;
use crate::generator::adapters::outbound::{LiteLlmClient, LiteLlmGenerationAdapter};
use crate::generator::use_cases::GeneratorRunUseCase;
use crate::orchestrator::adapters::inbound::Orchestrator;
use crate::orchestrator::use_cases::RunSessionUseCase;
use crate::reader::adapters::inbound::Reader;
use crate::reader::adapters::outbound::{FileSystemReader, SolidityContractReader};
use crate::reader::use_cases::ReadUseCase;
use crate::reporter::adapters::inbound::Reporter;
use crate::reporter::adapters::outbound::TerminalOutput;
use crate::shared::models::SessionConfig;
use crate::shared::ports::OrchestratorPort;

pub struct CompositionRoot;

impl CompositionRoot {
    pub fn build(config: SessionConfig) -> Box<dyn OrchestratorPort> {
        let workspace = config.workspace_root.clone();
        let model = config.model.clone();
        let api_key = config.llm_key.clone();

        // Generator (LLM engine)
        let llm_client = Box::new(LiteLlmClient::new(&model, Some(0.1), Some(config.max_tokens), config.llm_timeout_secs));
        let generation_adapter =
            Box::new(LiteLlmGenerationAdapter::new(&model, &api_key, llm_client));
        let generator_use_case = Box::new(GeneratorRunUseCase::new(generation_adapter));
        let generator = Box::new(Generator::new(generator_use_case));

        // Fuzzer engine
        let forge_runner = Box::new(ForgeRunner::new(workspace.clone()));
        let fuzzer_output = Box::new(FileSystemFuzzerOutput::new(workspace.clone()));
        let fuzzer_use_case = Box::new(RunFuzzerUseCase::new(forge_runner, fuzzer_output, workspace.clone()));
        let fuzzer = Box::new(FuzzerAdapter::new(fuzzer_use_case));

        // Executor
        let fs_writer = FileSystemWriter::new(workspace.clone());
        let code_generator = Arc::new(SolidityGenerator);
        let config_writer = Arc::new(FoundryConfigWriter);
        let executor_use_case =
            Box::new(ExecuteUseCase::new(fs_writer, code_generator, config_writer));
        let executor = Box::new(Executor::new(executor_use_case));

        // Reader
        let fs_reader = Arc::new(FileSystemReader::new(workspace.clone()));
        let contract_reader = Arc::new(SolidityContractReader::new(fs_reader.clone()));
        let reader_use_case = Box::new(ReadUseCase::new(contract_reader, fs_reader));
        let reader = Box::new(Reader::new(reader_use_case));

        let output: Box<dyn crate::reporter::ports::outbound::OutputPort> =
            Box::new(TerminalOutput::new());
        let reporter = Box::new(Reporter::new(output));

        // Orchestrator
        let run_session = Box::new(RunSessionUseCase::new(
            generator, fuzzer, executor, reporter, reader,
        ));
        Box::new(Orchestrator::new(run_session))
    }
}
