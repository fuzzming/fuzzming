# Reader

The Reader is the **read gateway** of FuzzMing. Before every LLM round, the orchestrator asks the Reader for three things: the contract source code, the coverage gaps, and the forge fuzz output. The Reader collects all of that and hands it to the LLM so it can write better invariants.

---

## The big picture

```
forge coverage  ──► lcov.info           ─┐
forge test      ──► fuzz_output.json    ─┤──► Reader ──► LLM
src/Vault.sol   ──► .sol file           ─┘
```

The Reader never writes anything. It only reads and transforms.

---

## Directory structure

```
src/reader/
├── reader.rs                          # Main orchestrator — wires everything together
├── infrastructure/
│   └── file_system_reader.rs          # Only place that calls tokio::fs
├── ports/
│   ├── contract_reader_port.rs        # Trait: read a .sol file
│   └── coverage_reader_port.rs        # Trait: read an LCOV file
├── adapters/
│   ├── solidity_contract_reader.rs    # Reads .sol files, strips pragma/imports
│   └── foundry_coverage_reader.rs     # Parses LCOV + attaches source lines to each gap
└── use_cases/
    └── parse_lcov.rs                  # Pure function: LCOV text → list of CoverageGap
```

---

## How the Reader is structured

`Reader` holds three things injected via DI:

```rust
pub struct Reader {
    contract_reader: Arc<dyn ContractReaderPort>,   // reads .sol files
    coverage_reader: Arc<dyn CoverageReaderPort>,   // reads lcov.info
    fs_reader:       Arc<FileSystemReader>,          // reads any file raw
    invariant_files: InvariantFiles,                 // all file paths in one place
}
```

`FileSystemReader` is the single I/O boundary — it is the only struct allowed to call `tokio::fs`. Both adapters and the `Reader` itself receive it via `Arc`.

`InvariantFiles` carries all the paths:

```rust
pub struct InvariantFiles {
    pub invariant_file_path: String,   // generated invariant .sol
    pub foundry_toml_path:   String,   // foundry.toml
    pub lcov_path:           String,   // lcov.info
    pub fuzz_output_path:    String,   // forge test --json output
}
```

---

## What the LLM asks for — and what happens

### 1. `get_contract_context(path)` → raw Solidity source

Reads the target contract and strips `pragma` and `import` lines. The LLM receives the full contract body as raw source — no regex extraction, no summarising.

```
src/Vault.sol
  ↓ strip pragma + import
ContractContext { source_code: "contract Vault { ... }" }
```

### 2. `get_fuzz_output()` → raw forge JSON or nothing

Reads the JSON file produced by `forge test --json` and passes it raw to the LLM. Returns `None` if the file does not exist yet (first round).

The LLM reads the JSON directly — failed test names, reasons, counterexamples — and decides what to fix.

### 3. `get_coverage_context()` → uncovered locations with source snippets

Reads `lcov.info` and returns every line, branch, and function that was never executed. Returns `None` if the file does not exist yet.

**How `parse_lcov` works:**

LCOV is a plain text format. The parser walks it line by line and records every entry with 0 hits as a `CoverageGap`:

| LCOV record | What it means | Kept if |
|-------------|--------------|---------|
| `SF:src/Vault.sol` | start of a file block | always — sets current file |
| `DA:42,0` | line 42 hit 0 times | hits == 0 |
| `BRDA:55,0,1,0` | branch on line 55 never taken | hits == 0 or `-` |
| `FNDA:0,withdraw` | function withdraw never called | hits == 0 |
| `end_of_record` | end of file block | resets current file |

**How `FoundryCoverageReader` enriches gaps:**

For each gap it opens the source file and attaches the 3 lines before and 3 lines after the gap line so the LLM has context:

```
"40:     uint256 shares = ...",
"41:     if (amount == 0) {",
"42:         revert ZeroAmount();",    ← gap
"43:     }",
"44:     token.transfer(msg.sender, amount);",
```

---

## Known limitations

### 1. `FNDA` has no line number — context is wrong

The LCOV `FNDA` record carries only hits and function name, no line number:
```
FNDA:0,withdraw
```
The parser stores `line: 0` for every uncovered function. The enrichment step then attaches lines 1–4 of the file instead of the actual function body. The LLM receives wrong source context for uncovered functions.

**Fix needed:** use the `FN` record (which does carry a line number) to look up the line for each function name before enriching.

### 2. Duplicate gaps for multi-branch lines

A line with multiple branches produces one `CoverageGap` per branch. If line 55 has 2 branches and one is never taken, the LLM sees the same source context twice.

**Fix needed:** deduplicate gaps by `(file, line)` after parsing, or merge branch gaps into a single entry.

### 3. Only handles Solidity paths from `SF:`

The `SF:` path is used as-is to open the source file for context enrichment. If forge writes an absolute path or a path relative to a different root than `FileSystemReader.base_path`, the enrichment silently fails and `source_context` stays empty. The gap is still reported but without context.

### 4. Does not generalise beyond Solidity + Foundry

The LCOV parser is generic, but `SolidityContractReader` hard-codes Solidity-specific stripping (`pragma`, `import`, comment removal). A different language would need a different adapter.

---

## Data flow for one full round

```
Orchestrator
  │
  ├─ reader.get_contract_context("src/Vault.sol")
  │    └─ SolidityContractReader uses FileSystemReader to read the file
  │    └─ strips pragma + imports
  │    └─ returns ContractContext { source_code }
  │
  ├─ reader.get_fuzz_output()
  │    └─ FileSystemReader opens fuzz_output.json
  │    └─ returns None if missing, Some(raw_json) if present
  │
  └─ reader.get_coverage_context()
       └─ FoundryCoverageReader uses FileSystemReader to open lcov.info
       └─ returns None if missing
       └─ parse_lcov() finds all 0-hit DA / BRDA / FNDA records
       └─ for each gap, FileSystemReader opens the source file
       └─ attaches 3 lines before + 3 lines after the gap
       └─ returns CoverageContext { gaps }
```

---

## Design decisions

**Raw source over parsed metadata** — the contract is sent as raw Solidity. A regex-extracted summary misses custom types, structs, events, and sized integers. The LLM reads raw Solidity accurately.

**Raw JSON for fuzz output** — forge JSON is passed directly to the LLM instead of being parsed into a summary. Less code, no information lost, the LLM handles JSON well.

**`FileSystemReader` as single I/O boundary** — mirrors the executor's `FileSystemWriter`. All `tokio::fs` calls go through one struct. Adapters receive it via `Arc` injection, making them easy to test by swapping the reader.

**`Option` instead of sentinel strings** — `read_file_optional` returns `None` for missing files. No string matching, no silent bugs if a message changes.
