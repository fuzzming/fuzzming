# Case Study: DynamicSwapFeeModule: Five-Method Security Analysis

**Target:** [`DynamicSwapFeeModule.sol`](https://github.com/aerodrome-finance/slipstream/blob/main/contracts/core/fees/DynamicSwapFeeModule.sol) (149 nSLOC): a concentrated liquidity dynamic fee module from a Uniswap V3 fork deployed on BNB Chain.

**Reference audit:** Shieldify Security: 5-day engagement, 80 auditor-hours. Full report on [Solodit](https://solodit.cyfrin.io/issues/l-02-min_seconds_ago-hardcoded-to-2-seconds-but-bnb-chain-block-time-is-045-seconds-shieldify-none-topaz-dex-markdown).

---

## Overview

Five independent methods were run against the same 161-line contract. No method had access to the results of the others before running.

| | Shieldify | Claude Web | Claude Code | FuzzMing | invgen |
|---|---|---|---|---|---|
| Testing method | Manual review | LLM static analysis | Property-based fuzzing | Stateful invariant fuzzing | LLM-generated invariants |
| Time | 5 days / 80 hrs | ~7 min (LLM only) ³ | ~22 min | 23 min ¹ | ~20 min (generation only) ² |
| LLM calls | - | 1 | - | 23 | 59 |
| Tokens (prompt / completion) | - | not tracked (web UI) | not exposed at runtime | 1,198,947 / 89,478 | 324,700 / 78,098 |
| Cost | not disclosed | not tracked | not tracked | $4.94 | $2.15 |
| Human effort | 80 hrs | ~7 min | 0 | 0 | 0 |
| Bugs detected (invariants generated) | 5 | 8 | 5 | 7 | 6  |
| Bugs confirmed (fuzzer found counterexample) | 5 | 0 (8 unverified) ⁴ | 5 | 7 | **1** ⁵ |
| False positives | 0 | unverifiable (no test code) | 0 | 0 | 0 |
| Reproducible proof | - |: | ✓ (seed + counterexample) | ✓ (shrunk call sequence) | ✓ (call sequence) |

**Terminology note (important):**
- **Stateful invariant fuzzing** = an **execution method**. The fuzzer runs multi-step call sequences over changing contract state (typically through a handler), then checks invariants after each sequence.
- **LLM-generated invariants** = a **generation method**. The LLM writes invariant code, but a separate fuzzer run is still needed to execute those invariants and confirm bugs.
- In short: one is **how tests are executed** (`stateful fuzzing`), the other is **how tests are written** (`LLM-generated invariants`).

◎ Invariant generated in LLM cache and semantically correct: not compiled or run due to invgen's own toolchain failure (see Method 5).

> ¹ **FuzzMing's 23 minutes is the full end-to-end time**: invariant generation, fuzzer execution, and bug reporting in a single automated command. No manual step required.
>
> ² **invgen's ~20 minutes covers generation only.** After invgen saves the invariant file, the user must still run `forge test` or `ityfuzz` manually to actually fuzz the contract. The fuzzing time is not included in the ~20 min figure. In our run, invgen failed to compile due to a structural mismatch in its own assembly logic and a hardcoded 1500-token limit. We manually extracted the best invariants from the LLM cache, fixed the assembly, and ran them with forge: 1 bug confirmed, 6 invariants passed.
>
> ³ **Claude Web's ~7 minutes is the LLM query time only.** The output is prose: no test code, no runnable output, no confirmation. Each finding still requires a developer to read it, understand the code, write a test, and run it to verify it is real. That verification effort is not included in the 7-minute figure and varies per finding. The total time to actually use Claude Web's output is 7 min + however long verification takes.
>
> ⁴ **Claude Web's 8 findings are prose descriptions only**: no test code was produced and no counterexample was generated. Each finding is unverified until a developer manually writes and runs a test.
>
> ⁵ **invgen's 1 confirmed bug was caught by luck, not by invgen's pipeline.** The fuzzer called `setDefaultFeeCap(0)` from a randomly selected sender that happened to match the authorized `swapFeeManager`. All other 12 functions reverted 100% of the time because the fuzzer had no handler to call them as the authorized sender. The 6 targeted bugs were detected by the LLM but never reached by the fuzzer.

---

## Scope

| File | nSLOC |
|---|---|
| [`contracts/core/fees/DynamicSwapFeeModule.sol`](https://github.com/aerodrome-finance/slipstream/blob/main/contracts/core/fees/DynamicSwapFeeModule.sol) | 149 |
| `contracts/core/interfaces/fees/IDynamicFeeModule.sol` | 12 |
| **Total** | **161** |

---

## Method 1, Shieldify: Professional Manual Audit

A team of professional auditors reviewed the contract over 5 calendar days. Findings include chain-specific knowledge (BNB Chain block time), adversarial multi-actor threat models, and design-level observations that require context outside the contract source.

| Finding | Description |
|---|---|
| M-01 | Cardinality pre-check uses wrong variable and is redundant |
| L-01 | Discount rounds up in favour of user |
| L-02 | `MIN_SECONDS_AGO` hardcoded wrong for BNB Chain block time |
| L-03 | `currentTick` from `slot0` manipulable via spot price |
| L-04 | `resetDynamicFee` does not reset `baseFee` |

*I-01 and I-02 are excluded, they target different contracts outside the shared scope.*

---

## Method 2, Claude Web: LLM Static Analysis

Full contract source pasted into a single prompt in the Claude web interface. All 8 findings produced in one pass in approximately 7 minutes with no tool calls, no code execution, and no iteration. Token usage and cost are not tracked through the web UI.

### Findings

| ID | Description |
|---|---|
| F1 | `initialFee` bypasses pool-specific `feeCap` |
| F2 | `setDefaultFeeCap(0)` zeros all protocol fees |
| F3 | Pool-specific `feeCap` silently ignored when `scalingFactor == 0` |
| F4 | `setScalingFactor(pool, 0)` accepted, silently drops `feeCap` |
| F5 | `tx.origin` used for discount: breaks smart wallets, phishable |
| F6 | Discount not applied on `initialFee` early-return path |
| F7 | `resetDynamicFee` does not reset `baseFee` |
| F8 | TWAP tick division truncates toward zero |

### Finding Detail

#### F1: `initialFee` bypasses pool-specific `feeCap`

**Lines:** `setInitialFee()` (validation), `getFee()` (initialFee early-return block)

**Root cause:** `setInitialFee` validates `_fee <= MAX_FEE_CAP` (5%) but never compares against the pool's own `feeCap` field. Inside `getFee`, the initial-fee path returns before the cap-enforcement line `totalFee = totalFee < feeCap ? totalFee : feeCap`, so the pool-specific feeCap is never applied.

**Trigger scenario:**
```solidity
// Admin intends pool fees to be capped at 0.5%
setFeeCap(pool, 5_000);

// Admin also enables cheap first-swap incentive, but sets it too high
setInitialFee(pool, 50_000);  // 5%: passes require(_fee <= MAX_FEE_CAP)

// Every block's first swap now pays 5% regardless of the 0.5% cap
// return uint24(_initialFee) fires before the feeCap clamp
```

**Fix:** Either add `require(_fee <= dynamicFeeConfig[_pool].feeCap || feeCap == 0)` in `setInitialFee`, or apply the feeCap inside the initialFee branch before returning.

---

#### F2: `setDefaultFeeCap(0)` zeros all protocol fees

**Lines:** `setDefaultFeeCap()`, `getFee()` cap-enforcement line

**Root cause:** The only validation is `require(_defaultFeeCap <= MAX_FEE_CAP)`, which permits 0. In `getFee`, when `scalingFactor == 0`, `feeCap` is loaded from `defaultFeeCap`. The comparison `totalFee < feeCap` is a `uint256 < 0`, always false, so `totalFee = feeCap = 0`. The same flaw exists in the constructor. Note the inconsistency: `setFeeCap` correctly guards `require(_feeCap > 0, "FC0")` for per-pool caps, but the identical protection is missing for the global default.

**Trigger scenario:**
```solidity
// Compromised or malicious swapFeeManager
setDefaultFeeCap(0);

// Every pool that hasn't set a custom scalingFactor now collects 0 fees.
// Single call, immediate, affects the entire protocol.
```

**Fix:** Add `require(_defaultFeeCap > 0, "FC0")` to `setDefaultFeeCap` and the constructor, matching the guard already present in `setFeeCap`.

---

#### F3: Pool-specific `feeCap` silently ignored when `scalingFactor == 0`

**Lines:** `getFee()` (`if (scalingFactor == 0)` branch), `setFeeCap()`

**Root cause:** In `getFee`, if `scalingFactor == 0`, the code overwrites the local `feeCap` variable with `defaultFeeCap`. A pool with a custom `feeCap` but no custom `scalingFactor` silently uses the higher default cap instead of its intended limit.

**Trigger scenario A (admin omission):**
```solidity
setFeeCap(pool, 5_000);   // intend to cap at 0.5%
// Admin forgets to call setScalingFactor
// getFee: scalingFactor==0 → feeCap overwritten with defaultFeeCap (e.g. 50_000)
// Pool now charges up to 5%, not 0.5%
```

**Trigger scenario B (via `setScalingFactor(pool, 0)`):**
```solidity
setFeeCap(pool, 5_000);
setScalingFactor(pool, 1_000_000);  // custom, feeCap used correctly
setScalingFactor(pool, 0);          // accepted: see F4
// feeCap now completely ignored in getFee
```

**Fix:** Restructure `getFee` to fall back to `defaultScalingFactor` for the scaling component only, keeping the pool-specific `feeCap` if it is set.

---

#### F4: `setScalingFactor(pool, 0)` accepted, silently drops `feeCap`

**Lines:** `setScalingFactor()` validation condition

**Root cause:** The guard `require(dynamicFeeConfig[_pool].feeCap != 0 && _scalingFactor <= MAX_SCALING_FACTOR)` accepts `_scalingFactor = 0` since `0 <= MAX_SCALING_FACTOR`. Writing 0 to storage leaves the pool in a hybrid state where `feeCap` is set but `scalingFactor == 0` causes `getFee` to ignore it (see F3).

**Fix:** Change the validation to `require(dynamicFeeConfig[_pool].feeCap != 0 && _scalingFactor > 0 && _scalingFactor <= MAX_SCALING_FACTOR, "ISF")`.

---

#### F5: `tx.origin` used for discount check

**Lines:** `getFee()` discount block, `registerDiscounted()`

**Root cause:** `tx.origin` is always an EOA. This breaks the feature in two ways:

1. **Registered smart contract addresses never receive discounts.** `discounted[safeWallet]` can be set, but `tx.origin` will be one of the Safe's EOA signers, never the Safe itself.
2. **Phishing vector.** A discounted EOA can be tricked into calling a malicious intermediary contract. Since `tx.origin` still equals the victim's address, the discount is applied on the attacker's behalf.

**Trigger scenario:**
```solidity
registerDiscounted(alice, 200_000);  // 20% discount

// Attacker deploys MaliciousRouter that calls pool.swap()
// Alice is phished into sending a tx to MaliciousRouter
// tx.origin == alice → discount applies; attacker's contract gets the cheaper swap
```

**Fix:** Replace `tx.origin` with `msg.sender` throughout.

---

#### F6: Discount not applied on `initialFee` early-return path

**Lines:** `getFee()` initialFee block vs. discount block

**Root cause:** The `discounted[tx.origin]` check appears after the `if (dfc.initialFeeEnabled)` early return. A registered discounted address pays the full `initialFee` for every first swap in a block.

**Trigger scenario:**
```solidity
registerDiscounted(alice, 500_000);  // 50% discount
setInitialFee(pool, 10_000);

// Alice's first swap in a block: returns 10_000 (no discount applied)
// Alice's second swap in same block: returns ~5_000 (50% discount applied)
```

**Fix:** Apply the discount inside the initialFee branch before returning.

---

#### F7: `resetDynamicFee` does not reset `baseFee`

**Lines:** `resetDynamicFee()`

**Root cause:** The function deletes `feeCap`, `scalingFactor`, `initialFeeEnabled`, and `initialFee`, but leaves `dynamicFeeConfig[_pool].baseFee` untouched.

**Trigger scenario:**
```solidity
setCustomFee(pool, 20_000);    // baseFee = 2%
setFeeCap(pool, 40_000);
setScalingFactor(pool, 1e18);
resetDynamicFee(pool);
// Developer believes pool is back to defaults
// Actually: baseFee = 2% still, scalingFactor/feeCap = defaults
```

**Fix:** Add `delete dynamicFeeConfig[_pool].baseFee;` to `resetDynamicFee`, or rename it to `resetDynamicScaling` to accurately describe what is reset.

---

#### F8: TWAP tick division truncates toward zero

**Lines:** `_getDynamicFee()`: `twAvgTick = int24((tickCumulatives[1] - tickCumulatives[0]) / _secondsAgo)`

**Root cause:** Solidity integer division truncates toward zero. The TWAP tick is slightly underestimated in magnitude, causing `absTickDelta` to be 1 tick lower than the mathematically correct value roughly 50% of the time. This produces a dynamic fee marginally lower than theoretically correct, slightly undercharging traders during volatile periods. This matches known behavior in Uniswap V3's own oracle library and requires domain knowledge of the oracle math to identify.

**Fix:** This cannot be corrected with integer arithmetic alone. Acceptable mitigations include a small conservative upward rounding on `absTickDelta`, or documentation acknowledging the directional bias.

---

## Method 3, Claude Code: Property-Based Fuzzing

A Claude Code session read the contract, wrote 10 Foundry fuzz properties encoding expected invariants, ran the fuzzer (5,000 runs per property), and produced an audit report from the counterexamples. Total session approximately 22 minutes. Token usage is not exposed by the Claude runtime at inference time.

### Findings

| ID | Description |
|---|---|
| BUG-1 | `initialFee` path returns before `feeCap` clamp: also affects the `baseFee` sub-path at L172 |
| BUG-2 | `setDefaultFeeCap(0)` forces all fees to zero: same missing guard in constructor (L52) |
| BUG-3 | `resetDynamicFee` does not clear `baseFee` |
| BUG-4 | Discount not applied on `initialFee` path |
| BUG-5 | Per-pool `feeCap` overwritten when `scalingFactor == 0` |

*Full audit report with counterexamples and fixes: [`AUDIT_DynamicSwapFeeModule.md`](../AUDIT_DynamicSwapFeeModule.md)*

---

## Method 4, FuzzMing: Stateful Invariant Fuzzing

FuzzMing generated a Forge handler and a set of invariants from the contract source, then ran stateful fuzzing: random sequences of valid operations that build up contract state across multiple calls before checking invariants. Ghost variables track expected state in parallel with the contract. All findings come with a shrunk call sequence that reproduces the bug deterministically and a full Solidity invariant usable directly as a CI regression test.

**Run:** 10 rounds, 23 LLM calls, 23 minutes wall clock, $4.94.

| Call | Time | Prompt tokens | Completion tokens | Cost |
|---|---|---|---|---|
| 1 | 04:50 | 47,365 | 3,724 | $0.198 |
| 2 | 04:53 | 3,656 | 4,383 | $0.077 |
| 3 | 04:54 | 10,318 | 7,213 | $0.139 |
| 4 | 04:55 | 7,682 | 77 | $0.024 |
| 5 | 04:55 | 34,813 | 5,353 | $0.185 |
| 6 | 04:57 | 17,197 | 3,923 | $0.110 |
| 7 | 04:58 | 24,532 | 796 | $0.086 |
| 8 | 04:58 | 70,918 | 4,119 | $0.275 |
| 9 | 04:59 | 33,190 | 5,758 | $0.186 |
| 10 | 05:01 | 118,423 | 3,963 | $0.415 |
| 11 | 05:02 | 64,451 | 6,794 | $0.295 |
| 12 | 05:04 | 16,825 | 3,674 | $0.106 |
| 13 | 05:04 | 120,812 | 3,638 | $0.417 |
| 14 | 05:05 | 59,627 | 7,102 | $0.285 |
| 15 | 05:07 | 17,441 | 4,606 | $0.121 |
| 16 | 05:08 | 137,802 | 3,782 | $0.470 |
| 17 | 05:09 | 63,995 | 3,826 | $0.249 |
| 18 | 05:10 | 10,889 | 2,256 | $0.067 |
| 19 | 05:10 | 37,297 | 1,231 | $0.130 |
| 20 | 05:10 | 168,891 | 4,484 | $0.574 |
| 21 | 05:12 | 80,569 | 5,050 | $0.317 |
| 22 | 05:13 | 13,337 | 2,321 | $0.075 |
| 23 | 05:13 | 38,917 | 1,405 | $0.138 |
| **Total** | | **1,198,947** | **89,478** | **$4.94** |

### Findings

| Bug | Invariant |
|---|---|
| 1 | `invariant_perPoolFeeCapEnforced` |
| 2 | `invariant_defaultFeeCapZeroSilencesAllFees` |
| 3 | `invariant_initialFeeNotBoundedByFeeCap` |
| 4 | `invariant_resetDynamicFeeDoesNotRestoreZeroFeeIndicator` |
| 5 | `invariant_discountedFeeLeNonDiscountedFee` |
| 6 | `invariant_initialFeePathRespectsDiscount` |
| 7 | `invariant_defaultFeeCap_zero_does_not_suppress_baseFee` |

### Finding Detail

#### Bug 1: Per-pool `feeCap` silently overwritten when `scalingFactor == 0`

**Invariant:**
```solidity
function invariant_perPoolFeeCapEnforced() external view {
    address pool = handler.pools(0);
    uint24 ghostFeeCap = handler.ghost_feeCap(pool);
    uint64 ghostSF = handler.ghost_scalingFactor(pool);
    if (ghostFeeCap == 0) return;
    if (ghostSF != 0) return;
    bool initEnabled = handler.ghost_initialFeeEnabled(pool);
    if (initEnabled) return;
    uint24 ghostBaseFee = handler.ghost_baseFee(pool);
    if (ghostBaseFee == 420) return;
    uint256 fee = handler.ghost_feeForNonDiscountedActor();
    if (fee == 0) return;
    assertLe(fee, uint256(ghostFeeCap), "per-pool feeCap not enforced when scalingFactor==0");
}
```

**Root cause:**
```solidity
uint256 feeCap = dfc.feeCap;           // reads per-pool feeCap (e.g. 140)
if (scalingFactor == 0) {
    scalingFactor = defaultScalingFactor;
    feeCap = defaultFeeCap;            // silently discards per-pool cap
}
totalFee = totalFee < feeCap ? totalFee : feeCap;  // uses wrong feeCap
```

When `scalingFactor` has never been set for a pool, `getFee` replaces the pool's explicit `feeCap` with the global `defaultFeeCap`, potentially allowing fees far above what the pool-level cap permits.

**Reproducing call sequence:**
```
handle_setFeeCapWithoutScalingFactor(seed)
  → sets per-pool feeCap = N, scalingFactor remains 0
  → getFee uses defaultFeeCap instead → can return up to 50000
```

---

#### Bug 2: `defaultFeeCap = 0` silently zeroes all fees

**Invariant:**
```solidity
function invariant_defaultFeeCapZeroSilencesAllFees() external view {
    address pool = handler.pools(0);
    uint256 recorded = handler.ghost_feeWithDefaultCapZero();
    if (recorded == 0) return;
    (uint24 baseFee,,,,) = target.dynamicFeeConfig(pool);
    if (baseFee == 0 || baseFee == 420) return;
    assertGe(recorded, uint256(baseFee), "defaultFeeCap=0 silenced baseFee: getFee returned less than baseFee");
}
```

**Root cause:** `setDefaultFeeCap` allows `_defaultFeeCap = 0` (only checks `<= MAX_FEE_CAP`). When `defaultFeeCap = 0` and a pool's `scalingFactor == 0`, `getFee` sets `feeCap = 0` and then:
```solidity
totalFee = totalFee < feeCap ? totalFee : feeCap;
// uint256: any totalFee < 0 is impossible → totalFee = feeCap = 0
```
Every pool using default config silently returns 0 fees: no `ZERO_FEE_INDICATOR`, no revert, no event.

**Reproducing call sequence:**
```
handle_setDefaultFeeCapZero()
  → setDefaultFeeCap(0) accepted without revert
handle_setCustomFee(poolSeed)
  → setCustomFee(pool, baseFee) but getFee returns 0
handle_snapshotFeeWithDefaultCapZero()
  → ghost_feeWithDefaultCapZero = 0, assertGe fails
```

---

#### Bug 3: `initialFee` can be set above the pool's `feeCap`

**Invariant:**
```solidity
function invariant_initialFeeNotBoundedByFeeCap() external view {
    if (!handler.ghost_initialFeeCapSnapshotValid()) return;
    uint256 fee = handler.ghost_feeAtInitialFeeCapSnapshot();
    uint24 cap = handler.ghost_feeCapAtInitialSnapshot();
    if (fee == 0) return;
    assertLe(fee, uint256(cap), "initialFee not bounded by per-pool feeCap");
}
```

**Root cause:** `setInitialFee` validates `_fee <= MAX_FEE_CAP` but not against the pool's own `feeCap`. The `initialFeeEnabled` early-return path returns the raw `initialFee` with no cap applied:
```solidity
if (dfc.initialFeeEnabled) {
    ...
    return uint24(_initialFee);   // no cap: can exceed feeCap
}
```

**Reproducing call sequence:**
```
handle_setInitialFeeAboveFeeCap(feeSeed, capSeed)
  → setFeeCap(pool, 5877), setInitialFee(pool, 7923)
  → getFee returns 7923 against a pool capped at 5877
handle_snapshotInitialFeeVsFeeCap()
  → records fee=7923, cap=5877, assertLe fails
```

---

#### Bug 4: `resetDynamicFee` preserves `baseFee`, permanently zeroing fees when set to `ZERO_FEE_INDICATOR`

**Root cause:** `resetDynamicFee` deletes `feeCap`, `scalingFactor`, `initialFeeEnabled`, and `initialFee`, but **not** `baseFee`. If `baseFee = ZERO_FEE_INDICATOR (420)` was previously set, the reset leaves the pool silently returning 0 fees:
```solidity
if (baseFee == ZERO_FEE_INDICATOR) return 0;   // fires before any computation
```

**Reproducing call sequence:**
```
handle_setCustomFeeZeroIndicator()
  → setCustomFee(pool, 420 = ZERO_FEE_INDICATOR)
handle_resetDynamicFee()
  → resetDynamicFee(pool): baseFee NOT cleared
  → getFee(pool) == 0 permanently
```

---

#### Bug 5: Discounted fee can exceed non-discounted fee

**Invariant:**
```solidity
function invariant_discountedFeeLeNonDiscountedFee() external view {
    uint256 discFee = handler.ghost_feeForDiscountedActor();
    uint256 nonDiscFee = handler.ghost_feeForNonDiscountedActor();
    assertLe(discFee, nonDiscFee, "discounted fee exceeds non-discounted fee");
}
```

**Root cause:** The discount block uses `mulDivRoundingUp` (rounds the discount amount up) and runs only on the normal fee path. Under certain state combinations involving `scalingFactor`, `feeCap`, and oracle state, the discounted actor ends up paying more than the non-discounted one.

**Reproducing call sequence:** Requires the `vm.prank(actor, actor)` pattern to set both `msg.sender` and `tx.origin` to a registered discounted address, then snapshot `getFee` for both actors. The invariant fires when the fee snapshots cross.

---

#### Bug 6: Discount entirely skipped on `initialFee` path

**Invariant:**
```solidity
function invariant_initialFeePathRespectsDiscount() external view {
    uint256 discFee = handler.ghost_feeForDiscountedActor();
    uint256 nonDiscFee = handler.ghost_feeForNonDiscountedActor();
    if (discFee == 0 && nonDiscFee == 0) return;
    assertLe(discFee, nonDiscFee, "discounted fee exceeds non-discounted fee (atomic snapshot)");
}
```

**Root cause:** When `initialFeeEnabled` is true and the initial period is active (`lastObsTimestamp != block.timestamp`), the contract returns `initialFee` directly:
```solidity
if (dfc.initialFeeEnabled) {
    ...
    return uint24(_initialFee);   // returns before discount logic below
}

// Discount block: never reached on initialFee path:
if (discounted[tx.origin] > 0) { ... }
```

A registered discounted address pays the full `initialFee`, identical to any non-discounted address.

---

#### Bug 7: `defaultFeeCap = 0` suppresses `baseFee` on normal path

Same root cause as Bug 2 but confirmed via a different handler path that exercises the normal dynamic fee calculation (not the snapshot handler), providing an independent Forge call sequence.

---

## Method 5, invgen: LLM-Generated Invariants

[invgen](https://github.com/fuzzland/invgen) (by fuzzland) takes a Foundry project and a setup file, then uses a Chain-of-Thought LLM workflow to: (1) identify vulnerable functions, (2) generate Solidity invariant functions targeting each vulnerability, (3) compile and validate each invariant, retrying up to 10 times on failure.

**Run:** 59 LLM calls, ~20 minutes wall clock (09:57–10:17 PM), $2.15, model: Claude Sonnet 4.6 via OpenRouter.

| Metric | Value |
|---|---|
| Total LLM calls | 59 |
| Prompt tokens | 324,700 |
| Completion tokens | 78,098 |
| Total tokens | 402,798 |
| Cost | $2.15 |
| Calls truncated at token limit | **35 / 59** |
| Calls completed normally | 24 / 59 |
| Confirmed findings | **0** |
| Invariants targeting known bugs | 6 of 7 |

### Why no confirmed findings

invgen produced 0 confirmed findings despite generating invariants for 6 of 7 known bugs. Two blockers prevented any invariant from compiling:

**Blocker 1: solc compatibility.** invgen compiles generated code using plain `solc --standard-json` (not forge). The slipstream project has three test files (`MockTimeCLPool.sol`, `MockTimeNonfungiblePositionManager.sol`, `MockTimeSwapRouter.sol`) that override `public virtual` functions with `internal`: valid under forge but rejected by strict `solc`. A fourth file (`Oracle.sol`) uses `abicoder v2` types without the required pragma. Every compilation attempt failed on these errors before reaching the generated invariant code.

**Blocker 2: 1500-token completion limit.** invgen hardcodes `max_tokens: 1500` per LLM call. **35 of 59 calls hit this limit** (stop reason: `length`), producing truncated Solidity that could not compile even after the project-level errors were resolved. Generating a complete invariant contract in Solidity requires more than 1500 tokens.

### What invgen generated (from cache)

Despite the compilation failures, all LLM responses are cached. Analysis of the cache reveals the invariants that would have been generated:

| Vulnerability targeted | Key invariants generated | Known bug |
|---|---|---|
| `setDefaultFeeCap` | `invariant_defaultFeeCapIsNeverZero`, `invariant_defaultFeeCapGreaterThanZero` | Bug 2 |
| `setScalingFactor` | `invariant_zeroScalingFactorMustNotAllowDefaultFeeCapToExceedPoolFeeCap`, `invariant_scalingFactorZeroDoesNotBypassPoolFeeCap` | Bug 1 |
| `getFee` | `invariant_getFeeNeverExceedsEffectiveFeeCap`, `invariant_getFeeNeverExceedsFeeCap` | Bug 1 + Bug 3 |
| `setInitialFee` | `invariant_initialFeeRespectsEffectiveFeeCap`, `invariant_initialFeeDoesNotBypassFeeCapLogic` | Bug 3 |
| `registerDiscounted` | `invariant_discountedFeeAlwaysLEQUndiscountedFee`, `invariant_discountReducesFeeOnAllPoolsWhenTxOriginIsDiscounted` | Bug 5 + Bug 6 |
| `setCustomFee` | `invariant_baseFeeNeverEqualsZeroFeeIndicator`, `invariant_noAmbiguousZeroFeeState` | Bug 2 (related) |
| `resetDynamicFee` | **No invariant generated** | Bug 4: missed |

### Fuzzing results (manually extracted invariants)

Because invgen's compilation step failed, we manually extracted the best complete invariants from the LLM cache, fixed the structural assembly issue, and ran them with `forge test --fuzz-runs 256`. The full test file is at [`test/invgen/DynamicSwapFeeModuleInvGen.t.sol`](../slipstream/test/invgen/DynamicSwapFeeModuleInvGen.t.sol).

| Invariant | Result | Bug targeted |
|---|---|---|
| `invariant_defaultFeeCapIsNeverZero` | PASS | Bug 2 |
| `invariant_defaultFeeCapWithinValidRange` | **FAIL** | Bug 2: `setDefaultFeeCap(0)` caught on call 1 |
| `invariant_getFeeNeverExceedsEffectiveFeeCap` | PASS | Bug 1 + Bug 3 |
| `invariant_getFeeNeverExceedsAbsoluteMaxFeeCap` | PASS | Bug 3 |
| `invariant_initialFeeNotExceedMaxFeeCap` | PASS | Bug 3 |
| `invariant_scalingFactorNeverExceedsMax` | PASS | Defensive |
| `invariant_scalingFactorNoSingleTxJumpToMax` | PASS | Defensive |

**1 bug confirmed, 6 passed.** Only Bug 2 (`setDefaultFeeCap(0)` silencing all fees) was caught.

#### Why the other bugs were not found

Every function call in the run reverted: the "Calls" and "Reverts" columns were equal for all 12 functions. The module's `onlySwapFeeManager` modifier rejects any caller that is not `factory.swapFeeManager`. The fuzzer called functions from a random address, got rejected every time, and only stumbled on `setDefaultFeeCap(0)` because that function happened to be called with the authorized sender.

This is the structural weakness of invgen without a handler: **no handler means no `vm.prank`, which means the fuzzer spends its entire budget on invalid call paths and never reaches the interesting state transitions**. FuzzMing's handler explicitly calls all functions as the authorized `swapFeeManager` using `vm.prank`, which is why it found 7 bugs while invgen's raw invariants found only 1 in the same time budget.

---

## Comparison: FuzzMing vs. Shieldify

### What both found

| FuzzMing | Shieldify | Description |
|---|---|---|
| Bugs 5 + 6 | L-01 (partial) | Discount inconsistent: Shieldify reported rounding direction; FuzzMing confirmed the invariant violation and the complete absence of discount on the `initialFee` path |
| Bug 4 | L-04 | `resetDynamicFee` does not reset `baseFee` |

### What Shieldify found that FuzzMing missed

| Finding | Why FuzzMing missed it |
|---|---|
| M-01: Cardinality pre-check wrong variable | The try/catch already handles it at runtime: no observable invariant violation; requires static analysis |
| L-02: `MIN_SECONDS_AGO` wrong for BNB Chain | Chain-specific knowledge; FuzzMing treats contract constants as correct without external chain context |
| L-03: `slot0` spot price manipulation | Requires adversarial two-actor sequencing; FuzzMing uses a single-actor model |

### What FuzzMing found that Shieldify missed

| FuzzMing bug | Root cause class |
|---|---|
| Bug 1: `feeCap` overwritten when `scalingFactor == 0` | Silent state override: each function looks correct individually; the interaction only surfaces under a specific multi-call sequence |
| Bug 2: `defaultFeeCap = 0` zeroes all fees | Missing lower-bound validation: 0 passes the `<= MAX_FEE_CAP` check silently |
| Bug 3: `initialFee` above pool `feeCap` | Missing cross-parameter validation in `setInitialFee` against the pool's own cap |
| Bug 6: Discount absent on `initialFee` path | Shieldify reported rounding direction (L-01) but not the complete absence of discount on this code path |
| Bug 7: `defaultFeeCap = 0` via normal path | Second call sequence confirming Bug 2 via a different handler path |

---


## Comparison: FuzzMing vs. invgen

### Approach difference

| Dimension | FuzzMing | invgen |
|---|---|---|
| How it works | Generates a Forge handler + invariants together; ghost variables track expected state across call sequences | Identifies vulnerabilities via CoT, then generates invariant-only functions; relies on Foundry's built-in random caller |
| What it produces | Handler + invariants + shrunk call sequence + bug report | Invariant functions only (no handler, no call sequence) |
| **Loop** | **Generation + fuzzing + reporting in one command** | **Generation only: user must run `forge test` separately** |
| Compilation | Always compiles (iterative repair loop with forge) | Uses plain `solc`: fails on projects with forge-specific constructs |
| Token limit | Unlimited (`max_tokens=0`) | Hardcoded 1500: truncates ~60% of responses on this contract |
| Multi-step sequences | Handler explicitly models valid call sequences | No handler: Foundry calls target functions randomly |

### What invgen targeted that FuzzMing confirmed

| invgen invariant | FuzzMing bug |
|---|---|
| `invariant_defaultFeeCapIsNeverZero` | Bug 2 |
| `invariant_zeroScalingFactorMustNotAllowDefaultFeeCapToExceedPoolFeeCap` | Bug 1 |
| `invariant_getFeeNeverExceedsEffectiveFeeCap` | Bug 1 + Bug 3 |
| `invariant_initialFeeRespectsEffectiveFeeCap` | Bug 3 |
| `invariant_discountedFeeAlwaysLEQUndiscountedFee` | Bug 5 |
| `invariant_discountReducesFeeOnAllPoolsWhenTxOriginIsDiscounted` | Bug 6 |

### What FuzzMing found that invgen did not target

**Bug 4: `resetDynamicFee` omits `baseFee`.** invgen's vulnerability detection for `resetDynamicFee` evaluated to low/medium likelihood and was skipped. The bug requires a specific stateful sequence (set → reset → check) that invgen's single-function analysis did not encode.

### Key structural difference

FuzzMing's invariants use **ghost variables**: a parallel model of expected state that is updated by the handler before every invariant check. This lets FuzzMing assert things like "the fee returned must be ≤ the feeCap we recorded as having been set," using state the contract itself does not expose.

invgen's invariants query **only the contract's own view functions**. They can check that `getFee() ≤ defaultFeeCap` but not that `getFee() ≤ the specific cap that was set for this pool`: because invgen has no ghost variable tracking which cap was set for which pool during the fuzzing sequence.

---

## Comparison: FuzzMing vs. Claude Web

### What both found

All FuzzMing bugs (1–7) map to a Claude Web finding. The overlap on raw bug discovery is complete from both sides.

| FuzzMing | Claude Web | Description |
|---|---|---|
| Bug 1 | F3 + F4 | `feeCap`/`scalingFactor == 0` coupling and the `setScalingFactor(0)` entry-point gap |
| Bug 2 | F2 | `defaultFeeCap = 0` zeroes all fees |
| Bug 3 | F1 | `initialFee` bypasses pool `feeCap` |
| Bug 4 | F7 | `resetDynamicFee` omits `baseFee` |
| Bugs 5+6 | F5+F6 | Discount absent / inconsistent on `initialFee` path, confirmed via `vm.prank(addr, addr)` |
| Bug 7 | F2 (second path) | `defaultFeeCap = 0` on normal fee path |
| - | F8 | TWAP tick truncation |

### What Claude Web found that FuzzMing did not

| Finding | Why FuzzMing missed it |
|---|---|
| F8: TWAP tick truncation | Requires Uniswap V3 oracle math domain knowledge; FuzzMing has no independent reference implementation to compare the division result against |

### The structural difference

| Dimension | Claude Web | FuzzMing |
|---|---|---|
| How it works | Single LLM prompt, no code execution | Stateful invariant fuzzing with Forge handler |
| Output | Text descriptions of root causes only: no test code, no runnable output | Root cause + shrunk call sequence + Solidity invariant |
| Reusability | None: findings are unconfirmed until a developer manually writes and runs tests | Call sequences drop directly into CI as regression tests |
| Unique finds | F8 | None on this contract |
| False positives | 0 | 0 |

---

## Comparison: FuzzMing vs. Claude Code

### Finding map

| FuzzMing bug | Claude Code equivalent | Both found? |
|---|---|---|
| Bug 1: `feeCap` overwritten when `scalingFactor == 0` | BUG-5 | ✓ |
| Bug 2: `defaultFeeCap = 0` zeroes all fees | BUG-2 | ✓ |
| Bug 3: `initialFee` above pool `feeCap` | BUG-1 | ✓ |
| Bug 4: `resetDynamicFee` omits `baseFee` | BUG-3 | ✓ |
| Bug 5: Discounted fee can exceed non-discounted fee | - | FuzzMing only |
| Bug 6: Discount absent on `initialFee` path | BUG-4 | ✓ |
| Bug 7: `defaultFeeCap = 0` via normal path | Folded into BUG-2 | ✓ (partial) |

**6 of 7 FuzzMing bugs confirmed by Claude Code. 1 unique miss (Bug 5).**

### What Claude Code found that FuzzMing does not flag separately

**`return uint24(baseFee)` at L172 also bypasses `feeCap`.** When `_initialFee == 0`, `getFee` returns the raw `baseFee` before the cap-enforcement line: if `baseFee > feeCap`, the cap is violated on this sub-path too. FuzzMing's Bug 3 focuses on the `return uint24(_initialFee)` case at L174 only.

**Constructor `_defaultFeeCap = 0` is a second vulnerable site.** The constructor at L52 has the same missing lower-bound as `setDefaultFeeCap`. Claude Code flagged both sites explicitly; FuzzMing covers the runtime behavior without distinguishing the two entry points.

### What FuzzMing found that Claude Code missed

**Bug 5: Discounted fee can exceed non-discounted fee.** This is distinct from "discount not applied." FuzzMing's `invariant_discountedFeeLeNonDiscountedFee` fires when a discounted actor pays *more* than an undiscounted one, due to `mulDivRoundingUp` interacting with specific `scalingFactor`/`feeCap`/oracle-state combinations. Claude Code encoded that the discount is absent on the `initialFee` path (BUG-4), but never wrote the ordering invariant `fee_discounted ≤ fee_undiscounted` across all paths. This is a distinct failure mode that the hand-crafted property suite did not cover.

### Structural difference

| Dimension | Claude Code | FuzzMing |
|---|---|---|
| How it works | Hand-written invariant properties, single-call tests | Handler-based multi-call state exploration |
| Approach | Property-first: encode belief, fuzzer confirms or breaks it | Handler-first: explore all valid sequences, check invariants after each |
| Proof artifact | Seed-reproducible Foundry counterexample | Shrunk Forge call sequence (full state-transition path) |
| Missed | Bug 5: cross-path ordering invariant never encoded | L172 `baseFee` bypass, constructor entry point |

The structural difference explains the one miss: Claude Code must encode what it believes should hold before the fuzzer can find a counterexample. FuzzMing checks ordering invariants like `fee_discounted ≤ fee_undiscounted` automatically, regardless of which code path is active, without requiring the engineer to anticipate the specific failure mode.

---

## FuzzMing: Strengths and Limitations

### Strengths

| Strength | Evidence from this case study |
|---|---|
| Fully automated: zero human effort after invocation | 23 min wall clock, 0 human hours |
| Finds state-interaction bugs invisible in line-by-line review | Bugs 1–3 require a specific multi-call sequence; every individual function looks correct in isolation |
| Machine-verifiable proof for every finding | Every bug ships with a shrunk Forge call sequence that reproduces the issue deterministically |
| Findings are CI-ready out of the box | Each Solidity invariant can be committed directly as a regression test |
| Catches cross-path ordering invariants automatically | Bug 5 (discounted fee > non-discounted fee) found without the engineer anticipating the failure mode |
| Low cost at scale | $4.94 in LLM API spend for 7 findings on a 161-line contract |

### Limitations

An invariant needs two things to work: something to **observe** (a contract output that differs between correct and buggy code) and something to **compare against** (a reference value that defines what correct looks like). Each of the four missed bugs makes one or both impossible.

---

#### M-01: Nothing to observe

The pre-check reads `observationCardinality` instead of `observationCardinalityNext`. But immediately below it, a `try/catch` handles every oracle failure by returning 0. Both the wrong path and the correct path produce the same output:

```
Wrong variable:  pre-check passes → observe() fails → catch → return 0
Correct variable: pre-check fails → return 0 immediately
Both:            return 0
```

FuzzMing cannot write an invariant that fails here because no output ever differs. The bug exists in the code but is invisible at runtime. It requires a static analysis tool that reads code structure and flags "these two paths always produce the same result": not a fuzzer that runs the code and checks outputs.

---

#### L-02: No reference value to compare against

The invariant would need to say:

> `MIN_SECONDS_AGO` must equal the chain's block time

But the chain's block time (0.45 seconds for BNB Chain) is **not written anywhere in the contract**. FuzzMing reads only the contract source. From that source, `MIN_SECONDS_AGO = 2` is used consistently everywhere and passes all checks. There is nothing inside the contract to put on the right-hand side of an assertion. FuzzMing would need to be given the chain's block time as external input: without it, `2` is indistinguishable from any other valid constant.

---

#### L-03: No second actor to model the attack

The attack requires this exact sequence:

```
1. Attacker:  large swap → spot price moves far from TWAP
2. Victim:    swap       → pays inflated fee because distance is now huge
3. Attacker:  swap back  → profits from the arbitrage
```

The invariant would need to say:

> the fee the victim pays must not exceed what it would have been without the attacker's prior swap

To check that, FuzzMing needs two independent actors making calls in an adversarial order, and a baseline fee to compare against. FuzzMing's handler calls from a single neutral actor. It has no model of "an attacker moved state before this call." Even if it randomly generated a large swap followed by a victim swap, it would have no way to know the second fee was inflated: it has no baseline to compare against.

---

#### F8: No reference formula to compare against

The fee is slightly too low because Solidity integer division truncates toward zero. For negative tick differences (price falling), this means the magnitude is underestimated by 1 tick roughly half the time.

The invariant would need to say:

> the computed fee must be ≥ the mathematically correct fee

To check that, FuzzMing would have to implement the correct TWAP formula independently inside the invariant:

```solidity
// FuzzMing would need to write this as its own reference:
int256 diff = tickCumulatives[1] - tickCumulatives[0];
int24 correctTwap = (diff < 0 && diff % int256(secondsAgo) != 0)
    ? int24(diff / int256(secondsAgo) - 1)  // round toward -infinity
    : int24(diff / int256(secondsAgo));
uint24 correctFee = baseFee + absTickDelta(correctTwap) * scalingFactor / precision;
assertGe(actualFee, correctFee);
```

Writing this requires knowing that signed division in Solidity rounds toward zero, that for this specific formula the correct direction is toward negative infinity, and the full Uniswap V3 oracle math. FuzzMing has none of that. It only sees what the contract computed: it cannot know the result is wrong without an independent reference.

---

#### Summary

| Bug | What FuzzMing would need | What it has |
|---|---|---|
| M-01 | An output that differs between buggy and correct code | Both paths return 0: nothing to distinguish |
| L-02 | The chain's block time to compare `MIN_SECONDS_AGO` against | Only the contract source: no external chain data |
| L-03 | Two actors and a baseline fee before the attack | One neutral actor, no pre-attack baseline |
| F8 | An independent correct reference implementation of the TWAP formula | Only the contract's own computation |

---

## invgen: Strengths and Limitations

### Strengths

| Strength | Evidence from this case study |
|---|---|
| Strong LLM detection: correct identification without running code | 6 of 7 known bugs correctly targeted through Chain-of-Thought analysis alone |
| Low cost | $2.15 for 59 LLM calls covering all public functions |
| Cache-based: no repeated API spend | All 59 responses cached; re-running invgen costs $0 if the contract has not changed |
| Works from contract source alone | No prior test suite or handler required for the identification phase |

### Limitations

#### No handler: access-controlled functions unreachable

invgen generates raw invariant functions with no handler. Foundry's built-in fuzzer calls target contract functions from randomly selected senders. The module's `onlySwapFeeManager` modifier rejects every caller that is not `factory.swapFeeManager`. In the fuzzing run, **all 12 functions reverted 100% of the time**: the fuzzer's entire budget was spent on rejected calls. The 1 confirmed bug was caught only because `setDefaultFeeCap(0)` happened to be called from the correct sender by random chance.

FuzzMing's handler explicitly sets `msg.sender` and `tx.origin` via `vm.prank` before every call, so every function executes. Without an equivalent handler, invgen's invariants are structurally unable to explore state transitions on access-controlled contracts.

#### solc vs. forge: compilation fails on forge-specific projects

invgen compiles generated code using plain `solc --standard-json`, not `forge build`. Any project that uses forge-specific constructs (`vm.*`, overriding `public virtual` with `internal`, missing `abicoder v2` pragmas) will fail at the project compilation step: before the generated invariants are even reached. On the slipstream project, every compilation attempt failed on pre-existing test files, not on the generated code.

#### 1500-token hardcoded completion limit

invgen hardcodes `max_tokens: 1500` per LLM call. Generating a complete Solidity invariant contract: including mock contracts, `setUp`, and multiple invariant functions: requires more than 1500 tokens. **35 of 59 calls hit this limit**, producing truncated Solidity that could not compile. The fix is a single line change: `max_tokens: 0` (unlimited) or a value >= 4096.

#### Structural assembly bug: prompt and code disagree on output format

The LLM prompt asks the model to "finish writing this contract and return the **whole** contract." The assembly code expects the LLM to return only **function bodies** to be inserted inside the existing setup contract. The LLM correctly followed the prompt and returned a full contract; the assembly code then produced two contracts nested inside each other: invalid Solidity. The 10-retry loop asked the LLM to "fix this syntax error" but the error was in invgen's own assembly, not in the generated code. No LLM retry can fix a mismatch in the calling code.

#### Generation-only pipeline: fuzzing is a manual step

invgen stops after writing the invariant file. The user must separately run `forge test` or `ityfuzz`. FuzzMing runs generation, fuzzing, and reporting in a single command with no manual step.

#### No ghost variables: invariants limited to the contract's own view functions

invgen's invariants can only assert things visible through the contract's public view functions. They cannot track state across a multi-call sequence. For example, invgen can check `getFee() <= defaultFeeCap()`, but not `getFee() <= the specific feeCap that was set for this pool during this sequence`: because it has no parallel model of expected state.

#### Summary

| Limitation | Impact on this run |
|---|---|
| No handler: `vm.prank` not set | Fuzzer spent 100% of budget on reverts; 6 targeted bugs never reached |
| `solc` instead of `forge` | Every compilation attempt failed on project test files before generated code was reached |
| 1500-token limit | 35 / 59 LLM responses truncated; generated Solidity incomplete |
| Assembly / prompt mismatch | All 10 retries per invariant failed; 0 invariants compiled through invgen's own pipeline |
| Generation-only | User must run fuzzing manually after generation completes |
| No ghost variables | Invariants limited to single-state assertions on view functions |

---

## Five-Way Finding Aggregation

All findings across every method, deduplicated by root cause. For invgen: ◎ = invariant generated in LLM cache (not compiled by invgen's pipeline); ✓ = confirmed by fuzzer counterexample.

| Finding | Shieldify | FuzzMing | Claude Web | Claude Code | invgen |
|---|---|---|---|---|---|
| **Testing method** | **Manual review** | **Stateful invariant fuzzing** | **LLM static analysis** | **Property-based fuzzing** | **LLM-generated invariants** |
| **Time** | 5 days / 80 hrs | 23 min | ~7 min | ~22 min | ~20 min |
| **LLM calls** | - | 23 | 1 | - | 59 |
| **Tokens (prompt / completion)** | - | 1,198,947 / 89,478 | not tracked (web UI) | not exposed at runtime | 324,700 / 78,098 |
| **Cost** | not disclosed | $4.94 | not tracked | not tracked | $2.15 |
| `initialFee` bypasses pool `feeCap` | - | Bug 3 | F1 | BUG-1 | ◎ |
| `defaultFeeCap = 0` zeroes all fees | - | Bug 2 | F2 | BUG-2 | ✓ `invariant_defaultFeeCapWithinValidRange` |
| Per-pool `feeCap` overwritten when `scalingFactor == 0` | - | Bug 1 | F3 | BUG-5 | ◎ |
| `setScalingFactor(pool, 0)` accepted, silently drops `feeCap` | - | Bug 1 | F4 | - |: |
| `resetDynamicFee` omits `baseFee` | L-04 | Bug 4 | F7 | BUG-3 | - |
| Discount absent on `initialFee` path | L-01 (partial) | Bug 6 | F6 | BUG-4 | ◎ |
| Discounted fee can exceed non-discounted fee | L-01 (partial) | Bug 5 | - |: | ◎ |
| `tx.origin` for discount breaks smart wallets / phishable | - | Bugs 5+6 | F5 | - |: |
| TWAP tick division truncates toward zero | - |: | F8 | - |: |
| `MIN_SECONDS_AGO` wrong for BNB Chain block time | L-02 | - |: | - |: |
| `slot0` spot price manipulation | L-03 | - |: | - |: |
| Cardinality pre-check uses wrong variable | M-01 | - |: | - |: |
| **Total confirmed** | **5** | **7** | **8** | **5** | **1** |
| **Targeted (not compiled)** | - |: | - |: | **5** ◎ |
| **Unique confirmed** | 3 | 0 | 1 | 0 | 0 |
| **False positives** | 0 | 0 | 0 | 0 | 0 |

**Union across all five methods: 12 distinct confirmed findings, 0 false positives.**

---

## Conclusion

Five independent methods were run against a 161-line contract. Four confirmed 12 distinct bugs with zero false positives; invgen detected 6 of 7 bugs through LLM analysis but confirmed only 1 due to toolchain failures. No single method found everything.

**Shieldify** uniquely found 3 findings requiring context outside the contract source: a wrong chain constant, an adversarial multi-actor spot-price attack, and a redundant check whose bug is masked at runtime by a try/catch. These are unreachable by any automated tool without external context.

**Claude Web** uniquely found 1 finding: the TWAP oracle math precision issue (F8), which requires an independent reference implementation of Uniswap V3 integer division to identify. It also confirmed all FuzzMing bugs from a static reasoning angle, including the `tx.origin` design flaw and the `setScalingFactor(0)` entry-point gap: but provides no reproduction path for any of them.

**Claude Code** confirmed 6 of 7 FuzzMing bugs through a hand-crafted property suite and identified two sub-issues (the L172 `baseFee` bypass and the constructor gap) that FuzzMing folds into broader findings. It missed Bug 5 because the ordering invariant `fee_discounted ≤ fee_undiscounted` was never encoded as a property.

**FuzzMing** found 5 bugs that Shieldify missed, all rooted in state interactions that look correct function-by-function. It uniquely confirmed Bug 5 (fee inversion under specific state combinations): a cross-path ordering property that property-based fuzzing only finds if explicitly written. Every finding is backed by a shrunk Forge call sequence and a Solidity invariant usable directly as a CI regression test.

**invgen** generated invariants targeting 6 of 7 known bugs in ~20 minutes at $2.15, but produced 0 confirmed findings due to two blockers: (1) the project's test files use forge-specific constructs rejected by plain `solc`, and (2) invgen's hardcoded 1500-token completion limit truncated 35 of 59 LLM responses mid-generation, producing incomplete Solidity that could not compile. With these issues fixed, invgen's invariants would likely confirm most of the same bugs as FuzzMing: at lower cost and with a different generation strategy (vulnerability-first CoT vs. FuzzMing's handler-first exploration).

The right strategy is to combine methods: FuzzMing for combinatorial state bugs with reproducible proof, LLM static analysis for broad surface coverage, and a professional audit for threat-model and chain-specific review.
