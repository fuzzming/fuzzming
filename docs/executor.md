# Executor — What changed

This document covers everything added or modified in the executor implementation.

---

## New files

### `src/lib.rs`
Added a library target so examples and future integration tests can import crate modules directly. `main.rs` is now a thin entry point that delegates to the library.

### `src/shared/artifacts/bodies_json.rs`
Defines the `BodiesJson` artifact — the structured JSON produced by the LLM each round. Every value is already valid Solidity. The generator assembles `.sol` files from these strings without any interpretation.

Key types:
- `BodiesJson` — top-level struct with `meta`, `handler`, `invariant_test`
- `HandlerBodies` — imports, state vars, ghost vars, constructor, functions (`IndexMap`), target selectors
- `InvariantTestBodies` — imports, state vars, setUp body, invariants (`IndexMap`)

`IndexMap` is used for `functions` and `invariants` so insertion order is preserved across serialisation rounds — critical for stable `targetSelectors` weights.

All fields serialise with camelCase keys to match the JSON schema expected by the LLM.

### `src/executor/writers/bodies_writer.rs`
Serialises `BodiesJson` to `test/<ContractName>.bodies.json` using `serde_json::to_string_pretty`.

### `src/executor/writers/solidity_generator.rs`
Two functions — `generate_handler` and `generate_invariant_test` — that assemble complete Solidity contracts from `BodiesJson` strings. The generator emits strings verbatim; it never interprets or reformats them.

### `src/executor/writers/solidity_generator_tests.rs`
Separate test file linked via `#[path]`. Six tests covering:
- Handler `.sol` file is created at the path declared in bodies.json
- Invariant test `.sol` file is created at the path declared in bodies.json
- Handler content contains all expected Solidity sections
- Invariant test content contains all expected Solidity sections
- Functions appear in insertion order (IndexMap guarantee)
- `bodies.json` is saved to disk and round-trips cleanly through `serde_json`

### `examples/Vault.bodies.json`
A complete real example using a Vault contract — two handler functions, two invariants, weighted `targetSelectors`. Used directly by the generate example.

### `examples/generate.rs`
Runnable example:
```bash
cargo run --example generate
```
Reads `examples/Vault.bodies.json`, writes `test/Vault.bodies.json` and both generated `.sol` files to `examples/output/`. Use this to inspect output after editing the JSON.

---

## Modified files

### `src/shared/artifacts/foundry_config.rs`
Added `current_toml: Option<String>`. The Reader provides the existing `foundry.toml` content; the Orchestrator packages it into `FoundryConfig` before calling the Executor. This lets `FoundryConfigWriter` patch only the managed sections without the Executor ever reading the file itself.

### `src/shared/artifacts/mod.rs`
Added `pub mod bodies_json` and the corresponding `pub use`.

### `src/llm/ports/executor_port.rs`
Replaced `write_invariants(InvariantSet)` with `write_bodies(BodiesJson)`. The old method wrote raw Solidity directly; the new method writes bodies.json and triggers the generator. `append_memory` was removed from the trait (the user removed it as out of scope for this iteration).

### `src/executor/infrastructure/file_system_writer.rs`
Implemented. Wraps `tokio::fs::create_dir_all` + `tokio::fs::write`. This is the only place in the codebase that calls those functions directly — all writers go through this struct.

### `src/executor/writers/mod.rs`
Replaced `invariant_writer` and `memory_writer` with `bodies_writer` and `solidity_generator`.

### `src/executor/writers/foundry_config_writer.rs`
Implemented. Builds the `[profile.fuzzming]` TOML section from `FoundryConfig` fields, then patches `foundry.toml` using `replace_or_append_section`. Writes `[profile.coverage]` when the section is absent (first round). Four unit tests cover append, replace, section preservation, and empty-base edge cases.

### `src/executor/executor.rs`
Implemented `ExecutorPort`:
- `write_bodies` — calls `write_bodies`, `generate_handler`, `generate_invariant_test` in sequence
- `write_foundry_config` — delegates to `foundry_config_writer`

Functions imported directly (`use ... ::fn_name`) rather than called with module-path prefix.

### `src/main.rs`
Simplified to a single entry point that imports from the library crate.

### `Cargo.toml`
Added:
- `[lib]` target pointing to `src/lib.rs`
- `indexmap = { version = "2", features = ["serde"] }` — ordered map for function/invariant keys
- `[dev-dependencies] tempfile = "3"` — temp directories in tests
