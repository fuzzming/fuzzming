# Reader Component

The Reader is the **read gateway** of FuzzMing. Before every LLM round, the orchestrator asks the Reader for three things: the contract source code, the coverage gaps, and the forge fuzz output. The Reader collects all of that and hands it back so the orchestrator can assemble the `RoundSignal`.

---

## The big picture

```
forge coverage  ──► lcov.info           ─┐
forge test      ──► fuzz_output.json    ─┤──► Reader ──► Orchestrator ──► Generator
src/Vault.sol   ──► .sol file           ─┘
```

The Reader never writes anything. It only reads and transforms.

---

## Directory structure

```
src/reader/
├── adapters/
│   ├── inbound/
│   │   └── reader.rs                      # Inbound adapter — implements ReaderPort, delegates to ReaderRunPort
│   └── outbound/
│       ├── file_system_reader.rs          # FileSystemReader — only place that calls tokio::fs::read
│       ├── solidity_contract_reader.rs    # Implements ContractReaderPort — reads .sol, strips pragma/imports
│       └── foundry_coverage_reader.rs     # Implements CoverageReaderPort — parses LCOV + attaches source lines
├── ports/
│   ├── inbound/
│   │   └── reader_run_port.rs             # ReaderRunPort — inbound contract between adapter and use case
│   └── outbound/
│       ├── contract_reader_port.rs        # ContractReaderPort — read a .sol file
│       └── coverage_reader_port.rs        # CoverageReaderPort — read an LCOV file
└── use_cases/
    ├── read.rs                            # ReadUseCase — owns outbound ports, implements ReaderRunPort
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
    ├─ CoverageReaderPort (ports/outbound)          ← outbound contract
    │      │
    │  FoundryCoverageReader (adapters/outbound)    ← implements CoverageReaderPort
    │
    └─ FileSystemReader (adapters/outbound)         ← raw I/O boundary, injected into outbound adapters
```

### Inbound adapter — `adapters/inbound/reader.rs`

Implements `ReaderPort`. Holds `Box<dyn ReaderRunPort>`. Delegates all method calls to the use case — contains no logic of its own.

### Inbound port — `ports/inbound/reader_run_port.rs`

```rust
pub trait ReaderRunPort: Send + Sync {
    async fn get_contract_context(&self, path: &str, include_comments: bool) -> Result<ContractContext>;
    async fn get_fuzz_output(&self) -> Result<Option<String>>;
    async fn get_coverage_context(&self) -> Result<Option<CoverageContext>>;
    async fn get_invariant_files(&self) -> Result<InvariantFiles>;
}
```

### Use case — `use_cases/read.rs`

`ReadUseCase` implements `ReaderRunPort`. Owns all outbound dependencies:

```rust
pub struct ReadUseCase {
    contract_reader: Arc<dyn ContractReaderPort>,
    coverage_reader: Arc<dyn CoverageReaderPort>,
    fs_reader: Arc<FileSystemReader>,
    invariant_files: InvariantFiles,
}
```

`FileSystemReader` is the single I/O boundary — the only struct allowed to call `tokio::fs`. Both adapters and the use case receive it via `Arc`.

`InvariantFiles` carries all the paths:

```rust
pub struct InvariantFiles {
    pub invariant_file_path: String,
    pub foundry_toml_path:   String,
    pub lcov_path:           String,
    pub fuzz_output_path:    String,
}
```

---

## What the orchestrator asks for

### 1. `get_contract_context(path)` → raw Solidity source

Reads the target contract and strips `pragma` and `import` lines. The Generator receives the full contract body as raw source — no regex extraction, no summarising.

```
src/Vault.sol
  ↓ strip pragma + import
ContractContext { source_code: "contract Vault { ... }" }
```

### 2. `get_fuzz_output()` → raw forge JSON or nothing

Reads the JSON file produced by `forge test --json` and passes it raw. Returns `None` if the file does not exist yet (first round).

### 3. `get_coverage_context()` → uncovered locations with source snippets

Reads `lcov.info` and returns every line, branch, and function that was never executed. Returns `None` if the file does not exist yet.

**How `parse_lcov` works:**

| LCOV record | What it means | Kept if |
|-------------|--------------|---------|
| `SF:src/Vault.sol` | start of a file block | always — sets current file |
| `DA:42,0` | line 42 hit 0 times | hits == 0 |
| `BRDA:55,0,1,0` | branch on line 55 never taken | hits == 0 or `-` |
| `FNDA:0,withdraw` | function withdraw never called | hits == 0 |
| `end_of_record` | end of file block | resets current file |

**How `FoundryCoverageReader` enriches gaps:**

For each gap it opens the source file and attaches the 3 lines before and 3 lines after the gap line:

```
"40:     uint256 shares = ...",
"41:     if (amount == 0) {",
"42:         revert ZeroAmount();",    ← gap
"43:     }",
"44:     token.transfer(msg.sender, amount);",
```

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
             │     strips pragma + imports → ContractContext
             │
             ├─ get_fuzz_output()
             │     FileSystemReader opens fuzz_output.json
             │     → None if missing, Some(raw_json) if present
             │
             └─ get_coverage_context()
                   FoundryCoverageReader reads lcov.info via FileSystemReader
                   parse_lcov() finds all 0-hit records
                   enriches each gap with ±3 lines of source
                   → CoverageContext { gaps }
```

---

## Wiring at startup

```rust
let fs_reader       = Arc::new(FileSystemReader::new(base_path));
let contract_reader = Arc::new(SolidityContractReader::new(Arc::clone(&fs_reader)));
let coverage_reader = Arc::new(FoundryCoverageReader::new(Arc::clone(&fs_reader)));
let use_case        = Box::new(ReadUseCase::new(contract_reader, coverage_reader, fs_reader, invariant_files));
let reader          = Reader::new(use_case);
```

`Reader` never imports `ReadUseCase`. All concrete types are resolved at the entry point only.

---

## Known limitations

### 1. `FNDA` has no line number — context is wrong

The LCOV `FNDA` record carries only hits and function name, no line number. The parser stores `line: 0` for every uncovered function. The enrichment attaches lines 1–4 of the file instead of the actual function body.

**Fix needed:** use the `FN` record (which carries a line number) to look up the line for each function name before enriching.

### 2. Duplicate gaps for multi-branch lines

A line with multiple branches produces one `CoverageGap` per branch. The Generator may see the same source context twice.

**Fix needed:** deduplicate gaps by `(file, line)` after parsing.

### 3. Only handles Solidity paths from `SF:`

The `SF:` path is used as-is to open the source file. If forge writes an absolute path or a path relative to a different root, the enrichment silently fails and `source_context` stays empty.

### 4. Does not generalise beyond Solidity + Foundry

`SolidityContractReader` hard-codes Solidity-specific stripping. A different language would need a different adapter implementing `ContractReaderPort`.
