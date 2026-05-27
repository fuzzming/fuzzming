# Contributing to FuzzMing

Thanks for your interest in contributing. This document covers everything you need to get started, from setting up the environment to opening a PR.

---

## Table of Contents

- [Dev environment](#dev-environment)
- [Project orientation](#project-orientation)
- [Running tests](#running-tests)
- [Branch and PR conventions](#branch-and-pr-conventions)
- [Architectural rules](#architectural-rules)
- [Adding a new language or fuzzer](#adding-a-new-language-or-fuzzer)
- [Code style](#code-style)
- [Commit style](#commit-style)

---

## Dev environment

**Requirements:**

| Tool | Version | Install |
|---|---|---|
| Rust | stable | [rustup.rs](https://rustup.rs) |
| Foundry (`forge`) | latest | `curl -L https://foundry.paradigm.xyz \| bash` |

**Setup:**

```bash
git clone https://github.com/your-org/fuzzming
cd fuzzming
cargo build
cargo test
```

If `cargo test` passes, you're ready.

---

## Project orientation

Read [docs/shared.md](docs/shared.md) first. Understanding the shared data layer (models, ports, requests, responses) is the fastest way to understand how components talk to each other.

Then read the doc for the component you want to change:

| Component | Doc | What it owns |
|---|---|---|
| Orchestrator | [docs/orchestrator.md](docs/orchestrator.md) | Session loop, round coordination, termination logic |
| Generator | [docs/generator.md](docs/generator.md) | LLM calls, prompt assembly, retry/repair |
| Executor | [docs/executor.md](docs/executor.md) | Writing Solidity files and `foundry.toml` |
| Fuzzer | [docs/fuzzer.md](docs/fuzzer.md) | `forge test`, output parsing, coverage |
| Reader | [docs/reader.md](docs/reader.md) | Reading source files and previous-round artifacts |
| Reporter | [docs/reporter.md](docs/reporter.md) | Formatting and emitting results |
| Composition root | [docs/composition.md](docs/composition.md) | The only file that wires concrete types |

---

## Running tests

```bash
# All tests (unit + integration)
cargo test

# Individual integration suites
cargo test --test executor_integration
cargo test --test reader_integration
cargo test --test fuzzer_integration  # requires forge

# Unit tests only (no forge required)
cargo test --lib

# Single test by name
cargo test correct_vault_invariants_pass
```

The fuzzer integration tests spawn real `forge` subprocesses. They require Foundry to be installed and `~/.foundry/bin/forge` to exist: `ForgeRunner` adds that path automatically, so `forge` does not need to be on your system `PATH`.

---

## Branch and PR conventions

**Branch naming:**

```
feat/<short-description>      new feature
fix/<short-description>       bug fix
refactor/<short-description>  refactor without behaviour change
docs/<short-description>      documentation only
chore/<short-description>     tooling, deps, CI
```

**PRs:**

- Target `main`.
- Keep scope tight: one concern per PR. A PR that adds a new fuzzer adapter should not also refactor the reporter.
- Include a clear description of what changed and why, especially for changes to `src/shared/`.
- `cargo test` must pass before opening a PR.

---

## Architectural rules

These are hard rules. PRs that violate them will be asked to restructure.

**1. No component imports another component.**
Components only import from `src/shared/`. The only file allowed to import concrete types from multiple components is `src/composition/composition_root.rs`.

**2. All inter-component contracts live in `src/shared/ports/`.**
If you add a new method to a port, add it there, not inside the component.

**3. Changes to `src/shared/` are breaking changes.**
Any change to a shared model or port affects every component that uses it. Treat them like public API: add fields with caution, never remove fields without checking all consumers.

**4. All OS calls live in `adapters/outbound/` only.**
No `tokio::fs`, `std::process`, or HTTP calls inside use cases or inbound adapters. Use cases call outbound ports; outbound adapters do the I/O.

**5. The LLM never controls file paths.**
`BodiesJson` has no `output_path` fields. All paths are derived by the executor from `contract_name`. If you add new file output, derive the path from `contract_name`, never from LLM output.

**6. Inbound adapters are thin delegators.**
An inbound adapter implements a shared port and delegates to the use case via an inbound port. It holds no state and contains no logic.

---

## Adding a new language or fuzzer

The architecture is designed for this. Full checklist in [docs/composition.md](docs/composition.md). Summary:

1. Add a new `ContractReaderPort` adapter in `src/reader/adapters/outbound/`.
2. Add a new `CodeGeneratorPort` adapter in `src/executor/adapters/outbound/`.
3. Add a new `ConfigWriterPort` adapter in `src/executor/adapters/outbound/`.
4. Add a new `TestRunnerPort` adapter in `src/fuzzer/adapters/outbound/`.
5. Add the new config struct to `src/shared/models/` and a new variant to `FuzzerConfigArtifact`.
6. Add a new prompt template branch to the generator.
7. Add new `Language` and `Fuzzer` enum variants to `SessionConfig`.
8. Wire the new adapters in `CompositionRoot::build` behind the new variants.

The orchestrator, reporter, session loop, and all shared ports require zero changes.

---

## Code style

- **No comments by default.** Only add a comment when the *why* is non-obvious: a hidden constraint, a workaround for a specific bug, a subtle invariant. If removing the comment wouldn't confuse a future reader, don't write it.
- **No premature abstractions.** Three similar lines is better than a helper that exists for one caller. Don't design for hypothetical future requirements.
- **No error handling for impossible cases.** Trust internal code and framework guarantees. Only validate at system boundaries (user input, external APIs, subprocess output).
- **Prefer `PathBuf::join` over string concatenation** for all path construction.
- **Use `IndexMap` where insertion order matters** (generated Solidity functions, invariants).

---

## Commit style

```
type: short imperative description (72 chars max)
```

Types: `feat`, `fix`, `refactor`, `docs`, `chore`, `test`.

Examples:
```
feat: add cargo-fuzz adapter for Rust contract support
fix: restore stashed dirs before forge run on retry
docs: update orchestrator termination table for CompileError
```

No scope in parentheses, no ticket numbers in the subject line.

---

## License

By contributing you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).
