# FuzzMing — End-to-End Testing Report

Three buggy smart contracts were written at increasing difficulty levels to validate that FuzzMing can autonomously generate invariant tests, run them, and find real bugs. This document records what each contract does, what bug it contains, how FuzzMing performed, and what problems were discovered and fixed along the way.

---

## Target Contracts

### EasyBank — Easy

```solidity
function withdraw(uint256 amount) external {
    require(balanceOf[msg.sender] >= amount, "insufficient balance");
    balanceOf[msg.sender] -= amount;
    unchecked {
        totalDeposits -= amount + 1;   // bug: subtracts one extra per withdrawal
    }
    token.transfer(msg.sender, amount);
}
```

**Bug**: `totalDeposits` is decremented by `amount + 1` instead of `amount` inside an `unchecked` block. After every withdrawal, `totalDeposits` drifts lower than the real sum of balances by 1. After enough withdrawals it wraps to near `type(uint256).max`.

**Minimum trigger sequence**: deposit → withdraw (1 step after setup).

---

### MediumVault — Medium

```solidity
function withdraw(uint256 shares) external returns (uint256 amount) {
    amount = (shares * totalAssets) / totalShares;
    uint256 fee = (amount * FEE_BPS) / 10_000;
    sharesOf[msg.sender] -= shares;
    totalShares -= shares;
    totalAssets -= (amount - fee);   // bug: should be totalAssets -= amount
    token.transfer(feeCollector, fee);
    token.transfer(msg.sender, amount - fee);
}
```

**Bug**: The fee portion is sent to `feeCollector` but only `amount - fee` is subtracted from `totalAssets`. The fee tokens physically leave the vault but `totalAssets` keeps counting them. After any withdrawal `totalAssets > token.balanceOf(vault)` — the vault is insolvent on paper.

**Minimum trigger sequence**: deposit → withdraw.

---

### HardStaking — Hard

```solidity
function stake(uint256 amount) external {
    stakingToken.transferFrom(msg.sender, address(this), amount);
    stakedOf[msg.sender] += amount;
    totalStaked += amount;
    // bug: missing _updateReward(msg.sender) call
}
```

**Bug**: `stake()` does not call `_updateReward(msg.sender)` before increasing `stakedOf`. This leaves `userRewardPerTokenPaid[msg.sender]` at 0. When rewards were already added before this user staked, `earned()` returns a large positive value — the new staker retroactively receives rewards from before they joined.

The consequence: `totalRewardsClaimed` can exceed `totalRewardsAdded`, breaking conservation.

**Minimum trigger sequence**: actor A stakes → addRewards → actor B stakes → both claim (3 steps, 2 actors).

---

## Run History

### Run 1 — All three contracts: CompileError every round

**Outcome**: 0 bugs found across all three contracts.

**What happened**: The LLM placed an em dash character (`—`) inside a Solidity string literal:
```solidity
"totalDeposits exceeds ghost_total_deposited — unchecked underflow wrap detected"
```
Solidity rejects non-ASCII characters in plain string literals (error 8936). Because Foundry compiles the entire project together, one broken file in `EasyBank` prevented `MediumVault` and `HardStaking` from running too. All three contracts exhausted their 3-round budget without ever fuzzing.

**Fix applied**:
- Prompt rule added: all string literals must use plain ASCII only — no Unicode dashes, smart quotes, or non-ASCII characters.
- The LLM was also importing `Token` from the target contract file (`import {Token} from "src/EasyBank.sol"`) instead of its own source file. Fixed by adding `extract_dependency_imports()` — FuzzMing now parses the target contract's imports, resolves their paths, and sends the exact import lines to the LLM.

---

### Run 2 — EasyBank and HardStaking find bugs; MediumVault terminates immediately

| Contract | Result | Bugs found |
|---|---|---|
| EasyBank | Exhausted | 12 |
| MediumVault | DevTestFailed (terminated) | 0 |
| HardStaking | Exhausted | 3 |

**EasyBank** performed well — 12 invariant failures across 3 rounds, all pointing to the off-by-one. The report said "no bugs found" despite the logs showing 12. Fixed: `SessionOutcome` was missing a `bugs` field so the Exhausted formatter always printed "no bugs found". Added `bugs: Vec<BugInfo>` to `SessionOutcome` and updated the formatter.

**MediumVault** terminated in round 1 with `DevTestFailed`. The LLM generated an unused local variable:
```solidity
uint256 netDeposited = handler.ghost_net_deposited(); // declared but never read
```
Solidity treats unused variables as errors. The system immediately killed the session and never gave the LLM a chance to repair it. Two fixes applied:
- `DevTestFailed` is now retryable — behaves the same as `CompileError`, continues while rounds remain.
- The error output was also empty. `filter_output` only captures the `InvariantTest` section of forge stdout; unused-variable errors appear before that section, so `fuzz_output.txt` was blank and the LLM had no context to fix. Fixed: when `DevTestFailed` produces empty filtered output, the fuzzer falls back to writing the full `stderr + stdout`.
- Prompt rule added: never declare a local variable that is not used in the function body.

**HardStaking** found 3 bugs but they were the wrong bug. The LLM dealt tokens without an upper bound, allowing stake amounts near `type(uint256).max`. Inside `earned()`, the multiplication `stakedOf[account] * rewardPerTokenStored` overflowed and panicked. The fuzzer found the overflow before ever reaching the retroactive-rewards scenario. Fix applied: prompt rule added to cap all dealt amounts at `type(uint128).max`.

---

### Run 3 — All three contracts find bugs

| Contract | Rounds | Bugs found | Bug correct |
|---|---|---|---|
| EasyBank | 3 | 10 | Yes |
| MediumVault | 3 | 10 | Yes |
| HardStaking | 3 | 1 | Partially |

**EasyBank** found 10 bugs across 3 rounds. The off-by-one in `totalDeposits` was caught by multiple invariants — ghost accounting, sum of balances, underflow wrap detection — all with minimal 2-step sequences. Confirmed correct.

**MediumVault** found 10 bugs across 3 rounds. The fee accounting bug was correctly identified: `token.balanceOf(vault) != vault.totalAssets()` after any withdrawal. HardStaking had `DevTestFailed` in rounds 1 and 2 — the LLM received the full error output, repaired the test in round 3, and found 1 bug.

**HardStaking** found 1 bug in round 3 (`invariant_earnedNeverExceedsAvailableRewards`). The call sequence still shows very large stake amounts, so this may still be the overflow path rather than the intended retroactive-rewards scenario. Next run should confirm with bounded inputs.

---

## Summary

| Contract | Bug type | First run | After fixes | Correct bug |
|---|---|---|---|---|
| EasyBank | Arithmetic off-by-one | CompileError (em dash) | 10 bugs found | Yes |
| MediumVault | Fee accounting drift | CompileError (em dash) | 10 bugs found | Yes |
| HardStaking | Missing reward snapshot | CompileError (em dash) | 1 bug found | Unclear — large-amount path still present |

---

## Analysis — Bug Detection Performance

### EasyBank

- Signal strength: high. The bug shows up with a 2-step sequence and is visible to multiple invariants.
- Robustness: strong. Even with early infrastructure errors, once compilation succeeded the fuzzer consistently found the off-by-one across rounds.

### MediumVault

- Signal strength: high once the test suite could run. The bug is a direct accounting mismatch that invariants catch quickly.
- Sensitivity to tooling: medium. A single unused variable caused DevTestFailed and terminated the run before fixes; after making DevTestFailed retryable and surfacing full error output, the bug was consistently detected (10 bugs in the latest run).

### HardStaking

- Signal strength: moderate to low. The correct bug requires a multi-actor, multi-step sequence with timing sensitivity.
- Failure mode: competing large-amount paths still dominate; the latest run shows extremely large stake inputs in the failing sequence.
- Status: partial. The invariant that failed matches the intended property, but the call sequence suggests the overflow path may still be active.

### Overall

- FuzzMing reliably detects simple and medium accounting bugs once compilation and error-reporting issues are resolved.
- The hardest class (multi-actor reward accounting) needs tighter input bounds and more targeted action sequencing to avoid unrelated overflow paths.
- The fixes applied primarily improved run stability and observability, which directly increased bug discovery rates.

### Proposed Solution (Generator)

- For order-dependent bugs, the generator should emit a scripted handler action that deterministically enforces the critical sequence at least once per round (e.g., stake A -> addRewards -> stake B), plus a matching invariant that asserts the retroactive-reward condition.

---

## Issues Found and Fixed

| Issue | Root cause | Fix |
|---|---|---|
| All contracts CompileError | Em dash in string literal | Prompt rule: ASCII only in strings |
| Wrong Token import path | LLM guesses dependency paths | `extract_dependency_imports()` sends resolved paths to LLM |
| DevTestFailed immediately terminal | `check_termination` returned `terminate: true` unconditionally | DevTestFailed now retries like CompileError |
| DevTestFailed gave LLM empty error | `filter_output` only captures section inside `InvariantTest` header | Fallback to full stderr+stdout when filtered output is empty |
| Exhausted report showed "no bugs found" | `SessionOutcome` had no `bugs` field | Added `bugs: Vec<BugInfo>` to `SessionOutcome` |
| Overflow masking retroactive rewards bug | Stake amounts near `type(uint256).max` overflow `earned()` | Prompt rule: bound all amounts to `type(uint128).max` |
| LLM declares unused variables | No rule against it | Prompt rule: no unused local variable declarations |

---

## How to Run the Tests Manually

```bash
# Verify all three contracts compile
FOUNDRY_PROFILE=fuzzming forge build --root tests/fixtures/foundry_vault

# Run a specific contract's invariant tests
FOUNDRY_PROFILE=fuzzming forge test --root tests/fixtures/foundry_vault --match-contract EasyBankInvariantTest -v
FOUNDRY_PROFILE=fuzzming forge test --root tests/fixtures/foundry_vault --match-contract MediumVaultInvariantTest -v
FOUNDRY_PROFILE=fuzzming forge test --root tests/fixtures/foundry_vault --match-contract HardStakingInvariantTest -v

# Run fuzzming against all three
cargo run -- \
  --targets tests/fixtures/foundry_vault/src/EasyBank.sol \
            tests/fixtures/foundry_vault/src/MediumVault.sol \
            tests/fixtures/foundry_vault/src/HardStaking.sol \
  --max-rounds 3 \
  --model <model> \
  --llm-key <key> \
  --workspace-root tests/fixtures/foundry_vault
```
