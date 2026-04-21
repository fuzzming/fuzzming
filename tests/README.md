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
```

---

## Executor — `executor/executor_integration.rs`

Verifies the executor write pipeline: `BodiesJson` → JSON file + Solidity files on disk.

Uses real `FileSystemWriter` and `SolidityGenerator` writing to `tests/output/`.

| Assertion | What is checked |
|---|---|
| `test/Vault.bodies.json` exists and round-trips through serde | bodies JSON is written correctly |
| `VaultHandler.sol` contains the contract declaration and all handler function signatures | Solidity handler is generated from `BodiesJson` |
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

## Fixtures

`tests/fixtures/Vault.bodies.json` — a complete `BodiesJson` for a Vault contract with two handler functions and two invariants. Used by both the executor and generator tests.
