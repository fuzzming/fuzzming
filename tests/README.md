# Integration Tests

End-to-end tests that verify each component works correctly with real adapters. No external LLM calls are made: the file system is the only real I/O. The fuzzer integration tests spawn real `forge` subprocesses.

## Contents

- [How to run](#how-to-run)
- [Executor integration tests](#executor-executorexecutor_integrationrs)
- [Reader integration tests](#reader-readerreader_integrationrs)
- [Fuzzer integration tests](#fuzzer-fuzzerfuzzer_integrationrs)
- [Unit tests](#unit-tests-in-module)
- [Fixtures](#fixtures)

---

## How to run

```bash
# All tests (unit + integration)
cargo test

# Individual integration suites
cargo test --test executor_integration
cargo test --test reader_integration
cargo test --test fuzzer_integration

# Unit tests only (no forge required)
cargo test --lib
```

> **Note:** The fuzzer integration tests require Foundry (`forge`). `ForgeRunner` automatically prepends `~/.foundry/bin` to the subprocess `PATH`, so the tests work even when `forge` is not in the system `PATH`.

---

## Executor: `executor/executor_integration.rs`

Verifies the executor write pipeline: `BodiesJson` → JSON artifact + Solidity files on disk.

Uses real `FileSystemWriter` and `SolidityGenerator` writing to `tests/output/`. Input is loaded from `tests/fixtures/Vault.bodies.json`.

### Tests

| Test | What is checked |
|---|---|
| `executor_generates_vault_files` | Full write pipeline: bodies JSON, Handler `.sol`, InvariantTest `.sol` |

### Assertions

| Artifact | Assertion |
|---|---|
| `.fuzzming/Vault/Vault.bodies.json` | File exists and round-trips through `serde_json` without data loss |
| `test/fuzzming/Vault/VaultHandler.sol` | Contract declaration `VaultHandler is Test {` is present; all handler function names appear |
| `test/fuzzming/Vault/VaultInvariantTest.sol` | Contract declaration present; `setUp` present; function count matches `invariants.len() + 1` |

The `tests/output/` directory is gitignored: safe to delete and regenerate at any time.

---

## Reader: `reader/reader_integration.rs`

Verifies that the reader correctly cleans Solidity source code and loads coverage context artifacts.

Uses real `SolidityContractReader` and `FileSystemReader` against a `TempDir`.

### Tests

| Test | What is checked |
|---|---|
| `get_contract_context_strips_comments` | All three Solidity comment forms stripped; function body preserved |
| `get_coverage_context_returns_none_when_file_missing` | Returns `None` instead of an error when the artifact file does not exist |
| `get_coverage_context_reads_enriched_json` | Reads a pre-written `CoverageContext` JSON file and deserialises all fields correctly |

### Comment stripping assertions (`get_contract_context`)

| Input | Expected output |
|---|---|
| `// single line comment` | Absent |
| `/* block comment */` | Absent |
| `// inline comment` after a statement | Absent |
| `function deposit() external {}` | Present: code is not lost |

### Coverage context assertions (`get_coverage_context_reads_enriched_json`)

The reader now loads coverage context from a pre-serialised `CoverageContext` JSON artifact (written by `FoundryCoverageReader` at the end of the previous round), not directly from `lcov.info`. The test writes a `CoverageContext` struct to a temp file and verifies:

- `line_found`, `line_hit` counters round-trip correctly
- `gaps` array has the correct length
- Individual gap fields (`line`, `gap_type`, `source_context`) deserialise correctly

---

## Fuzzer: `fuzzer/fuzzer_integration.rs`

Runs real `forge test` and (where applicable) `forge coverage` against a self-contained Foundry project in `tests/fixtures/foundry_vault/`. Requires Foundry to be installed.

All tests in this suite share a `WORKSPACE_MUT` mutex to prevent filesystem races when writing to the shared fixture workspace.

### Foundry fixture: `tests/fixtures/foundry_vault/`

A complete Foundry project with no external dependencies beyond `forge-std`:

| File | Description |
|---|---|
| `src/Vault.sol` | Single-asset vault: 1:1 shares, deposit cap of 1,000,000 tokens, no intentional bug |
| `test/fuzzming/Vault/VaultHandler.sol` | Invariant handler: ghost vars tracking deposits and withdrawals |
| `test/fuzzming/Vault/VaultInvariantTest.sol` | Two invariants: `totalAssets == deposits - withdrawals` and `totalAssets <= depositCap` |
| `foundry.toml` | `fuzzming` profile (invariant fuzzer config) and `coverage` profile |

### Tests

| Test | What is checked |
|---|---|
| `correct_vault_invariants_pass` | Real `forge test` passes all invariants → `FuzzOutcome::Pass` |
| `fuzz_output_written_to_workspace` | `.fuzzming/Vault/fuzz_output.txt` is written and contains `VaultInvariantTest` |
| `compile_error_gives_compile_error_outcome` | A handler with a deliberate syntax error produces `FuzzOutcome::CompileError` |
| `healthy_contract_runs_when_peer_has_compile_error` | When one contract's code won't compile, its peer is temporarily stashed, forge runs without it, and the peer still gets `FuzzOutcome::Pass` while the erroring contract gets `FuzzOutcome::CompileError` |
| `leftover_disabled_dirs_are_restored` | A `.fuzzming-disabled/` stash left by a previous crashed session is restored to `test/fuzzming/` before the next forge run |

### Compile error isolation

When forge encounters a compile error, the entire workspace fails to build. FuzzMing's isolation strategy:

1. Identifies the contracts with compile errors by repeatedly disabling one `test/fuzzming/<Contract>/` directory at a time (moving it to `.fuzzming-disabled/<Contract>/`) until forge compiles.
2. Runs `forge test` without the erroring contracts.
3. Restores all stashed directories after the run.

The `healthy_contract_runs_when_peer_has_compile_error` test verifies this full stash-run-restore cycle.

---

## Unit tests (in-module)

The following unit tests live inside source files and run with `cargo test --lib`:

### `ForgeRunner`: `src/fuzzer/adapters/outbound/forge_runner.rs`

Verifies forge output parsing against a hardcoded multi-bug forge stdout fixture.

| Test | What is checked |
|---|---|
| `detects_all_three_bugs` | `collect_bugs` finds all 3 failing invariants in a multi-bug output |
| `call_sequence_extracted_per_bug` | Each `BugInfo` carries the correct call sequence lines |
| `no_duplicate_bugs_from_summary_section` | The forge summary block (printed twice) does not produce duplicate bugs |
| `empty_bugs_for_non_invariant_failure` | A regular test failure (not an invariant) returns an empty bug list |
| `filter_output_captures_contract_section` | `filter_output` captures the full section for the target contract and no other |
| `filter_lcov_keeps_matching_sf_records` | `filter_lcov` keeps only `SF:` blocks whose path contains the contract name |

### `Prompt`: `src/generator/domain/prompt.rs`

Verifies that prompt assembly includes the correct sections.

| Test | What is checked |
|---|---|
| `assembled_prompt_includes_context_sections_and_messages` | Two messages (system + user); `fuzz_output` and `coverage` appear in `context_sections`; user message contains `Round: N`, `FUZZ OUTPUT`, `COVERAGE SUMMARY` |
| `round_one_prompt_includes_full_generation_instruction` | Round 1 user message includes the full-generation instruction; `context_sections` is empty |
| `confirmed_bugs_appear_in_prompt_and_context_sections` | `confirmed_bugs` is non-empty → `CONFIRMED BUGS` appears in user message; `confirmed_bugs` appears in `context_sections` |

---

## Fixtures

`tests/fixtures/Vault.bodies.json`: a complete `BodiesJson` for a Vault contract with two handler functions and two invariants. Used by the executor integration test.

`tests/fixtures/foundry_vault/`: a self-contained Foundry project used by the fuzzer integration tests. The `out/`, `cache/`, `.fuzzming/`, and `lcov.info` artifacts are gitignored: forge regenerates them on each run.
