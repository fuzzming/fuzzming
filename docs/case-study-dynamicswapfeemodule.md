# Case Study: DynamicSwapFeeModule, Five-Method Security Analysis

**Target:** [`DynamicSwapFeeModule.sol`](https://github.com/aerodrome-finance/slipstream/blob/main/contracts/core/fees/DynamicSwapFeeModule.sol) (149 nSLOC): a concentrated liquidity dynamic fee module from a Uniswap V3 fork deployed on BNB Chain.

**Reference audit:** Shieldify Security: 5-day engagement, 80 auditor-hours. Full report on [Solodit](https://solodit.cyfrin.io/issues/l-02-min_seconds_ago-hardcoded-to-2-seconds-but-bnb-chain-block-time-is-045-seconds-shieldify-none-topaz-dex-markdown).

Five independent methods were run against the same 161-line contract. The five methods together found 11 distinct root causes with zero false positives. FuzzMing confirmed 7 findings across 6 unique root causes in 23 minutes at $4.94 with zero human effort, each backed by a shrunk Forge call sequence that reproduces the finding deterministically.

---

## Contents

- [Key Concepts](#key-concepts)
- [Overview](#overview)
- [Five-Way Finding Aggregation](#five-way-finding-aggregation)
- [Scope](#scope)
- [Method 1, Shieldify: Professional Manual Audit](#method-1-shieldify-professional-manual-audit)
- [Method 2, Claude Web: LLM Static Analysis](#method-2-claude-web-llm-static-analysis)
  - [Findings](#findings)
  - [Finding Detail](#finding-detail)
    - [F1: `initialFee` bypasses pool-specific `feeCap`](#f1-initialfee-bypasses-pool-specific-feecap)
    - [F2: `setDefaultFeeCap(0)` zeros all protocol fees](#f2-setdefaultfeecap0-zeros-all-protocol-fees)
    - [F3: Pool-specific `feeCap` silently ignored when `scalingFactor == 0`](#f3-pool-specific-feecap-silently-ignored-when-scalingfactor--0)
    - [F4: `setScalingFactor(pool, 0)` accepted, silently drops `feeCap`](#f4-setscalingfactorpool-0-accepted-silently-drops-feecap)
    - [F5: `tx.origin` used for discount check](#f5-txorigin-used-for-discount-check)
    - [F6: Discount not applied on `initialFee` early-return path](#f6-discount-not-applied-on-initialfee-early-return-path)
    - [F7: `resetDynamicFee` does not reset `baseFee`](#f7-resetdynamicfee-does-not-reset-basefee)
    - [F8: TWAP tick division truncates toward zero](#f8-twap-tick-division-truncates-toward-zero)
- [Method 3, Claude Code: Property-Based Fuzzing](#method-3-claude-code-property-based-fuzzing)
  - [Findings](#findings-1)
- [Method 4, FuzzMing: Stateful Invariant Fuzzing](#method-4-fuzzming-stateful-invariant-fuzzing)
  - [Findings](#findings-2)
  - [Finding Detail](#finding-detail-1)
    - [Bug 1: Per-pool `feeCap` silently overwritten when `scalingFactor == 0`](#bug-1-per-pool-feecap-silently-overwritten-when-scalingfactor--0)
    - [Bug 2: `defaultFeeCap = 0` silently zeroes all fees](#bug-2-defaultfeecap--0-silently-zeroes-all-fees)
    - [Bug 3: `initialFee` can be set above the pool's `feeCap`](#bug-3-initialfee-can-be-set-above-the-pools-feecap)
    - [Bug 4: `resetDynamicFee` preserves `baseFee`, permanently zeroing fees](#bug-4-resetdynamicfee-preserves-basefee-permanently-zeroing-fees-when-set-to-zero_fee_indicator)
    - [Bug 5: Discounted fee can exceed non-discounted fee](#bug-5-discounted-fee-can-exceed-non-discounted-fee)
    - [Bug 6: Discount entirely skipped on `initialFee` path](#bug-6-discount-entirely-skipped-on-initialfee-path)
    - [Bug 7: `defaultFeeCap = 0` suppresses `baseFee` on normal path](#bug-7-defaultfeecap--0-suppresses-basefee-on-normal-path)
- [Method 5, invgen: LLM-Generated Invariants](#method-5-invgen-llm-generated-invariants)
  - [Why only 1 unique bug was confirmed](#why-only-1-unique-bug-was-confirmed)
  - [What invgen generated (from cache)](#what-invgen-generated-from-cache)
  - [Fuzzing results (manually extracted invariants)](#fuzzing-results-manually-extracted-invariants)
- [Comparisons](#comparisons)
  - [FuzzMing vs. Shieldify](#fuzzming-vs-shieldify)
  - [FuzzMing vs. Claude Web](#fuzzming-vs-claude-web)
  - [FuzzMing vs. Claude Code](#fuzzming-vs-claude-code)
  - [FuzzMing vs. invgen](#fuzzming-vs-invgen)
- [Strengths and Limitations](#strengths-and-limitations)
  - [FuzzMing](#fuzzming)
  - [invgen](#invgen)
- [Conclusion](#conclusion)

---

## Key Concepts

*Already familiar with smart contract fuzzing and Foundry? Skip this section.*

**Invariant:** A rule that must always be true about a smart contract, no matter how it is called or in what order. Example: *"the fee charged to any user must never exceed the configured cap."* An invariant test repeatedly calls the contract with random inputs and checks whether the rule ever breaks.

**Fuzzing:** Automatically testing software by feeding it a large number of random inputs at speed, looking for cases where it crashes or behaves incorrectly. In smart contract security, this means calling contract functions with random values and sequences to discover unexpected behavior.

**Stateful invariant fuzzing** (FuzzMing's approach): The fuzzer builds up contract state across multiple transactions before checking invariants. A single call might look fine. It is the combination of calls (set this, reset that, call in this order) that reveals the bug. This is what catches state-interaction bugs that are invisible to code review.

**Property-based fuzzing** (Claude Code's approach): The developer hand-writes the rules they believe should hold, then runs a fuzzer to find a counterexample. The fuzzer confirms or breaks properties that were explicitly written. It does not discover new properties on its own.

**Handler:** A wrapper contract written around the target contract. It calls the target as an authorized user (so access-control checks pass), tracks state in ghost variables, and gives the fuzzer a realistic set of operations to explore.

**Ghost variables:** Variables inside the handler that independently track what the contract's state *should* be. For example, when the handler calls `setFeeCap(pool, 5000)`, it also records `ghost_feeCap[pool] = 5000`. An invariant can then assert `getFee(pool) ≤ ghost_feeCap[pool]`, comparing the contract's actual output against what the test itself observed being set.

**`vm.prank`:** A Foundry testing utility that sets who appears to be making the next call. Without it, the fuzzer calls functions from random addresses that get immediately rejected by access-control rules.

**Shrunk call sequence:** When the fuzzer finds a bug, it automatically trims the reproduction steps down to the minimum needed to trigger it. Instead of a 200-step random trace, you get something like `setDefaultFeeCap(0) → getFee(pool)`: two steps a developer can immediately understand and reproduce.

**LLM (Large Language Model):** An AI model capable of reading and writing code, such as Claude or GPT-4. Used here to generate Solidity test contracts, analyze vulnerabilities, or both.

**Foundry / `forge`:** The standard smart contract testing framework. `forge build` compiles contracts; `forge test` runs tests including invariant fuzzing; `forge coverage` measures which lines of code the tests exercise.

**`solc`:** The Solidity compiler. Its only job is to translate `.sol` files into bytecode that runs on the blockchain. `forge build` uses `solc` under the hood, but first does its own preparation: it resolves all file imports, handles Foundry-specific test utilities like `vm.*`, and is more permissive with patterns commonly used in test code. Calling `solc` directly skips all of that preparation. It sees raw files as-is, rejects anything it does not recognize as standard Solidity, and has no awareness of Foundry conventions. The analogy: `solc` is a basic compiler, `forge build` is a full build system that runs that compiler with the right context and configuration. A project built for `forge build` will often fail when compiled with `solc` directly.

---

## Overview

No method had access to the results of the others before running.

> **Note on counting:** All bug counts use **unique root causes**, not raw finding counts. Two findings with the same root cause and the same fix count as 1. See footnotes for details on each method's deduplication.
>
> **"Stateful invariant fuzzing"** (FuzzMing) is *how tests are executed*: the fuzzer builds multi-step call sequences and checks invariants after each one. **"LLM-generated invariants"** (invgen) is *how tests are written*: the LLM produces the test code, but a separate fuzzer must still run it to confirm a bug. They are not competing approaches. invgen needs a fuzzer like FuzzMing to produce a confirmed finding.

| Metric | Shieldify | Claude Web | Claude Code | FuzzMing | invgen |
|---|---|---|---|---|---|
| Testing method | Manual review | LLM static analysis | Property-based fuzzing | Stateful invariant fuzzing | LLM-generated invariants |
| Time | 5 days / 80 hrs | ~7 min (LLM only) ³ | ~22 min | 23 min ¹ | ~20 min (generation only) ² |
| LLM calls | - | 1 | - | 23 | 59 |
| Cost | not disclosed | not tracked | not tracked | $4.94 | $2.15 |
| Human effort | 80 hrs | ~7 min | 0 | 0 | 0 |
| Bugs detected (unique root causes) | 5 | 7 ⁶ | 5 | 6 ⁷ | 5 |
| Bugs confirmed (unique root causes) | 5 | 0 (7 unverified) ⁴ | 5 | 6 ⁷ | **1** ⁵ |
| False positives | 0 | unverifiable (no test code) | 0 | 0 | 0 |
| Reproducible proof | - | - | ✓ (seed + counterexample) | ✓ (shrunk call sequence) | ✓ (call sequence) |

> ¹ **FuzzMing's 23 minutes is the full end-to-end time**: invariant generation, fuzzer execution, and bug reporting in a single automated command. No manual step required.
>
> ² **invgen's ~20 minutes covers generation only.** After invgen saves the invariant file, the user must still run `forge test` or `ityfuzz` manually. In our run, invgen failed to compile due to a toolchain mismatch and a 1500-token limit. We manually extracted the invariants from the LLM cache, applied four fixes, and ran them with forge: 1 unique bug confirmed.
>
> ³ **Claude Web's ~7 minutes is the LLM query time only.** The output is prose with no runnable test code. Each finding requires a developer to manually write and run a test to verify it is real.
>
> ⁴ **Claude Web's 7 unique root causes are prose descriptions only.** No test code was produced and no counterexample was generated. All 7 remain unverified until a developer manually writes and runs a test.
>
> ⁵ **invgen confirmed 1 unique bug (Bug 2) after four fixes were applied.** Two invariants fired but share the same root cause and the same fix. See Method 5 for the full analysis.
>
> ⁶ **Claude Web detected 7 unique root causes, not 8.** F3 and F4 share the same root behavior (`getFee` silently overwrites the pool-level `feeCap` when `scalingFactor == 0`). FuzzMing confirmed both as a single Bug 1.
>
> ⁷ **FuzzMing confirmed 6 unique root causes from 7 findings.** Bug 7 is a second reproduction path for Bug 2 (same root cause, same fix: `require(_defaultFeeCap > 0)`). All counts use unique root causes.

---

## Five-Way Finding Aggregation

All findings across every method, deduplicated by root cause.

**Legend:** ✓ confirmed by fuzzer counterexample, ◎ detected by LLM analysis only (not compiled or confirmed), `-` not found by this method.

| Finding | Shieldify | FuzzMing | Claude Web | Claude Code | invgen |
|---|---|---|---|---|---|
| `initialFee` bypasses pool `feeCap` | - | Bug 3 | F1 | BUG-1 | ◎ |
| `defaultFeeCap = 0` zeroes all fees | - | Bugs 2+7 ⁷ | F2 | BUG-2 | ✓ |
| Per-pool `feeCap` overwritten when `scalingFactor == 0` (via `getFee` and via `setScalingFactor`) | - | Bug 1 | F3+F4 ⁶ | BUG-5 | ◎ |
| `resetDynamicFee` omits `baseFee` | L-04 | Bug 4 | F7 | BUG-3 | - |
| Discount absent on `initialFee` path | L-01 (partial) | Bug 6 | F6 | BUG-4 | ◎ |
| Discounted fee can exceed non-discounted fee | L-01 (partial) | Bug 5 | - | - | ◎ |
| `tx.origin` for discount breaks smart wallets / phishable | - | Bugs 5+6 | F5 | - | - |
| TWAP tick division truncates toward zero | - | - | F8 | - | - |
| `MIN_SECONDS_AGO` wrong for BNB Chain block time | L-02 | - | - | - | - |
| `slot0` spot price manipulation | L-03 | - | - | - | - |
| Cardinality pre-check uses wrong variable | M-01 | - | - | - | - |


**Union across all five methods: 11 distinct root causes.**

---

## Scope

| File | nSLOC |
|---|---|
| [`contracts/core/fees/DynamicSwapFeeModule.sol`](https://github.com/aerodrome-finance/slipstream/blob/main/contracts/core/fees/DynamicSwapFeeModule.sol) | 149 |
| `contracts/core/interfaces/fees/IDynamicFeeModule.sol` | 12 |
| **Total** | **161** |

*nSLOC = non-comment source lines of code, a standard measure of contract size.*

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

**Root cause analysis:** All 5 findings are distinct root causes with distinct fixes. No deduplication needed.

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

**Root cause analysis:** F3 and F4 share the same root behavior: `getFee` treats `scalingFactor == 0` as valid and silently replaces the pool-level `feeCap` with the global default. F4 is an entry point that creates the `scalingFactor == 0` state; F3 is the consequence in `getFee`. FuzzMing confirmed both as a single Bug 1. Claude Web counts them separately as 2 findings, but they map to **1 unique root cause**. All other findings are distinct. Total: **8 reported, 7 unique root causes.**

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

**Root cause:** `tx.origin` is always an EOA (Externally Owned Account, meaning a regular user wallet, as opposed to a smart contract). This breaks the feature in two ways:

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

**Root cause:** Solidity integer division truncates toward zero. The TWAP (Time-Weighted Average Price, an average price computed over a time window to resist short-term manipulation) tick is slightly underestimated in magnitude, causing `absTickDelta` to be 1 tick lower than the mathematically correct value roughly 50% of the time. This produces a dynamic fee marginally lower than theoretically correct, slightly undercharging traders during volatile periods. This matches known behavior in Uniswap V3's own oracle library and requires domain knowledge of the oracle math to identify.

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

**Root cause analysis:** All 5 findings are distinct root causes with distinct fixes. No deduplication needed. Total: **5 unique root causes.**

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

**Root cause analysis:** Bug 7 shares the same root cause as Bug 2. Both are triggered by `setDefaultFeeCap(0)` being accepted without validation; both are fixed by adding `require(_defaultFeeCap > 0)`. Bug 7 is a second reproduction path through a different handler sequence, not a new bug. Bugs 1 through 6 are all distinct root causes. Total: **7 confirmed findings, 6 unique root causes.**

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

[invgen](https://github.com/fuzzland/invgen) (by fuzzland) takes a Foundry project and a setup file, then uses a Chain-of-Thought LLM workflow (a technique where the model reasons step by step before producing its final answer, rather than answering immediately) to: (1) identify vulnerable functions, (2) generate Solidity invariant functions targeting each vulnerability, (3) compile and validate each invariant, retrying up to 10 times on failure.

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
| Confirmed findings | **1** unique root cause (2 invariants fired, same bug) |
| Invariants targeting known bugs | 5 of 6 unique root causes (missed Bug 4) |

### Why only 1 unique bug was confirmed

Four problems were identified in the original invgen run. Each was investigated and fixed. After all four fixes, 2 invariants fired for Bug 2 and no other bug was confirmed.

#### Problem 1: solc compatibility (fixed)

**Problem:** invgen compiles generated code using plain `solc --standard-json` instead of `forge build`. The slipstream project has test files that override `public virtual` functions with `internal` (valid under Foundry, rejected by strict `solc`) and a file that uses `abicoder v2` types without the required pragma declaration. Every compilation attempt failed on these pre-existing project files before reaching the generated invariant code.

**Fix:** Switched to `forge build` and added `pragma abicoder v2;` at the top of the test file. Compilation succeeded.

---

#### Problem 2: 1500-token completion limit (not applicable here)

**Problem:** invgen hardcodes `max_tokens: 1500` per LLM call. **35 of 59 calls hit this limit**, producing truncated Solidity that could not compile.

**Fix:** Not applicable for the manually extracted run: the invariants were extracted directly from the LLM cache as complete functions and assembled by hand, bypassing the token limit entirely.

---

#### Problem 3: Access control (partially addressed)

**Problem:** invgen generates no authorized sender setup. The module's `onlySwapFeeManager` modifier rejects any caller that is not `factory.swapFeeManager`. With the fuzzer calling functions from random addresses, all 12 functions reverted 100% of the time. Bug 2 was the only exception: `setDefaultFeeCap(0)` happened to be called from the authorized address by chance.

**Fix attempt:** Passed `address(this)` as the `swapFeeManager` in the `MockFactory` constructor. In Foundry, `address(this)` is `0x7FA9385...`, the default test contract address.

**Why this was insufficient:** There are two separate registries that need to agree, and Problem 3's fix only updated one of them.

The first registry is the contract's authorization check: `factory.swapFeeManager`. Problem 3 set this to `0x7FA9385...`. That part is correct.

The second registry is the fuzzer's sender pool: the set of addresses Foundry actually uses as `msg.sender` when calling the target. Problem 3 did not touch this. When no `targetSender` is specified, Foundry builds this pool from randomly generated addresses. None of them are `0x7FA9385...` unless explicitly added.

So after Problem 3, every call arrived from a random address that was not `0x7FA9385...` and was immediately rejected:

```
factory.swapFeeManager = 0x7FA9385...  ← authorized address (Problem 3's fix)
msg.sender             = 0xAbC123...  ← fuzzer's random pick, never 0x7FA9385
                         ↑
                       always reverts
```

Running the fuzzer confirmed it: **Calls = Reverts for every function across all passing invariants.** State never changed because the two registries pointed at different addresses.

---

#### Problem 4: Missing `targetSender` (the key fix)

**Problem:** Problem 3 updated the contract's authorization registry but not the fuzzer's sender registry. Those are two separate things and both must point at the same address.

**Fix:** Added `targetSender(address(this))` to `setUp()`. This overwrites the fuzzer's sender pool with a single entry, so every call arrives from `0x7FA9385...`, the same address that is already the authorized `swapFeeManager`.

```solidity
targetContract(address(module));
targetSender(address(this));   // ← aligns the fuzzer's sender with factory.swapFeeManager
```

After this, both registries agree:

```
factory.swapFeeManager = 0x7FA9385...  ← authorized address
msg.sender             = 0x7FA9385...  ← fuzzer's sender (now restricted)
                         ↑
                       onlySwapFeeManager passes
```

**Result:** The `onlySwapFeeManager` reverts stopped. But the revert rate only dropped from ~100% to ~70%, not to near zero. The remaining ~70% come from a different check entirely: most functions also validate that the pool address passed as a parameter is registered in the factory. The fuzzer still picks random addresses for that argument, and almost none of them match the one pool registered in `setUp()`. Fixing the sender did not fix the pool address. These are two separate revert causes and targetSender only addressed one of them. (Detailed in Reason 1 below.)

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

Invariants were extracted directly from the LLM cache, assembled by hand into a complete test file, and run with `forge test --fuzz-runs 256` with all four fixes applied. The full test file is at [`test/invgen/DynamicSwapFeeModuleInvGen.t.sol`](../slipstream/test/invgen/DynamicSwapFeeModuleInvGen.t.sol).

| Invariant | Result | Bug targeted |
|---|---|---|
| `invariant_defaultFeeCapIsNeverZero` | **FAIL** | Bug 2 |
| `invariant_defaultFeeCapWithinValidRange` | **FAIL** | Bug 2 |
| `invariant_getFeeNeverExceedsEffectiveFeeCap` | PASS | Bug 1 + Bug 3 |
| `invariant_getFeeNeverExceedsAbsoluteMaxFeeCap` | PASS | Bug 3 |
| `invariant_initialFeeNotExceedMaxFeeCap` | PASS | Bug 3 |
| `invariant_scalingFactorNeverExceedsMax` | PASS | Defensive |
| `invariant_scalingFactorNoSingleTxJumpToMax` | PASS | Defensive |

**1 unique bug confirmed (Bug 2).** Two invariants fired (`invariant_defaultFeeCapIsNeverZero` and `invariant_defaultFeeCapWithinValidRange`), but both target the same root cause and the same fix. They are counted as 1, not 2: writing multiple invariants that check the same property in slightly different ways does not increase the number of bugs found.

#### Why the other bugs were still not found

Even with all four fixes applied, the remaining bugs were not confirmed. Three structural reasons:

**Reason 1: Unregistered pool addresses (~70% reverts remain).**

After Problem 4, the sender check passes. But functions like `setFeeCap(pool, value)`, `setInitialFee(pool, value)`, and `setScalingFactor(pool, value)` take a pool address as a parameter and require it to be registered in the factory. The fuzzer generates random `uint160` values for that argument. Only `address(pool)` from `setUp()` is registered. So the call passes `onlySwapFeeManager` and then immediately reverts on the pool-registration check. Two separate guards, two separate problems. `targetSender` fixed the first one; nothing fixed the second.

This is the structural difference with FuzzMing: FuzzMing's handler hardcodes the registered pool address. Every call to `setFeeCap`, `setInitialFee`, or `setScalingFactor` goes through the handler, which always passes `address(pool)` as the argument — the one address that is registered. The fuzzer never touches the pool address directly; it only controls the call sequence and the numeric parameters. invgen has no handler, so the fuzzer picks the pool address randomly and almost always picks one that is not registered.

**Reason 2: Oracle state is never driven (Bug 1 specifically).**

Even when `setFeeCap(address(pool), 5000)` succeeds, `getFee(address(pool))` still does not exceed the cap. The fee formula is `baseFee + absTickDelta × scalingFactor / precision`. The MockPool's cumulative tick values (`cumOld` and `cumNew`) are both zero and never changed by the fuzzer, so `absTickDelta = 0` and `getFee` returns just `baseFee`. With no price movement simulated, the fee is always small and never violates any cap. A proper handler would drive oracle state by updating the pool's cumulative ticks to simulate price volatility.

**Reason 3: No two-actor setup (Bugs 5 and 6).**

These bugs require comparing `getFee` for a discounted address against `getFee` for a non-discounted address. The discount check uses `tx.origin`. The fuzzer has no mechanism to call `getFee` twice with different `tx.origin` values in the same sequence. A handler with `vm.prank(actor, actor)` (which sets both `msg.sender` and `tx.origin`) would be needed.

**Bug 4** has no invariant generated at all and was never reachable.

In summary: Problem 4 (`targetSender`) was the missing fix that allowed both Bug 2 invariants to fire. But finding the remaining bugs requires a proper handler that drives oracle state, ensures the correct pool address is always used, and supports multi-actor scenarios, all of which go beyond what invgen generates.

---

## Comparisons

The following sections compare FuzzMing directly against each of the other four methods, covering what was found, what was missed, and the structural reason for each gap.

### FuzzMing vs. Shieldify

#### What both found

| FuzzMing | Shieldify | Description |
|---|---|---|
| Bugs 5 + 6 | L-01 (partial) | Discount inconsistent: Shieldify reported rounding direction; FuzzMing confirmed the invariant violation and the complete absence of discount on the `initialFee` path |
| Bug 4 | L-04 | `resetDynamicFee` does not reset `baseFee` |

#### What Shieldify found that FuzzMing missed

| Finding | Why FuzzMing missed it |
|---|---|
| M-01: Cardinality pre-check wrong variable | The try/catch already handles it at runtime: no observable invariant violation; requires static analysis |
| L-02: `MIN_SECONDS_AGO` wrong for BNB Chain | Chain-specific knowledge; FuzzMing treats contract constants as correct without external chain context |
| L-03: `slot0` spot price manipulation | Requires adversarial two-actor sequencing; FuzzMing uses a single-actor model |

#### What FuzzMing found that Shieldify missed

| FuzzMing bug | Root cause class |
|---|---|
| Bug 1: `feeCap` overwritten when `scalingFactor == 0` | Silent state override: each function looks correct individually; the interaction only surfaces under a specific multi-call sequence |
| Bug 2: `defaultFeeCap = 0` zeroes all fees | Missing lower-bound validation: 0 passes the `<= MAX_FEE_CAP` check silently |
| Bug 3: `initialFee` above pool `feeCap` | Missing cross-parameter validation in `setInitialFee` against the pool's own cap |
| Bug 6: Discount absent on `initialFee` path | Shieldify reported rounding direction (L-01) but not the complete absence of discount on this code path |
| Bug 7: `defaultFeeCap = 0` via normal path | Second call sequence confirming Bug 2 via a different handler path |

---

### FuzzMing vs. Claude Web

#### What both found

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

#### What Claude Web found that FuzzMing did not

| Finding | Why FuzzMing missed it |
|---|---|
| F8: TWAP tick truncation | Requires Uniswap V3 oracle math domain knowledge; FuzzMing has no independent reference implementation to compare the division result against |

#### The structural difference

| Dimension | Claude Web | FuzzMing |
|---|---|---|
| How it works | Single LLM prompt, no code execution | Stateful invariant fuzzing with Forge handler |
| Output | Text descriptions of root causes only: no test code, no runnable output | Root cause + shrunk call sequence + Solidity invariant |
| Reusability | None: findings are unconfirmed until a developer manually writes and runs tests | Call sequences drop directly into CI as regression tests |
| Unique finds | F8 | None on this contract |
| False positives | 0 | 0 |

---

### FuzzMing vs. Claude Code

#### Finding map

| FuzzMing bug | Claude Code equivalent | Both found? |
|---|---|---|
| Bug 1: `feeCap` overwritten when `scalingFactor == 0` | BUG-5 | ✓ |
| Bug 2: `defaultFeeCap = 0` zeroes all fees | BUG-2 | ✓ |
| Bug 3: `initialFee` above pool `feeCap` | BUG-1 | ✓ |
| Bug 4: `resetDynamicFee` omits `baseFee` | BUG-3 | ✓ |
| Bug 5: Discounted fee can exceed non-discounted fee | - | FuzzMing only |
| Bug 6: Discount absent on `initialFee` path | BUG-4 | ✓ |
| Bug 7: `defaultFeeCap = 0` via normal path | Folded into BUG-2 | ✓ (partial) |

**5 of 6 FuzzMing unique root causes confirmed by Claude Code. 1 unique miss (Bug 5).**

#### What Claude Code found that FuzzMing does not flag separately

**`return uint24(baseFee)` at L172 also bypasses `feeCap`.** When `_initialFee == 0`, `getFee` returns the raw `baseFee` before the cap-enforcement line: if `baseFee > feeCap`, the cap is violated on this sub-path too. FuzzMing's Bug 3 focuses on the `return uint24(_initialFee)` case at L174 only.

**Constructor `_defaultFeeCap = 0` is a second vulnerable site.** The constructor at L52 has the same missing lower-bound as `setDefaultFeeCap`. Claude Code flagged both sites explicitly; FuzzMing covers the runtime behavior without distinguishing the two entry points.

#### What FuzzMing found that Claude Code missed

**Bug 5: Discounted fee can exceed non-discounted fee.** This is distinct from "discount not applied." FuzzMing's `invariant_discountedFeeLeNonDiscountedFee` fires when a discounted actor pays *more* than an undiscounted one, due to `mulDivRoundingUp` interacting with specific `scalingFactor`/`feeCap`/oracle-state combinations. Claude Code encoded that the discount is absent on the `initialFee` path (BUG-4), but never wrote the ordering invariant `fee_discounted ≤ fee_undiscounted` across all paths. This is a distinct failure mode that the hand-crafted property suite did not cover.

#### Structural difference

| Dimension | Claude Code | FuzzMing |
|---|---|---|
| How it works | Hand-written invariant properties, single-call tests | Handler-based multi-call state exploration |
| Approach | Property-first: encode belief, fuzzer confirms or breaks it | Handler-first: explore all valid sequences, check invariants after each |
| Proof artifact | Seed-reproducible Foundry counterexample | Shrunk Forge call sequence (full state-transition path) |
| Missed | Bug 5: cross-path ordering invariant never encoded | L172 `baseFee` bypass, constructor entry point |

The structural difference explains the one miss: Claude Code must encode what it believes should hold before the fuzzer can find a counterexample. FuzzMing checks ordering invariants like `fee_discounted ≤ fee_undiscounted` automatically, regardless of which code path is active, without requiring the engineer to anticipate the specific failure mode.

---

### FuzzMing vs. invgen

#### Approach difference

| Dimension | FuzzMing | invgen |
|---|---|---|
| How it works | Generates a Forge handler + invariants together; ghost variables track expected state across call sequences | Identifies vulnerabilities via CoT, then generates invariant-only functions; relies on Foundry's built-in random caller |
| What it produces | Handler + invariants + shrunk call sequence + bug report | Invariant functions only (no handler, no call sequence) |
| **Loop** | **Generation + fuzzing + reporting in one command** | **Generation only: user must run `forge test` separately** |
| Compilation | Always compiles (iterative repair loop with forge) | Uses plain `solc`: fails on projects with forge-specific constructs |
| Token limit | Unlimited (`max_tokens=0`) | Hardcoded 1500: truncates ~60% of responses on this contract |
| Multi-step sequences | Handler explicitly models valid call sequences | No handler: Foundry calls target functions randomly |

#### What invgen targeted that FuzzMing confirmed

| invgen invariant | FuzzMing bug |
|---|---|
| `invariant_defaultFeeCapIsNeverZero` | Bug 2 |
| `invariant_zeroScalingFactorMustNotAllowDefaultFeeCapToExceedPoolFeeCap` | Bug 1 |
| `invariant_getFeeNeverExceedsEffectiveFeeCap` | Bug 1 + Bug 3 |
| `invariant_initialFeeRespectsEffectiveFeeCap` | Bug 3 |
| `invariant_discountedFeeAlwaysLEQUndiscountedFee` | Bug 5 |
| `invariant_discountReducesFeeOnAllPoolsWhenTxOriginIsDiscounted` | Bug 6 |

#### What FuzzMing found that invgen did not target

**Bug 4: `resetDynamicFee` omits `baseFee`.** invgen's vulnerability detection for `resetDynamicFee` evaluated to low/medium likelihood and was skipped. The bug requires a specific stateful sequence (set → reset → check) that invgen's single-function analysis did not encode.

#### Key structural difference

FuzzMing's invariants use **ghost variables**: a parallel model of expected state that is updated by the handler before every invariant check. This lets FuzzMing assert things like "the fee returned must be ≤ the feeCap we recorded as having been set," using state the contract itself does not expose.

invgen's invariants query **only the contract's own view functions**. They can check that `getFee() ≤ defaultFeeCap()` but not that `getFee() ≤ the specific cap that was set for this pool`: because invgen has no ghost variable tracking which cap was set for which pool during the fuzzing sequence.

---

## Strengths and Limitations

### FuzzMing

#### Strengths

| Strength | Evidence from this case study |
|---|---|
| Fully automated: zero human effort after invocation | 23 min wall clock, 0 human hours |
| Finds state-interaction bugs invisible in line-by-line review | Bugs 1–3 require a specific multi-call sequence; every individual function looks correct in isolation |
| Machine-verifiable proof for every finding | Every bug ships with a shrunk Forge call sequence that reproduces the issue deterministically |
| Findings are CI-ready out of the box | Each Solidity invariant can be committed directly as a regression test |
| Catches cross-path ordering invariants automatically | Bug 5 (discounted fee > non-discounted fee) found without the engineer anticipating the failure mode |
| Low cost at scale | $4.94 in LLM API spend for 7 findings on a 161-line contract |

#### Limitations

An invariant needs two things to work: something to **observe** (a contract output that differs between correct and buggy code) and something to **compare against** (a reference value that defines what correct looks like). Each of the four missed bugs makes one or both impossible.

---

##### M-01: Nothing to observe

The pre-check reads `observationCardinality` instead of `observationCardinalityNext`. But immediately below it, a `try/catch` handles every oracle failure by returning 0. Both the wrong path and the correct path produce the same output:

```
Wrong variable:  pre-check passes → observe() fails → catch → return 0
Correct variable: pre-check fails → return 0 immediately
Both:            return 0
```

FuzzMing cannot write an invariant that fails here because no output ever differs. The bug exists in the code but is invisible at runtime. It requires a static analysis tool that reads code structure and flags "these two paths always produce the same result": not a fuzzer that runs the code and checks outputs.

---

##### L-02: No reference value to compare against

The invariant would need to say:

> `MIN_SECONDS_AGO` must equal the chain's block time

But the chain's block time (0.45 seconds for BNB Chain) is **not written anywhere in the contract**. FuzzMing reads only the contract source. From that source, `MIN_SECONDS_AGO = 2` is used consistently everywhere and passes all checks. There is nothing inside the contract to put on the right-hand side of an assertion. FuzzMing would need to be given the chain's block time as external input: without it, `2` is indistinguishable from any other valid constant.

---

##### L-03: No second actor to model the attack

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

##### F8: No reference formula to compare against

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

##### Summary

| Bug | What FuzzMing would need | What it has |
|---|---|---|
| M-01 | An output that differs between buggy and correct code | Both paths return 0: nothing to distinguish |
| L-02 | The chain's block time to compare `MIN_SECONDS_AGO` against | Only the contract source: no external chain data |
| L-03 | Two actors and a baseline fee before the attack | One neutral actor, no pre-attack baseline |
| F8 | An independent correct reference implementation of the TWAP formula | Only the contract's own computation |

---

### invgen

#### Strengths

| Strength | Evidence from this case study |
|---|---|
| Strong LLM detection: correct identification without running code | 5 of 6 unique root causes correctly targeted through Chain-of-Thought analysis alone |
| Low cost | $2.15 for 59 LLM calls covering all public functions |
| Cache-based: no repeated API spend | All 59 responses cached; re-running invgen costs $0 if the contract has not changed |
| Works from contract source alone | No prior test suite or handler required for the identification phase |

#### Limitations

##### No handler: four problems, four fixes, 1 unique bug confirmed

invgen generates raw invariant functions with no handler. The original run had four problems that were investigated and corrected one by one:

1. **solc compatibility**: fixed by switching to `forge build` and adding `pragma abicoder v2`.
2. **1500-token limit**: bypassed by extracting complete invariants from the LLM cache manually.
3. **Access control (100% reverts)**: partially addressed by passing `address(this)` as `swapFeeManager`. However, Foundry's fuzzer still picked random senders most of the time, keeping the revert rate near 100%.
4. **Missing `targetSender`**: the key fix, adding `targetSender(address(this))` restricted the fuzzer to only use the authorized address as caller.

After all four fixes, the revert rate dropped to ~70% and **both Bug 2 invariants fired**. But only Bug 2 was confirmed. Three structural problems still blocked the remaining bugs:

- **Unregistered pool addresses**: `setFeeCap`, `setInitialFee`, `setScalingFactor` accept only registered pool addresses. The fuzzer picks random addresses for the pool parameter; ~70% of calls still revert for this reason.
- **Oracle state never driven**: `getFee` returns `baseFee + absTickDelta × scalingFactor / precision`. With MockPool's cumulative ticks fixed at zero, `absTickDelta = 0` and `getFee ≈ 0` regardless of caps. Bug 1 requires simulated price movement to surface.
- **No ghost variables**: the invariants can only check values readable from the contract's own storage. They cannot track "what feeCap was set for this pool" or compare fees across two actors with different `tx.origin` values.

##### solc vs. forge: compilation fails on forge-specific projects

invgen compiles generated code using plain `solc --standard-json`, not `forge build`. Any project that uses forge-specific constructs (`vm.*`, overriding `public virtual` with `internal`, missing `abicoder v2` pragmas) will fail at the project compilation step: before the generated invariants are even reached. On the slipstream project, every compilation attempt failed on pre-existing test files, not on the generated code.

##### 1500-token hardcoded completion limit

invgen hardcodes `max_tokens: 1500` per LLM call. Generating a complete Solidity invariant contract: including mock contracts, `setUp`, and multiple invariant functions: requires more than 1500 tokens. **35 of 59 calls hit this limit**, producing truncated Solidity that could not compile. The fix is a single line change: `max_tokens: 0` (unlimited) or a value >= 4096.

##### Structural assembly bug: prompt and code disagree on output format

The LLM prompt asks the model to "finish writing this contract and return the **whole** contract." The assembly code expects the LLM to return only **function bodies** to be inserted inside the existing setup contract. The LLM correctly followed the prompt and returned a full contract; the assembly code then produced two contracts nested inside each other: invalid Solidity. The 10-retry loop asked the LLM to "fix this syntax error" but the error was in invgen's own assembly, not in the generated code. No LLM retry can fix a mismatch in the calling code.

##### Generation-only pipeline: fuzzing is a manual step

invgen stops after writing the invariant file. The user must separately run `forge test` or `ityfuzz`. FuzzMing runs generation, fuzzing, and reporting in a single command with no manual step.

##### No ghost variables: invariants limited to the contract's own view functions

invgen's invariants can only assert things visible through the contract's public view functions. They cannot track state across a multi-call sequence. For example, invgen can check `getFee() <= defaultFeeCap()`, but not `getFee() <= the specific feeCap that was set for this pool during this sequence`: because it has no parallel model of expected state.

##### Summary

| Limitation | Impact on this run |
|---|---|
| No handler by default | Original run: 100% reverts. Four fixes applied: forge build, manual extraction, `address(this)` as manager, `targetSender`. Result: revert rate dropped to ~70%, both Bug 2 invariants fired. Remaining issues: unregistered pool addresses, no oracle state, no ghost variables. |
| `solc` instead of `forge` | Every compilation attempt failed on project test files before generated code was reached |
| 1500-token limit | 35 / 59 LLM responses truncated; generated Solidity incomplete |
| Assembly / prompt mismatch | All 10 retries per invariant failed; 0 invariants compiled through invgen's own pipeline |
| Generation-only | User must run fuzzing manually after generation completes |
| No ghost variables | Invariants limited to single-state assertions on view functions |

---

## Conclusion

Five independent methods were run against a 161-line contract. The five methods together found 11 distinct root causes with zero false positives; invgen detected 5 of 6 unique root causes through LLM analysis but confirmed only 1 unique bug due to toolchain and structural limitations. Two invariants fired but for the same root cause, which counts as 1. No single method found everything.

**Shieldify** uniquely found 3 findings requiring context outside the contract source: a wrong chain constant, an adversarial multi-actor spot-price attack, and a redundant check whose bug is masked at runtime by a try/catch. These are unreachable by any automated tool without external context.

**Claude Web** uniquely found 1 finding: the TWAP oracle math precision issue (F8), which requires an independent reference implementation of Uniswap V3 integer division to identify. It also confirmed all FuzzMing bugs from a static reasoning angle, including the `tx.origin` design flaw and the `setScalingFactor(0)` entry-point gap: but provides no reproduction path for any of them.

**Claude Code** confirmed 5 of 6 FuzzMing unique root causes through a hand-crafted property suite and identified two sub-issues (the L172 `baseFee` bypass and the constructor gap) that FuzzMing folds into broader findings. It missed Bug 5 because the ordering invariant `fee_discounted ≤ fee_undiscounted` was never encoded as a property.

**FuzzMing** confirmed 7 findings across 6 unique root causes. Bugs 2 and 7 share the same root cause and fix; the remaining 6 bugs are distinct. FuzzMing found 5 root causes that Shieldify missed, all rooted in state interactions that look correct function-by-function. It uniquely confirmed Bug 5 (fee inversion under specific state combinations): a cross-path ordering property that property-based fuzzing only finds if explicitly written. Every finding is backed by a shrunk Forge call sequence and a Solidity invariant usable directly as a CI regression test.

**invgen** generated invariants targeting 5 of 6 unique root causes in ~20 minutes at $2.15. Four problems were identified and fixed: switching to `forge build`, adding `pragma abicoder v2`, passing `address(this)` as the authorized manager, and adding `targetSender(address(this))`. With all four fixes applied, Bug 2 was confirmed by two invariants. The remaining bugs were not found for three reasons that persist after all fixes: pool-specific calls still revert ~70% of the time because the fuzzer picks random pool addresses that are not registered; oracle state is never driven so `getFee` always returns near zero regardless of caps; and there is no two-actor mechanism to compare discounted vs non-discounted fees. Confirming the remaining 5 unique root causes would require a full handler that uses the correct pool address, drives oracle state, and supports multi-actor scenarios.

The right strategy is to combine methods: FuzzMing for combinatorial state bugs with reproducible proof, LLM static analysis for broad surface coverage, and a professional audit for threat-model and chain-specific review.
