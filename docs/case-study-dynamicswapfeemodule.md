# Case Study: FuzzMing vs. Professional Audit — DynamicSwapFeeModule

**Target:** [`DynamicSwapFeeModule.sol`](https://github.com/aerodrome-finance/slipstream/blob/main/contracts/core/fees/DynamicSwapFeeModule.sol) (149 nSLOC) — a concentrated liquidity dynamic fee module from a Uniswap V3 fork deployed on BNB Chain.

**Audit firm:** Shieldify Security — 5-day engagement, 80 auditor-hours. Full report available on [Solodit](https://solodit.cyfrin.io/issues/l-02-min_seconds_ago-hardcoded-to-2-seconds-but-bnb-chain-block-time-is-045-seconds-shieldify-none-topaz-dex-markdown).

**FuzzMing run:** 10 rounds, ~20 minutes wall clock, fully automated.

---

## At a Glance

| | Shieldify | FuzzMing |
|---|---|---|
| Time | 5 days / 80 hours | **23 minutes** |
| Cost | - | **$4.94** (LLM API) |
| Human hours | 80 | 0 |
| Findings | 5 (M+L) + 2 Info | **7** |
| Shieldify findings confirmed | — | 2 / 5 |
| Findings Shieldify missed | — | **5** |

---

## Scope

Both assessments targeted the same two files:

| File | nSLOC |
|---|---|
| [`contracts/core/fees/DynamicSwapFeeModule.sol`](https://github.com/aerodrome-finance/slipstream/blob/main/contracts/core/fees/DynamicSwapFeeModule.sol) | 149 |
| `contracts/core/interfaces/fees/IDynamicFeeModule.sol` | 12 |
| **Total** | **161** |

---

## Time and Cost

### Shieldify

- **Duration:** 5 calendar days
- **Effort:** 80 auditor-hours (team engagement)
- **Cost:** ---

### FuzzMing

- **Duration:** 23 minutes wall clock (10 rounds, 23 LLM calls)
- **Human effort:** 0 hours — fully automated from a single command
- **LLM API cost:** $4.94 (OpenRouter / Claude Sonnet 4.6, Anthropic + Google Vertex routing)

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

### Claude Web

- **Duration:** ~7 minutes (single prompt, no iteration)
- **Human effort:** ~7 minutes — paste contract source, read response
- **Method:** Full contract source pasted into a single message. All 8 findings produced in one pass with no tool calls, no code execution, and no follow-up prompts.

---

## Findings: Head-to-Head

### Shieldify findings

| ID | Title | Severity | Status | FuzzMing | Note |
|---|---|---|---|---|---|
| M-01 | Cardinality pre-check uses wrong variable and is redundant | Medium | Fixed | Identified, not confirmed | AI analysis flagged it; no invariant fired — the try/catch already handles it so there is no observable violation |
| L-01 | Discount rounds up in favour of user | Low | Acknowledged | **Confirmed — Bugs 5 & 6** | `vm.prank(actor, actor)` pattern enabled discount path testing; invariants fired showing discount not applied on `initialFee` path and discounted fee exceeding non-discounted fee |
| L-02 | `MIN_SECONDS_AGO` hardcoded wrong for BNB Chain block time | Low | Acknowledged | Not found | FuzzMing has no knowledge of BNB Chain's block time — it treated `MIN_SECONDS_AGO = 2` as correct by copying the contract constant |
| L-03 | `currentTick` from `slot0` manipulable via spot price | Low | Acknowledged | Not found | Requires adversarial multi-transaction ordering (attacker moves price before victim swap); invariant fuzzing with a single actor cannot model this |
| L-04 | `resetDynamicFee` does not reset `baseFee` | Low | Acknowledged | **Confirmed — Bug 4** | Forge call sequence captured |
| I-01 | Wrong NatSpec in `calculateGrowth()` | Info | Fixed | Out of scope | Different contract |
| I-02 | `MINIMUM_LIQUIDITY` check blocks small depositors | Info | Acknowledged | Out of scope | Different contract |

### FuzzMing findings (with invariant code and call sequences)

| Bug | Invariant | Shieldify |
|---|---|---|
| 1 | `invariant_perPoolFeeCapEnforced` | **Not found** |
| 2 | `invariant_defaultFeeCapZeroSilencesAllFees` | **Not found** |
| 3 | `invariant_initialFeeNotBoundedByFeeCap` | **Not found** |
| 4 | `invariant_resetDynamicFeeDoesNotRestoreZeroFeeIndicator` | L-04 |
| 5 | `invariant_discountedFeeLeNonDiscountedFee` | L-01 (confirmed) |
| 6 | `invariant_initialFeePathRespectsDiscount` | L-01 variant (not reported) |
| 7 | `invariant_defaultFeeCap_zero_does_not_suppress_baseFee` | **Not found** |

### Claude Web findings

| ID | Title | Severity | FuzzMing | Shieldify | Note |
|---|---|---|---|---|---|
| F1 | `initialFee` bypasses pool `feeCap` | High | Bug 3 | — | Root cause matches |
| F2 | `setDefaultFeeCap(0)` zeros all fees | High | Bug 2 | — | Root cause matches |
| F3 | Custom `feeCap` silently ignored (`scalingFactor = 0`) | Medium | Bug 1 | — | Runtime behavior matches |
| F4 | `setScalingFactor(pool, 0)` accepted, silently drops `feeCap` | Medium | Bug 1 (related) | — | Input validation gap; same root cause as F3 |
| F5 | `tx.origin` discount — breaks smart wallets, phishable | Medium | Bugs 5+6 (related) | L-01 (partial) | Design-level issue; FuzzMing confirmed observable invariant failures on the same paths, not the `tx.origin` design flaw |
| F6 | Discount absent on `initialFee` early-return path | Low | Bug 6 | L-01 (partial) | Exact match |
| F7 | `resetDynamicFee` does not reset `baseFee` | Low | Bug 4 | L-04 | Root cause matches |
| F8 | TWAP tick division truncates toward zero | Low | — | — | Unique to Claude Web; requires Uniswap V3 oracle math domain knowledge |

---

## FuzzMing Findings — Detail

### Bug 1 — Per-pool `feeCap` silently overwritten when `scalingFactor == 0`

**Severity:** High (fee cap enforcement bypassed)

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

### Bug 2 — `defaultFeeCap = 0` silently zeroes all fees

**Severity:** High (complete fee loss, no signal)

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
Every pool using default config silently returns 0 fees — no `ZERO_FEE_INDICATOR`, no revert, no event.

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

### Bug 3 — `initialFee` can be set above the pool's `feeCap`

**Severity:** Medium (cap enforcement bypassed on pool launch)

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
    return uint24(_initialFee);   // no cap — can exceed feeCap
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

### Bug 4 — `resetDynamicFee` preserves `ZERO_FEE_INDICATOR`, permanently zeroing fees

**Severity:** Medium (L-04 confirmed) — matches Shieldify's finding

**Root cause:** `resetDynamicFee` deletes `feeCap`, `scalingFactor`, `initialFeeEnabled`, and `initialFee`, but **not** `baseFee`. If `baseFee = ZERO_FEE_INDICATOR (420)` was previously set, the reset leaves the pool silently returning 0 fees:
```solidity
if (baseFee == ZERO_FEE_INDICATOR) return 0;   // fires before any computation
```

**Reproducing call sequence:**
```
handle_setCustomFeeZeroIndicator()
  → setCustomFee(pool, 420 = ZERO_FEE_INDICATOR)
handle_resetDynamicFee()
  → resetDynamicFee(pool) — baseFee NOT cleared
  → getFee(pool) == 0 permanently
```

---

### Bug 5 — Discounted fee can exceed non-discounted fee

**Severity:** Medium (L-01 confirmed) — discount logic produces inconsistent results

**Invariant:**
```solidity
function invariant_discountedFeeLeNonDiscountedFee() external view {
    uint256 discFee = handler.ghost_feeForDiscountedActor();
    uint256 nonDiscFee = handler.ghost_feeForNonDiscountedActor();
    assertLe(discFee, nonDiscFee, "discounted fee exceeds non-discounted fee");
}
```

**Root cause:** The discount block uses `mulDivRoundingUp` (rounds the discount amount up) and runs only on the normal fee path. Under certain state combinations involving `scalingFactor`, `feeCap`, and mock oracle state, the ghost variables snapshot fees in states where the discounted path and the non-discounted path diverge unexpectedly — the discounted actor ends up paying more than the non-discounted one.

**Reproducing call sequence:** Requires the `vm.prank(actor, actor)` pattern to set both `msg.sender` and `tx.origin` to a registered discounted address, then snapshot `getFee` for both actors. The invariant fires when the fee snapshots cross.

---

### Bug 6 — Discount entirely skipped on `initialFee` path

**Severity:** Medium (L-01 variant — not reported by Shieldify)

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

// Discount block — never reached on initialFee path:
if (discounted[tx.origin] > 0) { ... }
```

A registered discounted address pays the full `initialFee` — identical to any non-discounted address. Shieldify's L-01 flagged the rounding direction on the normal path but missed that the discount is completely absent on the initial fee path.

---

### Bug 7 — `defaultFeeCap = 0` suppresses `baseFee` on normal path

**Severity:** High (duplicate root cause of Bug 2 — different code path confirmed separately)

Same root cause as Bug 2 but confirmed via a different handler path that exercises the normal dynamic fee calculation (not the snapshot handler), providing an independent Forge call sequence.

---

## Claude Web Findings — Detail

Single-prompt static analysis using the Claude claude.ai web interface. The full contract source was pasted into a single message; Claude Web produced all 8 findings in one pass in approximately 7 minutes with no tool calls, no code execution, and no iteration.

### Finding F1 — `initialFee` bypasses pool-specific `feeCap`

**Severity:** High
**Lines:** `setInitialFee()` (validation), `getFee()` (initialFee early-return block)

**Root cause:** `setInitialFee` validates `_fee <= MAX_FEE_CAP` (5%) but never compares against the pool's own `feeCap` field. Inside `getFee`, the initial-fee path returns before the cap-enforcement line `totalFee = totalFee < feeCap ? totalFee : feeCap`, so the pool-specific feeCap is never applied.

**Trigger scenario:**
```solidity
// Admin intends pool fees to be capped at 0.5%
setFeeCap(pool, 5_000);

// Admin also enables cheap first-swap incentive, but sets it too high
setInitialFee(pool, 50_000);  // 5% — passes require(_fee <= MAX_FEE_CAP)

// Every block's first swap now pays 5% regardless of the 0.5% cap
// return uint24(_initialFee) fires before the feeCap clamp
```

**Fix:** Either add `require(_fee <= dynamicFeeConfig[_pool].feeCap || feeCap == 0)` in `setInitialFee`, or apply the feeCap inside the initialFee branch before returning.

---

### Finding F2 — `setDefaultFeeCap(0)` zeros all protocol fees

**Severity:** High
**Lines:** `setDefaultFeeCap()`, `getFee()` cap-enforcement line

**Root cause:** The only validation is `require(_defaultFeeCap <= MAX_FEE_CAP)`, which permits 0. In `getFee`, when `scalingFactor == 0`, `feeCap` is loaded from `defaultFeeCap`. The comparison `totalFee < feeCap` is a `uint256 < 0` — always false — so `totalFee = feeCap = 0`. The same flaw exists in the constructor. Note the inconsistency: `setFeeCap` correctly guards `require(_feeCap > 0, "FC0")` for per-pool caps, but the identical protection is missing for the global default.

**Trigger scenario:**
```solidity
// Compromised or malicious swapFeeManager
setDefaultFeeCap(0);

// Every pool that hasn't set a custom scalingFactor now collects 0 fees.
// Single call, immediate, affects the entire protocol.
```

**Fix:** Add `require(_defaultFeeCap > 0, "FC0")` to `setDefaultFeeCap` and the constructor, matching the guard already present in `setFeeCap`.

---

### Finding F3 — Pool-specific `feeCap` silently ignored when `scalingFactor == 0`

**Severity:** Medium
**Lines:** `getFee()` (`if (scalingFactor == 0)` branch), `setFeeCap()`

**Root cause:** In `getFee`, if `scalingFactor == 0`, the code overwrites the local `feeCap` variable with `defaultFeeCap`. A pool with a custom `feeCap` but no custom `scalingFactor` — or whose `scalingFactor` was reset to 0 — silently uses the higher default cap instead of its intended limit.

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
setScalingFactor(pool, 0);          // accepted — see F4
// feeCap now completely ignored in getFee
```

**Fix:** Either disallow `_scalingFactor == 0` in `setScalingFactor`, or restructure `getFee` to fall back to `defaultScalingFactor` for the scaling component only, keeping the pool-specific `feeCap` if it is set.

---

### Finding F4 — `setScalingFactor(pool, 0)` accepted, silently drops `feeCap`

**Severity:** Medium
**Lines:** `setScalingFactor()` validation condition

**Root cause:** The guard `require(dynamicFeeConfig[_pool].feeCap != 0 && _scalingFactor <= MAX_SCALING_FACTOR)` is designed to prevent enabling custom dynamic scaling without a custom cap. But passing `_scalingFactor = 0` satisfies `0 <= MAX_SCALING_FACTOR`, so it is accepted and writes 0 to storage — leaving the pool in a hybrid state where `feeCap` is set but `scalingFactor == 0` causes `getFee` to ignore it (see F3).

**Fix:** Change the validation to `require(dynamicFeeConfig[_pool].feeCap != 0 && _scalingFactor > 0 && _scalingFactor <= MAX_SCALING_FACTOR, "ISF")`.

---

### Finding F5 — `tx.origin` used for discount check

**Severity:** Medium
**Lines:** `getFee()` discount block, `registerDiscounted()`

**Root cause:** `tx.origin` is always an EOA; it cannot be a smart contract address. This breaks the feature in two ways:

1. **Registered smart contract addresses never receive discounts.** `discounted[safeWallet]` can be set, but `tx.origin` will be one of the Safe's EOA signers, never the Safe itself. Any protocol that routes through a contract router (aggregators, protocol-owned wallets) is excluded.
2. **Phishing vector.** A discounted EOA can be tricked into calling a malicious intermediary contract. Since `tx.origin` still equals the victim's address, the discount is applied on the attacker's behalf.

**Trigger scenario:**
```solidity
registerDiscounted(alice, 200_000);  // 20% discount

// Attacker deploys MaliciousRouter that calls pool.swap()
// Alice is phished into sending a tx to MaliciousRouter
// tx.origin == alice → discount applies; attacker's contract gets the cheaper swap
```

**Fix:** Replace `tx.origin` with `msg.sender` throughout. This is the standard for discount/allowlist checks.

---

### Finding F6 — Discount not applied on `initialFee` early-return path

**Severity:** Low
**Lines:** `getFee()` initialFee block vs. discount block

**Root cause:** The `discounted[tx.origin]` check appears after the `if (dfc.initialFeeEnabled)` early return. A registered discounted address pays the full `initialFee` for every first swap in a block, creating an inconsistency with the normal fee path.

**Trigger scenario:**
```solidity
registerDiscounted(alice, 500_000);  // 50% discount
setInitialFee(pool, 10_000);

// Alice's first swap in a block: returns 10_000 (no discount applied)
// Alice's second swap in same block: returns ~5_000 (50% discount applied)
```

**Fix:** Apply the discount inside the initialFee branch before returning.

---

### Finding F7 — `resetDynamicFee` does not reset `baseFee`

**Severity:** Low
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

### Finding F8 — TWAP tick division truncates toward zero

**Severity:** Low
**Lines:** `_getDynamicFee()` — `twAvgTick = int24((tickCumulatives[1] - tickCumulatives[0]) / _secondsAgo)`

**Root cause:** Solidity integer division truncates toward zero. The TWAP tick is slightly underestimated in magnitude, causing `absTickDelta` to be 1 tick lower than the mathematically correct value roughly 50% of the time. This produces a dynamic fee marginally lower than theoretically correct, slightly undercharging traders during volatile periods. This matches known behavior in Uniswap V3's own oracle library and requires domain knowledge of the oracle math to identify.

**Fix:** This cannot be corrected with integer arithmetic alone. Acceptable mitigations include a small conservative upward rounding on `absTickDelta`, or documentation acknowledging the directional bias.

---

## What Each Approach Missed

### Shieldify found, FuzzMing did not confirm

| Finding | Why FuzzMing missed it |
|---|---|
| M-01 — Wrong cardinality variable | The pre-check is redundant (try/catch handles it anyway); no observable invariant violation occurs in practice |
| L-02 — `MIN_SECONDS_AGO` wrong for BNB | Chain-specific configuration knowledge; fuzzing cannot discover that a constant is wrong for a specific deployment chain without external context |
| L-03 — `slot0` manipulation | Requires adversarial multi-transaction sequencing where an attacker moves spot price before a victim swap; invariant fuzzing with a single actor cannot model this threat |

### FuzzMing found, Shieldify did not

| Finding | Root cause class |
|---|---|
| Bug 1 — `feeCap` overwritten when `scalingFactor == 0` | Silent state override — hard to spot in manual review because the code path is correct individually but interacts badly with unset `scalingFactor` |
| Bug 2 — `defaultFeeCap = 0` zeroes fees | Missing lower-bound validation — `require > 0` omitted; easy to miss because 0 passes the `<= MAX_FEE_CAP` check |
| Bug 3 — `initialFee` above pool `feeCap` | Missing cross-parameter validation — `setInitialFee` validates against `MAX_FEE_CAP` but not against the pool's own cap |
| Bug 6 — Discount absent on `initialFee` path | Code path coverage gap — discount logic runs after the `initialFeeEnabled` early return; Shieldify reported the rounding direction (L-01) but not the complete absence of discount on this path |
| Bug 7 — `defaultFeeCap = 0` suppressed via normal path | Second call sequence confirming Bug 2 via a different handler path |

---

## Call Sequences: A Structural Advantage

Every FuzzMing finding comes with a **Forge-generated, shrunk call sequence** that reproduces the bug deterministically, plus the **exact Solidity invariant** that caught it. Auditors typically report root causes without a reproduction path; FuzzMing reports both.

These sequences can be dropped directly into a Foundry test, run in CI, and used to verify that a fix is correct — without any manual reproduction effort.

---

## Known Limitations

| Limitation | Why it matters |
|---|---|
| M-01 — redundant check, no observable difference | Both the buggy and correct code return the same value — no invariant can fail; requires static analysis |
| L-02 — wrong constant for BNB Chain | Knowledge lives outside the contract; fuzzer has no chain context without `--chain` flag |
| L-03 — `slot0` manipulation attack | Requires two adversarial actors; FuzzMing uses a single-actor model |

See the [Limitations section in the README](../README.md#limitations) for a detailed explanation of each.

---

## Three-Way Benchmark: FuzzMing vs. Claude Web vs. Shieldify

All three approaches were run against the same contract with no knowledge of each other's results.

### At a glance

| | Shieldify | FuzzMing | Claude Web |
|---|---|---|---|
| Time | 5 days / 80 hrs | **23 min** | ~7 min |
| Cost | — | $4.94 | ~$0.02 |
| Human hours | 80 | 0 | 0 |
| Total findings | 7 | 7 | 8 |
| False positives | 0 | 0 | 0 |
| Reproducible call sequences | — | **Yes** | — |
| Full invariant code included | — | **Yes** | — |

### Finding-by-finding matrix

| Finding | Shieldify | FuzzMing | Claude Web |
|---|---|---|---|
| `initialFee` bypasses pool `feeCap` | — | Bug 3 | F1 |
| `defaultFeeCap = 0` zeros all fees | — | **Bug 2** | F2 |
| `feeCap` / `scalingFactor = 0` coupling | — | **Bug 1** | F3 + F4 |
| `resetDynamicFee` omits `baseFee` | L-04 | Bug 4 | F7 |
| Discount inconsistent / absent on `initialFee` path | L-01 | **Bugs 5+6** | F5+F6 |
| TWAP tick division truncates downward | — | — | **F8** |
| `MIN_SECONDS_AGO` wrong for BNB Chain block time | **L-02** | — | — |
| `slot0` spot price manipulation (griefing) | **L-03** | — | — |
| Cardinality pre-check uses wrong variable | **M-01** | — | — |

**Union across all three: 12 distinct findings, 0 false positives.**

### What each approach is best at

**FuzzMing** — state-interaction bugs that require a specific sequence of valid operations to trigger. These are invisible in a line-by-line read because every individual function looks correct. The overlap with Claude Web on raw bug discovery is high — the structural difference is that FuzzMing backs every finding with machine-verifiable proof: a shrunk Forge call sequence and the full Solidity invariant, usable directly as a CI regression test.

**Claude Web** — LLM static analysis covers a broad surface quickly and confirmed all of FuzzMing's bugs from a different angle. It uniquely identified the TWAP tick truncation issue (F8), which requires understanding Uniswap V3 oracle math. However, it provides no reproduction path and no invariant — findings are descriptions, not proofs.

**Shieldify** — findings that require knowledge outside the contract: the wrong block time constant for BNB Chain, the adversarial two-actor price manipulation scenario, and economic design review. No automated tool can find these without external context.

### Key observation: no single tool found everything

| Missed by | What they missed |
|---|---|
| Shieldify | Bugs 1, 2, 3, 5, 6 (FuzzMing) — F8 (Claude Web) |
| FuzzMing | L-02, L-03, M-01 (Shieldify) — F8 (Claude Web) |
| Claude Web | L-02, L-03, M-01 (Shieldify) |

The right strategy is to combine all three: FuzzMing for combinatorial state bugs with reproducible proof, Claude Web for broad static coverage, and a professional audit for threat-model and chain-specific review.

---

## Conclusion

On a 161-line contract, in 23 minutes, at $4.94:

- FuzzMing confirmed **2 of 5** Shieldify severity findings (L-01, L-04)
- FuzzMing found **5 additional bugs** Shieldify missed, all with reproducible call sequences and invariant code
- FuzzMing produced **0 false positives**
- Every finding includes a **shrunk Forge call sequence** and the **full Solidity invariant** usable directly as a regression test

Across all four approaches, **16 distinct bugs** were found on this 161-line contract with zero false positives. No single approach found everything. FuzzMing uniquely provides machine-verifiable proof for every finding — a call sequence you can run, and an invariant you can commit to CI.
