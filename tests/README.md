# Executor Integration Tests

End-to-end tests that verify the executor correctly generates Solidity files from a `BodiesJson` artifact.

## How to run

```bash
cargo test --test executor_integration
```

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

The `tests/output/` directory is gitignored — it is safe to delete and regenerate at any time.

## What is tested

| Assertion | What is checked |
|-----------|-----------------|
| `test/Vault.bodies.json` exists and round-trips | bodies JSON is written correctly |
| `VaultHandler.sol` contains handler functions | Solidity handler is generated from function bodies |
| `VaultInvariantTest.sol` contains invariants | Solidity invariant test is generated from invariant bodies |

## Fixture

`tests/fixtures/Vault.bodies.json` is the input artifact — a `Vault` contract with two handler functions and two invariants.

To test a different contract, add its `bodies.json` to `tests/fixtures/` and write a new test in `executor_integration.rs`.
