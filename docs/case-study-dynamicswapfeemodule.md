# Case Study: FuzzMing vs. Professional Audit — DynamicSwapFeeModule

**Target:** [`DynamicSwapFeeModule.sol`](https://github.com/aerodrome-finance/slipstream/blob/main/contracts/core/fees/DynamicSwapFeeModule.sol) (149 nSLOC) — a concentrated liquidity dynamic fee module from a Uniswap V3 fork deployed on BNB Chain.

**Audit firm:** Shieldify Security — 5-day engagement, 80 auditor-hours. Full report available on [Solodit](https://solodit.cyfrin.io/issues/l-02-min_seconds_ago-hardcoded-to-2-seconds-but-bnb-chain-block-time-is-045-seconds-shieldify-none-topaz-dex-markdown).

**FuzzMing run:** 10 rounds, 15 minutes wall clock, fully automated.

---

## At a Glance

| | Shieldify | FuzzMing |
|---|---|---|
| Time | 5 days / 80 hours | **15 minutes** |
| Cost | - | **$2.32** (LLM API) |
| Human hours | 80 | 0 |
| Findings | 5 (M+L) + 2 Info | **4** |
| Shieldify findings confirmed | — | 1 / 5 |
| Findings Shieldify missed | — | **3** |

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

- **Duration:** 15 minutes wall clock (10 rounds × ~90 seconds average per round)
- **Human effort:** 0 hours — fully automated from a single command
- **LLM API cost:** $2.32 (OpenRouter / Claude Sonnet 4.6, 10 rounds)

| LLM call | Prompt tokens | Completion tokens | Cost |
|---|---|---|---|
| Round 1 — analysis | 3,577 | 4,337 | $0.076 |
| Round 1 — bodies | 10,298 | 6,305 | $0.125 |
| Round 1 — config | 7,859 | 75 | $0.025 |
| Round 1 — security | 10,852 | 3,160 | $0.080 |
| Round 2 — generator | 14,033 | 3,131 | $0.089 |
| Round 2 — security | 20,445 | 3,491 | $0.114 |
| Round 3 — generator | 16,923 | 2,866 | $0.094 |
| Round 3 — security | 27,131 | 3,554 | $0.135 |
| Round 4 — generator | 25,510 | 3,977 | $0.136 |
| Round 4 — security | 39,810 | 4,361 | $0.185 |
| Round 5 — generator | 22,471 | 4,062 | $0.128 |
| Round 5 — security | 87,815 | 4,534 | $0.331 |
| Round 6 — generator | 44,068 | 4,295 | $0.197 |
| Rounds 7–10 — patch | ~90,000 | ~5,000 | $0.398 |
| **Total** | | | **$2.32** |

---

## Findings: Head-to-Head

### Shieldify findings

| ID | Title | Severity | Status | FuzzMing |
|---|---|---|---|---|
| M-01 | Cardinality pre-check uses wrong variable and is redundant | Medium | Fixed | Not confirmed |
| L-01 | Discount rounds up in favour of user | Low | Acknowledged | Not confirmed |
| L-02 | `MIN_SECONDS_AGO` hardcoded wrong for BNB Chain block time | Low | Acknowledged | Not found |
| L-03 | `currentTick` from `slot0` manipulable via spot price | Low | Acknowledged | Not confirmed |
| L-04 | `resetDynamicFee` does not reset `baseFee` | Low | Acknowledged | **Confirmed — Bug 4** |
| I-01 | Wrong NatSpec in `calculateGrowth()` | Info | Fixed | Out of scope |
| I-02 | `MINIMUM_LIQUIDITY` check blocks small depositors | Info | Acknowledged | Out of scope |

### FuzzMing findings (with call sequences)

| Bug | Invariant | Confirmed by | Shieldify |
|---|---|---|---|
| 1 | `invariant_getFeeNeverExceedsEffectiveFeeCap` | Forge call sequence | **Not found** |
| 2 | `invariant_defaultFeeCapZeroSilentlyZerosFee` | Forge call sequence | **Not found** |
| 3 | `invariant_initialFeeDoesNotExceedPoolFeeCap` | Forge call sequence | **Not found** |
| 4 | `invariant_resetDynamicFeeDoesNotRestoreZeroFeeIndicator` | Forge call sequence | L-04 |

---

## FuzzMing Findings — Detail

### Bug 1 — Per-pool `feeCap` silently overwritten when `scalingFactor == 0`

**Severity:** High (fee cap enforcement bypassed)

**Root cause:**
```solidity
uint256 feeCap = dfc.feeCap;           // reads per-pool feeCap (e.g. 140)
if (scalingFactor == 0) {
    scalingFactor = defaultScalingFactor;
    feeCap = defaultFeeCap;            // silently discards per-pool cap
}
totalFee = totalFee < feeCap ? totalFee : feeCap;  // uses wrong feeCap
```

When `scalingFactor` has never been set for a pool, `getFee` replaces the pool's explicit `feeCap` with the global `defaultFeeCap`, potentially allowing fees far above what the pool-level cap permits. A manager can call `setFeeCap(pool, 140)` and have it silently ignored if `scalingFactor` is unset.

**Reproducing call sequence:**
```
handle_setFeeCap(poolSeed, 140)
  → sets per-pool feeCap = 140, scalingFactor remains 0
  → getFee uses defaultFeeCap instead → can return up to 50000
```

---

### Bug 2 — `defaultFeeCap = 0` silently zeroes all fees

**Severity:** High (complete fee loss, no signal)

**Root cause:** `setDefaultFeeCap` allows `_defaultFeeCap = 0` (only checks `<= MAX_FEE_CAP`). When `defaultFeeCap = 0` and a pool's `scalingFactor == 0`, `getFee` sets `feeCap = 0` and then:
```solidity
totalFee = totalFee < feeCap ? totalFee : feeCap;
// uint256 comparison: any totalFee < 0 is false → totalFee = 0
```
Every pool using default config silently returns 0 fees — no `ZERO_FEE_INDICATOR`, no revert, no event.

**Reproducing call sequence:**
```
handle_setDefaultFeeCapZeroThenSetBaseFee(poolSeed, 297834)
  → setDefaultFeeCap(0) is accepted without revert
  → setCustomFee(pool, 297834) but getFee returns 0
```

---

### Bug 3 — `initialFee` can be set above the pool's `feeCap`

**Severity:** Medium (cap enforcement bypassed on pool launch)

**Root cause:** `setInitialFee` validates `_fee <= MAX_FEE_CAP` but does not check against the pool's own `feeCap`. The `initialFeeEnabled` early-return path returns the raw `initialFee` with no cap applied:
```solidity
if (dfc.initialFeeEnabled) {
    ...
    return uint24(_initialFee);   // no cap — can exceed feeCap
}
```

**Reproducing call sequence (5 steps):**
```
1. handle_setMockCurrentTick(3032)
2. handle_setMockObserveReverts(true)
3. handle_setZeroFeeIndicatorThenReset(18951)
4. handle_resetDynamicFee(12189)
5. handle_setInitialFeeAboveFeeCap(14782, feeCap=5877, initialFee=7923)
   → getFee returns 7923 against a pool capped at 5877
```

This finding was only reachable because FuzzMing generated a **fuzzable mock** with mutable `currentTick`, `observeReverts`, and observation state — steps 1 and 2 exercise the mock's variable behavior. A static mock would have made this path invisible.

---

### Bug 4 — `resetDynamicFee` preserves `ZERO_FEE_INDICATOR`, permanently zeroing fees

**Severity:** Medium (L-04 confirmed) — matches Shieldify's finding

**Root cause:** `resetDynamicFee` deletes `feeCap`, `scalingFactor`, `initialFeeEnabled`, and `initialFee`, but **not** `baseFee`. If `baseFee = ZERO_FEE_INDICATOR (420)` was previously set, the reset leaves the pool silently returning 0 fees:
```solidity
if (baseFee == ZERO_FEE_INDICATOR) return 0;   // fires before any computation
```

**Reproducing call sequence (1 step):**
```
handle_resetDynamicFeeAfterZeroFeeIndicator(19392)
  → setCustomFee(pool, ZERO_FEE_INDICATOR=420)
  → resetDynamicFee(pool)
  → getFee(pool) == 0  (baseFee retained, ZERO_FEE_INDICATOR still active)
```

---

## What Each Approach Missed

### Shieldify found, FuzzMing did not confirm

| Finding | Why FuzzMing missed it |
|---|---|
| M-01 — Wrong cardinality variable | The pre-check is redundant (try/catch handles it anyway); no observable invariant violation occurs in practice |
| L-01 — Discount rounds up | The `tx.origin` pattern cannot be tested from Foundry `view` invariants; requires `vm.prank(actor, actor)` + ghost state — partial support added but not confirmed this run |
| L-02 — `MIN_SECONDS_AGO` wrong for BNB | Chain-specific configuration knowledge; fuzzing cannot discover that a constant is wrong for a specific deployment chain without external context |
| L-03 — `slot0` manipulation | Requires adversarial multi-transaction sequencing where an attacker moves spot price before a victim swap; invariant fuzzing with a single actor cannot model this threat |

### FuzzMing found, Shieldify did not

| Finding | Root cause class |
|---|---|
| Bug 1 — `feeCap` overwritten when `scalingFactor == 0` | Silent state override — hard to spot in manual review because the code path is correct individually but interacts badly with unset scalingFactor |
| Bug 2 — `defaultFeeCap = 0` zeroes fees | Missing lower-bound validation — `require > 0` omitted; easy to miss because 0 passes the `<= MAX_FEE_CAP` check |
| Bug 3 — `initialFee` above pool `feeCap` | Missing cross-parameter validation — `setInitialFee` validates against `MAX_FEE_CAP` but not against the pool's own cap; required fuzzable mock state to discover |

---

## Call Sequences: A Structural Advantage

Every FuzzMing finding comes with a **Forge-generated, shrunk call sequence** that reproduces the bug deterministically. Auditors typically report root causes without a reproduction path; FuzzMing reports both.

**Bug 4 reproduction — one line:**
```
handle_resetDynamicFeeAfterZeroFeeIndicator(19392)
```

**Bug 3 reproduction — five steps:**
```
handle_setMockCurrentTick(3032)
handle_setMockObserveReverts(true)
handle_setZeroFeeIndicatorThenReset(18951)
handle_resetDynamicFee(12189)
handle_setInitialFeeAboveFeeCap(14782, 5877, 7923)
```

These sequences can be dropped directly into a Foundry test, run in CI, and used to verify that a fix is correct — without any manual reproduction effort.

---

## Remaining Limitations

FuzzMing is a complementary tool, not a full replacement for human audit. The gaps visible in this run:

| Limitation | Class |
|---|---|
| Chain-specific constants (L-02) | Requires deployment context — fuzzing is chain-agnostic |
| Adversarial transaction ordering (L-03) | Requires multi-actor sandwich simulation beyond single-round invariant testing |
| `tx.origin` paths not fully covered | Foundry view invariants cannot set `tx.origin`; requires ghost pattern, partially implemented |
| 5 of 10 rounds lost to compile errors | ASCII rule violated in round 6; cascading errors in rounds 7–10 |

---

## Conclusion

On a 161-line contract, in 15 minutes, at $2.32:

- FuzzMing confirmed **1 of 5** Shieldify severity findings (L-04)
- FuzzMing found **3 additional bugs Shieldify missed**, all with reproducible call sequences
- FuzzMing produced **0 false positives**
- Every finding includes a **shrunk Forge call sequence** usable directly as a regression test

The professional audit excels at findings that require chain-specific knowledge (L-02), adversarial ordering reasoning (L-03), and subtle economic design review (L-01). FuzzMing excels at state interaction bugs, missing validation, and cap-bypass paths that emerge from combinations of valid operations — bugs that are easy to miss in linear code review but obvious to a fuzzer.

Used together: the auditor covers the threat model and design review; FuzzMing covers state space and combinatorial path coverage.
