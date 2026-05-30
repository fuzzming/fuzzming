# Reader Component

The Reader is the **read gateway** of FuzzMing. Before every LLM round, the orchestrator asks the Reader for the contract source code and the coverage gaps. The Reader collects all of that and hands it back so the orchestrator can assemble the `RoundSignal`.

---

## Contents

- [The big picture](#the-big-picture)
- [Directory structure](#directory-structure)
- [Architecture layers](#architecture-layers)
- [What the orchestrator asks for](#what-the-orchestrator-asks-for)
- [Data flow](#data-flow)
- [Wiring at startup](#wiring-at-startup)
- [Known limitations](#known-limitations)

---

## The big picture

```
forge coverage  ──► .fuzzming/{Contract}/lcov.info          ─┐
forge test      ──► .fuzzming/{Contract}/fuzz_output.txt     ─┤──► Reader ──► Orchestrator ──► Generator
src/Vault.sol   ──► .sol file                                ─┤
executor        ──► .fuzzming/{Contract}/{Contract}.bodies.json ─┤
executor        ──► .fuzzming/{Contract}/{Contract}.config.json ─┘
```

The Reader never writes anything. It only reads and transforms.

---

## Directory structure

```
src/reader/
├── adapters/
│   ├── inbound/
│   │   └── reader.rs                      # Inbound adapter: implements ReaderPort, delegates to ReaderRunPort
│   └── outbound/
│       ├── file_system_reader.rs          # FileSystemReader: only place that calls tokio::fs::read
│       ├── solidity_contract_reader.rs    # Implements ContractReaderPort: reads .sol, strips comments
│       └── foundry_coverage_reader.rs     # Legacy adapter (unused by ReadUseCase)
├── ports/
│   ├── inbound/
│   │   └── reader_run_port.rs             # ReaderRunPort: inbound contract between adapter and use case
│   └── outbound/
│       ├── contract_reader_port.rs        # ContractReaderPort: read a .sol file
│       └── coverage_reader_port.rs        # Legacy port (unused by ReadUseCase)
└── use_cases/
    ├── read.rs                            # ReadUseCase: owns outbound ports, implements ReaderRunPort
    └── parse_lcov.rs                      # Pure function: LCOV text → list of CoverageGap
```

---

## Architecture layers

```
Orchestrator
    │
    └─ ReaderPort (shared/ports)
           │
    Reader (adapters/inbound)                       ← implements ReaderPort
           │
    ReaderRunPort (ports/inbound)                   ← inbound contract
           │
    ReadUseCase (use_cases)                         ← implements ReaderRunPort, owns outbound ports
           │
    ├─ ContractReaderPort (ports/outbound)          ← outbound contract
    │      │
    │  SolidityContractReader (adapters/outbound)   ← implements ContractReaderPort
    │
    └─ FileSystemReader (adapters/outbound)         ← raw I/O boundary, injected into SolidityContractReader and ReadUseCase
```

### Inbound adapter: `adapters/inbound/reader.rs`

Implements `ReaderPort`. Holds `Box<dyn ReaderRunPort>`. Delegates all method calls to the use case: contains no logic of its own.

### Inbound port: `ports/inbound/reader_run_port.rs`

```rust
pub trait ReaderRunPort: Send + Sync {
    async fn get_contract_context(&self, path: &str, include_comments: bool) -> Result<ContractContext>;
    async fn get_fuzz_output(&self, path: &str) -> Result<Option<String>>;
    async fn get_coverage_context(&self, lcov_path: &str) -> Result<Option<CoverageContext>>;
    async fn get_existing_bodies(&self, path: &str) -> Result<Option<BodiesJson>>;
    async fn get_existing_config(&self, path: &str) -> Result<Option<FuzzerConfigArtifact>>;
}
```

### Use case: `use_cases/read.rs`

`ReadUseCase` implements `ReaderRunPort`. Owns all outbound dependencies:

```rust
pub struct ReadUseCase {
    contract_reader: Arc<dyn ContractReaderPort>,
    fs_reader: Arc<FileSystemReader>,
}
```

`FileSystemReader` is the single I/O boundary: the only struct allowed to call `tokio::fs`. It takes a `PathBuf` base path. Both adapters and the use case receive it via `Arc`.

---

## What the orchestrator asks for

### 1. `get_contract_context(path)` → raw Solidity source

Reads the target contract and strips single-line comments (`//`), block comments (`/* */`), and inline comments. The Generator receives the clean contract body as raw source.

```
src/Vault.sol
  ↓ strip comments
ContractContext { source_code: "contract Vault { ... }" }
```

### 2. `get_existing_bodies(path)` and `get_existing_config(path)` → previous-round artifacts

On rounds 2+ the orchestrator asks for the last round's `BodiesJson` and `FuzzerConfigArtifact` from `.fuzzming/{Contract}/{Contract}.bodies.json` and `.fuzzming/{Contract}/{Contract}.config.json`. Both return `None` on round 1 (files do not exist yet). When the LLM returns a `Patch` response, the executor uses these as the base to apply diff operations. When it returns a `Full` response, they are ignored.

### 3. `get_coverage_context(path)` → uncovered locations with source snippets

Reads a pre-serialised `CoverageContext` JSON artifact written by the fuzzer at the end of the previous round. The path is `.fuzzming/{Contract}/coverage_context.json`. Returns `None` if the file does not exist yet (first round).

**Why a JSON artifact instead of parsing lcov every round?**

The fuzzer enriches each coverage gap with nearby source lines at the point of capture. Storing the enriched `CoverageContext` as JSON means the reader just deserialises it: no extra source file reads are needed in the read path, and the gap-to-source mapping is stable even if the source file later changes.

**How the artifact is built (fuzzer write path):**

1. The fuzzer reads the per-contract `lcov.info` it just wrote.
2. Parses it with `parse_lcov` into `CoverageGap` records:

| LCOV record | What it means | Kept if |
|-------------|--------------|---------|
| `SF:src/Vault.sol` | start of a file block | always: sets current file |
| `DA:42,0` | line 42 hit 0 times | hits == 0 |
| `BRDA:55,0,1,0` | branch on line 55 never taken | hits == 0 or `-` |
| `FNDA:0,withdraw` | function withdraw never called | hits == 0 |
| `end_of_record` | end of file block | resets current file |

3. For each gap, opens the source file and attaches one line before and one line after the gap line (best-effort):

```
"41:     if (amount == 0) {",
"42:         revert ZeroAmount();",    ← gap
"43:     }",
```

4. Serialises the enriched `CoverageContext` to `.fuzzming/{Contract}/coverage_context.json`.

---

## Data flow

```
Orchestrator
  │
  └─ Reader::get_*(...)                    ← ReaderPort (inbound adapter)
       │
       └─ ReadUseCase::get_*(...)          ← ReaderRunPort (use case)
             │
             ├─ get_contract_context(path)
             │     SolidityContractReader reads via FileSystemReader
             │     strips comments → ContractContext
             │
             ├─ get_existing_bodies(path)
             │     FileSystemReader reads .fuzzming/{Contract}/{Contract}.bodies.json
             │     → None (round 1) or BodiesJson (round N)
             │
             ├─ get_existing_config(path)
             │     FileSystemReader reads .fuzzming/{Contract}/{Contract}.config.json
             │     → None (round 1) or FuzzerConfigArtifact (round N)
             │
             └─ get_coverage_context(path)
                   FileSystemReader reads .fuzzming/{Contract}/coverage_context.json
                   deserialises pre-enriched CoverageContext
                   → None (first round) or CoverageContext { gaps, line_found, line_hit, ... }
```

---

## Wiring at startup

```rust
let fs_reader       = Arc::new(FileSystemReader::new(workspace_root)); // PathBuf
let contract_reader = Arc::new(SolidityContractReader::new(Arc::clone(&fs_reader)));
let use_case        = Box::new(ReadUseCase::new(contract_reader, Arc::clone(&fs_reader)));
let reader          = Reader::new(use_case);
```

Coverage context is loaded from a pre-serialised JSON artifact written by the fuzzer at the end of each passing round. `ReadUseCase` reads `.fuzzming/{Contract}/coverage_context.json` directly via `FileSystemReader`.

`Reader` never imports `ReadUseCase`. All concrete types are resolved at the entry point only.

---

## Known limitations

### 1. `FNDA` has no line number: context is wrong

The LCOV `FNDA` record carries only hits and function name, no line number. The parser stores `line: 0` for every uncovered function. The enrichment attaches lines 1–4 of the file instead of the actual function body.

**Fix needed:** use the `FN` record (which carries a line number) to look up the line for each function name before enriching.

### 2. Duplicate gaps for multi-branch lines

A line with multiple branches produces one `CoverageGap` per branch. The Generator may see the same source context twice.

**Fix needed:** deduplicate gaps by `(file, line)` after parsing.

### 3. Absolute `SF:` paths

The `SF:` path is used as-is to open the source file. If forge writes an absolute path, the reader now handles it correctly by falling back to the absolute path when the workspace-relative path doesn't resolve. If the path is neither relative nor absolute-readable, `source_context` stays empty.

### 4. Does not generalise beyond Solidity + Foundry

`SolidityContractReader` hard-codes Solidity comment stripping. A different language would need a different adapter implementing `ContractReaderPort`.
