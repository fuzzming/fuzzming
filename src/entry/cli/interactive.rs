use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use dialoguer::{Confirm, Input};
use walkdir::WalkDir;

use crate::entry::cli::arg_parser::RunArgs;
use crate::entry::cli::ui::CliUi;
use crate::shared::models::PromptMode;

const CONFIG_FILE_NAME: &str = "fuzzming.config";

#[derive(Debug, Default, Clone)]
struct ConfigFile {
    targets: Vec<String>,
    max_rounds: Option<u32>,
    model: Option<String>,
    llm_key: Option<String>,
    workspace_root: Option<PathBuf>,
    max_tokens: Option<u32>,
    llm_timeout_secs: Option<u64>,
    full_coverage_rounds: Option<u32>,
    prompt_mode: Option<PromptMode>,
}

#[derive(Debug, Clone)]
pub struct ResolvedCliConfig {
    pub targets: Vec<String>,
    pub max_rounds: u32,
    pub model: String,
    pub llm_key: String,
    pub workspace_root: PathBuf,
    pub verbose: bool,
    pub max_tokens: Option<u32>,
    pub llm_timeout_secs: u64,
    pub full_coverage_rounds: u32,
    pub prompt_mode: PromptMode,
}

pub fn resolve_cli_config(args: &RunArgs) -> Result<ResolvedCliConfig> {
    let config_path = default_config_path()?;
    let stored = load_config(&config_path).unwrap_or_default();

    let needs_prompt = args.interactive
        || args.model.is_none()
        || args.llm_key.is_none()
        || args.workspace_root.is_none()
        || args.targets.is_empty();

    let mut resolved = if args.from_config {
        resolve_from_config_only(&stored)?
    } else if args.defaults {
        resolve_with_defaults(args)?
    } else if needs_prompt {
        prompt_for_config(args, &stored, &config_path)?
    } else {
        resolve_from_args(args, &stored)?
    };

    if resolved.targets.is_empty() {
        resolved.targets = discover_targets(&resolved.workspace_root)?;
    }

    Ok(resolved)
}

fn resolve_from_config_only(stored: &ConfigFile) -> Result<ResolvedCliConfig> {
    let model = stored.model.clone()
        .ok_or_else(|| anyhow!("fuzzming.config is missing 'model'"))?;
    let llm_key = stored.llm_key.clone()
        .ok_or_else(|| anyhow!("fuzzming.config is missing 'llm_key'"))?;
    let workspace_root = stored.workspace_root.clone()
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(ResolvedCliConfig {
        targets: stored.targets.clone(),
        max_rounds: stored.max_rounds.unwrap_or(10),
        model,
        llm_key,
        workspace_root,
        verbose: false,
        max_tokens: stored.max_tokens,
        llm_timeout_secs: stored.llm_timeout_secs.unwrap_or(120),
        full_coverage_rounds: stored.full_coverage_rounds.unwrap_or(2),
        prompt_mode: stored.prompt_mode.clone().unwrap_or_default(),
    })
}

fn resolve_with_defaults(args: &RunArgs) -> Result<ResolvedCliConfig> {
    let model = args.model.clone()
        .ok_or_else(|| anyhow!("--defaults requires --model or LLM_MODEL env var"))?;
    let llm_key = args.llm_key.clone()
        .ok_or_else(|| anyhow!("--defaults requires --llm-key or LLM_KEY env var"))?;
    let workspace_root = args.workspace_root.clone()
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(ResolvedCliConfig {
        targets: args.targets.clone(),
        max_rounds: args.max_rounds.unwrap_or(10),
        model,
        llm_key,
        workspace_root,
        verbose: args.verbose,
        max_tokens: args.max_tokens,
        llm_timeout_secs: args.llm_timeout_secs,
        full_coverage_rounds: args.full_coverage_rounds,
        prompt_mode: PromptMode::default(),
    })
}

fn resolve_from_args(args: &RunArgs, stored: &ConfigFile) -> Result<ResolvedCliConfig> {
    let workspace_root = args
        .workspace_root
        .clone()
        .or_else(|| stored.workspace_root.clone())
        .unwrap_or_else(|| PathBuf::from("."));

    let model = args
        .model
        .clone()
        .or_else(|| stored.model.clone())
        .ok_or_else(|| anyhow!("missing LLM model"))?;

    let llm_key = args
        .llm_key
        .clone()
        .or_else(|| stored.llm_key.clone())
        .ok_or_else(|| anyhow!("missing LLM key"))?;

    let max_rounds = args.max_rounds.or(stored.max_rounds).unwrap_or(10);

    let prompt_mode = stored.prompt_mode.clone().unwrap_or_default();

    Ok(ResolvedCliConfig {
        targets: args.targets.clone(),
        max_rounds,
        model,
        llm_key,
        workspace_root,
        verbose: args.verbose,
        max_tokens: stored.max_tokens.or(args.max_tokens),
        llm_timeout_secs: stored.llm_timeout_secs.unwrap_or(args.llm_timeout_secs),
        full_coverage_rounds: stored.full_coverage_rounds.unwrap_or(args.full_coverage_rounds),
        prompt_mode,
    })
}

fn prompt_for_config(
    args: &RunArgs,
    stored: &ConfigFile,
    config_path: &Path,
) -> Result<ResolvedCliConfig> {
    let ui = CliUi::new();
    ui.divider();
    let default_workspace = args
        .workspace_root
        .clone()
        .or_else(|| stored.workspace_root.clone())
        .unwrap_or_else(|| PathBuf::from("."));

    let workspace_root = Input::<String>::new()
        .with_prompt(ui.question("Workspace root"))
        .with_initial_text(default_workspace.to_string_lossy())
        .interact_text()?
        .trim()
        .to_string();

    let workspace_root = if workspace_root.is_empty() {
        default_workspace
    } else {
        PathBuf::from(workspace_root)
    };

    ui.divider();
    let targets = if !args.targets.is_empty() {
        args.targets.clone()
    } else if !stored.targets.is_empty() {
        let use_saved = Confirm::new()
            .with_prompt(ui.question(&format!(
                "Use saved target contracts ({})?",
                stored.targets.join(",")
            )))
            .default(true)
            .interact()?;
        if use_saved {
            stored.targets.clone()
        } else {
            let targets_input = Input::<String>::new()
                .with_prompt(ui.question("Target contracts (comma-separated, empty = all)"))
                .allow_empty(true)
                .interact_text()?;
            parse_targets(&targets_input)
        }
    } else {
        let targets_input = Input::<String>::new()
            .with_prompt(ui.question("Target contracts (comma-separated, empty = all)"))
            .allow_empty(true)
            .interact_text()?;
        parse_targets(&targets_input)
    };

    let model_default = args
        .model
        .clone()
        .or_else(|| stored.model.clone())
        .unwrap_or_else(|| "openrouter/meta-llama/llama-3.3-70b-instruct".to_string());

    ui.divider();
    let model = Input::<String>::new()
        .with_prompt(ui.question("LLM model"))
        .with_initial_text(model_default)
        .interact_text()?
        .trim()
        .to_string();

    let tier_default = stored
        .prompt_mode
        .as_ref()
        .map(|t| t.as_str().to_string())
        .unwrap_or_else(|| PromptMode::Concise.as_str().to_string());

    ui.divider();
    let tier_input = Input::<String>::new()
        .with_prompt(ui.question("Prompt mode (concise = Claude/GPT-4+/Gemini, guided = open-source models)"))
        .with_initial_text(&tier_default)
        .interact_text()?;

    let prompt_mode = tier_input
        .trim()
        .parse::<PromptMode>()
        .unwrap_or_default();

    let llm_key_hint = if stored.llm_key.is_some() {
        "(leave blank to keep saved key)"
    } else {
        "(required)"
    };

    ui.divider();
    let llm_key_input = Input::<String>::new()
        .with_prompt(ui.question(&format!("LLM key {}", llm_key_hint)))
        .allow_empty(true)
        .interact_text()?;

    let llm_key = if llm_key_input.trim().is_empty() {
        stored
            .llm_key
            .clone()
            .ok_or_else(|| anyhow!("missing LLM key"))?
    } else {
        llm_key_input
    };

    let max_rounds_default = args.max_rounds.or(stored.max_rounds).unwrap_or(10);

    ui.divider();
    let max_rounds = Input::<u32>::new()
        .with_prompt(ui.question("Max rounds"))
        .with_initial_text(max_rounds_default.to_string())
        .interact_text()?;

    ui.divider();
    let llm_timeout_secs = Input::<u64>::new()
        .with_prompt(ui.question("LLM timeout (seconds)"))
        .with_initial_text(stored.llm_timeout_secs.unwrap_or(args.llm_timeout_secs).to_string())
        .interact_text()?;

    ui.divider();
    let max_tokens_raw = Input::<u32>::new()
        .with_prompt(ui.question("Max tokens per LLM call (0 = no limit)"))
        .with_initial_text(stored.max_tokens.or(args.max_tokens).unwrap_or(0).to_string())
        .interact_text()?;
    let max_tokens: Option<u32> = if max_tokens_raw == 0 { None } else { Some(max_tokens_raw) };

    ui.divider();
    let full_coverage_rounds = Input::<u32>::new()
        .with_prompt(ui.question("Stop after N consecutive rounds at 100% coverage"))
        .with_initial_text(stored.full_coverage_rounds.unwrap_or(args.full_coverage_rounds).to_string())
        .interact_text()?;

    let resolved = ResolvedCliConfig {
        targets,
        max_rounds,
        model,
        llm_key,
        workspace_root,
        verbose: args.verbose,
        max_tokens,
        llm_timeout_secs,
        full_coverage_rounds,
        prompt_mode,
    };

    save_config(config_path, &resolved)?;
    ui.success("Saved fuzzming.config");
    ui.warn("fuzzming.config contains your API key, make sure it is gitignored");
    println!();

    Ok(resolved)
}

fn parse_targets(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn load_config(path: &Path) -> Result<ConfigFile> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(ConfigFile::default()),
        Err(err) => return Err(err.into()),
    };

    let mut config = ConfigFile::default();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.splitn(2, '=');
        let key = parts.next().unwrap_or("").trim();
        let value = parts.next().unwrap_or("").trim();
        match key {
            "targets" => config.targets = parse_targets(value),
            "max_rounds" => config.max_rounds = value.parse::<u32>().ok(),
            "model" => config.model = Some(value.to_string()),
            "llm_key" => config.llm_key = Some(value.to_string()),
            "workspace_root" => config.workspace_root = Some(PathBuf::from(value)),
            "max_tokens" => config.max_tokens = value.parse::<u32>().ok().filter(|&v| v > 0),
            "llm_timeout_secs" => config.llm_timeout_secs = value.parse::<u64>().ok(),
            "full_coverage_rounds" => config.full_coverage_rounds = value.parse::<u32>().ok(),
            "prompt_mode" => config.prompt_mode = value.parse::<PromptMode>().ok(),
            _ => {}
        }
    }

    Ok(config)
}

fn save_config(path: &Path, resolved: &ResolvedCliConfig) -> Result<()> {
    let content = format!(
        "targets={}\nmax_rounds={}\nmodel={}\nllm_key={}\nworkspace_root={}\nmax_tokens={}\nllm_timeout_secs={}\nfull_coverage_rounds={}\nprompt_mode={}\n",
        resolved.targets.join(","),
        resolved.max_rounds,
        resolved.model,
        resolved.llm_key,
        resolved.workspace_root.to_string_lossy(),
        resolved.max_tokens.unwrap_or(0),
        resolved.llm_timeout_secs,
        resolved.full_coverage_rounds,
        resolved.prompt_mode.as_str(),
    );

    fs::write(path, content)?;
    ensure_gitignored(path)?;
    Ok(())
}

fn ensure_gitignored(config_path: &Path) -> Result<()> {
    let dir = config_path.parent().unwrap_or(Path::new("."));
    let gitignore_path = dir.join(".gitignore");
    let entry = CONFIG_FILE_NAME.to_string();

    let already_ignored = if gitignore_path.exists() {
        let contents = fs::read_to_string(&gitignore_path)?;
        contents.lines().any(|l| l.trim() == entry)
    } else {
        false
    };

    if !already_ignored {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&gitignore_path)?;
        use std::io::Write;
        writeln!(file, "{}", entry)?;
    }

    Ok(())
}

pub fn workspace_root_from_config() -> Option<PathBuf> {
    let path = default_config_path().ok()?;
    load_config(&path).ok()?.workspace_root
}

fn default_config_path() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join(CONFIG_FILE_NAME))
}

fn discover_targets(workspace_root: &Path) -> Result<Vec<String>> {
    let src_root = workspace_root.join("src");
    let contracts_root = workspace_root.join("contracts");

    let search_root = if src_root.exists() {
        src_root
    } else if contracts_root.exists() {
        contracts_root
    } else {
        return Err(anyhow!(
            "no targets specified and neither 'src/' nor 'contracts/' exist in '{}'",
            workspace_root.to_string_lossy()
        ));
    };

    let mut targets = Vec::new();
    for entry in WalkDir::new(&search_root).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            let path = entry.path();
            if path.extension().map(|e| e == "sol").unwrap_or(false) {
                let rel = path
                    .strip_prefix(workspace_root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();
                targets.push(rel);
            }
        }
    }

    targets.sort();
    Ok(targets)
}
