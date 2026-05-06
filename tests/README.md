# Integration Tests

End-to-end tests that verify each component works correctly with real adapters. No external services are called — the file system is the only real I/O, and the LLM HTTP client is replaced with a mock.

## How to run

```bash
# All tests
cargo test

# Individual suites
cargo test --test executor_integration
cargo test --test reader_integration
cargo test --test generation_adapter_test
cargo test --test fuzzer_integration
```

> **Note:** The fuzzer integration tests require forge to be installed (`~/.foundry/bin/forge`). `ForgeRunner` adds `~/.foundry/bin` to the subprocess PATH automatically, so the tests work even when forge is not in the system PATH.

---

## Executor — `executor/executor_integration.rs`

Verifies the executor write pipeline: `BodiesJson` → JSON file + Solidity files on disk.

Uses real `FileSystemWriter` and `SolidityGenerator` writing to `tests/output/`.

| Assertion | What is checked |
|---|---|
| `test/Vault.bodies.json` exists and round-trips through serde | bodies JSON is written correctly |
| `VaultHandler.sol` contains `contract VaultHandler is Test {` and all handler function signatures | Solidity handler inherits from `Test` and is generated from `BodiesJson` |
| `VaultInvariantTest.sol` contains the contract declaration, `setUp`, and the correct function count | Solidity invariant test is generated from `BodiesJson` |

After the test passes, inspect the generated files:

```
tests/output/
└── test/
    ├── Vault.bodies.json
    ├── handlers/
    │   └── VaultHandler.sol
    └── invariants/
        └── VaultInvariantTest.sol
```

The `tests/output/` directory is gitignored — safe to delete and regenerate at any time.

---

## Reader — `reader/reader_integration.rs`

Verifies that the reader correctly cleans Solidity source before passing it to the generator.

Uses real `SolidityContractReader` and `FileSystemReader` against a `TempDir`.

| Assertion | What is checked |
|---|---|
| `// single line`, `/* block */`, and inline `//` comments are absent from the output | All three Solidity comment forms are stripped when `include_comments: false` |
| `function deposit` is still present | The actual code is not lost during comment stripping |

---

## Generator — `generator/generation_adapter_test.rs`

Verifies that `LiteLlmGenerationAdapter` correctly orchestrates the 3-stage LLM call chain.

The `LlmClientPort` is replaced with a `MockLlmClient` that returns pre-loaded responses in sequence — no HTTP calls are made.

| Assertion | What is checked |
|---|---|
| 3 sequential mock responses (analysis → bodies → config) produce a `GenerationResponse::Full` | The full stage chain completes without error |

---

## Patch Applier — `executor::use_cases::apply_patch` (unit tests)

Verifies `apply_patches<T>()` — the dot-path JSON patch engine used by the executor to apply round-N LLM updates to existing `BodiesJson` and `FoundryConfig` artifacts.

Run with:

```bash
cargo test apply_patch
```

### BodiesJson patches

| Test | What is checked |
|---|---|
| `replace_existing_function_body` | Replace a handler function body; other functions untouched |
| `add_new_function` | Add a new key to `handler.functions`; map grows by 1 |
| `remove_function` | Remove a key from `handler.functions`; map shrinks by 1 |
| `add_new_invariant` | Add a new entry to `invariantTest.invariants` |
| `replace_meta_field` | Replace a scalar field at `meta.solidity` |
| `replace_array_element_by_index` | Replace `handler.stateVars.0` by numeric index |
| `add_to_array_appends` | `Add` on an array always appends; the last path segment is ignored |
| `remove_array_element_by_index` | Remove `handler.stateVars.0`; array becomes empty |
| `multiple_patches_applied_in_order` | Add then Replace on the same key — second patch wins |
| `bracket_navigation_syntax` | Navigate an intermediate array segment via numeric index |

### FoundryConfig patches

| Test | What is checked |
|---|---|
| `replace_depth` | Replace scalar field at root level |
| `replace_runs` | Replace another root-level scalar |
| `add_call_sequence_weight` | Add a new key into `call_sequence_weights` |
| `replace_call_sequence_weight` | Overwrite an existing weight value |
| `remove_call_sequence_weight` | Remove a weight entry from the map |

### FuzzerConfigArtifact

| Test | What is checked |
|---|---|
| `patch_fuzzer_config_artifact` | Path navigates through the enum variant name (`"Foundry.depth"`) |

### Error cases

| Test | What is checked |
|---|---|
| `error_on_empty_path` | Empty string path is rejected immediately |
| `error_on_add_duplicate_key` | `Add` to an existing key fails with "already exists" |
| `error_on_remove_missing_key` | `Remove` on a missing key fails with "not found" |
| `error_on_missing_intermediate_key` | Navigation through a nonexistent intermediate key fails |
| `error_on_array_index_out_of_bounds` | Index past end of array fails with "out of bounds" |

---

---

## Fuzzer — `fuzzer/fuzzer_integration.rs`

Runs real `forge test` and `forge coverage` against a self-contained Foundry project in `tests/fixtures/foundry_vault/`. Requires forge to be installed.

Run with:

```bash
cargo test --test real_fuzzer_integration
```

### Foundry fixture — `tests/fixtures/foundry_vault/`

A complete Foundry project with no external dependencies beyond forge-std:

| File | Description |
|---|---|
| `src/Token.sol` | Minimal ERC20 token with a `mint` function |
| `src/Vault.sol` | Single-asset vault — 1:1 shares, deposit cap of 1,000,000 tokens |
| `test/handlers/VaultHandler.sol` | Invariant handler — 3 actors, ghost vars tracking deposits and withdrawals |
| `test/invariants/VaultInvariantTest.sol` | Two invariants: `totalAssets == deposits - withdrawals` and `totalAssets <= depositCap` |
| `foundry.toml` | `fuzzming` profile (512 runs, depth 200 under `[profile.fuzzming.invariant]`) and `coverage` profile |

### Tests

| Test | What is checked |
|---|---|
| `correct_vault_invariants_pass` | Real `forge test --profile fuzzming` passes both invariants → `FuzzOutcome::Pass` |
| `fuzz_output_written_to_workspace` | `.fuzzming/fuzz_output.txt` is written to the workspace and contains forge's `Suite result` line |

The `foundry_vault/out/`, `cache/`, `lib/`, `.fuzzming/`, and `lcov.info` are gitignored — forge regenerates them on each run.

---

## Fixtures

`tests/fixtures/Vault.bodies.json` — a complete `BodiesJson` for a Vault contract with two handler functions and two invariants. Used by both the executor and generator tests.
