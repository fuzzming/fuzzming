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

| ID | Title | Severity | Status | FuzzMing | Note |
|---|---|---|---|---|---|
| M-01 | Cardinality pre-check uses wrong variable and is redundant | Medium | Fixed | Identified, not confirmed | AI analysis flagged it; no invariant fired — the try/catch already handles it so there is no observable violation |
| L-01 | Discount rounds up in favour of user | Low | Acknowledged | Identified, not confirmed | AI analysis named the exact bug; invariant written but never fired — `tx.origin` in Foundry view invariants is always the test contract, not the discounted address |
| L-02 | `MIN_SECONDS_AGO` hardcoded wrong for BNB Chain block time | Low | Acknowledged | Not found | FuzzMing has no knowledge of BNB Chain's block time — it treated `MIN_SECONDS_AGO = 2` as correct by copying the contract constant |
| L-03 | `currentTick` from `slot0` manipulable via spot price | Low | Acknowledged | Not found | Requires adversarial multi-transaction ordering (attacker moves price before victim swap); invariant fuzzing with a single actor cannot model this |
| L-04 | `resetDynamicFee` does not reset `baseFee` | Low | Acknowledged | **Confirmed — Bug 4** | Forge call sequence captured |
| I-01 | Wrong NatSpec in `calculateGrowth()` | Info | Fixed | Out of scope | Different contract |
| I-02 | `MINIMUM_LIQUIDITY` check blocks small depositors | Info | Acknowledged | Out of scope | Different contract |

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

## Why We Missed Them — And How FuzzMing Could Be Enhanced

### L-01 — Discount rounding (identified but not confirmed)

**Why we missed it**

The discount check in the contract reads:
```solidity
if (discounted[tx.origin] > 0) {
    uint256 discount = FullMath.mulDivRoundingUp(...);
    totalFee = totalFee - discount;
}
```

`tx.origin` is the original human wallet that started the transaction. When a user swaps through a DEX, `tx.origin` is their wallet address.

In Foundry invariant tests, when the test contract calls `target.getFee()`, Foundry puts the **test contract's own address** as `tx.origin` — not any real user wallet. The test contract is never registered in the `discounted` mapping, so `discounted[tx.origin]` is always 0, and the entire discount block is skipped every single time. More rounds do not fix this — the same wrong address is used in round 1 and round 1000.

**How we could fix it**

The solution is to call `getFee` from inside a handler function instead of from the invariant directly. Foundry has a two-argument version of `vm.prank(sender, origin)` that lets you set both the direct caller and `tx.origin` to the same discounted address. The handler would:
1. Call `vm.prank(discountedAddress, discountedAddress)`
2. Call `target.getFee(pool)` and store the result in a ghost variable
3. The invariant then checks the ghost variable rather than calling `getFee` itself

FuzzMing already has Rule 21 in its prompt that instructs the AI to use this pattern whenever `tx.origin` is detected in the source. This run did not fully trigger the pattern but the infrastructure is in place — a future run on the same contract would attempt it.

---

### M-01 — Cardinality check uses wrong variable (identified but not confirmed)

**Why we missed it**

The buggy line is:
```solidity
if (observationCardinality < _secondsAgo / MIN_SECONDS_AGO) return 0;
```

The bug: it uses `observationCardinality` (number of observation slots allocated) instead of `observationIndex` (number of observations actually written), and even the correct variable wouldn't reliably predict whether `observe()` reverts.

But immediately after this line:
```solidity
try ICLPool(_pool).observe(sa) returns (...) {
    // success
} catch {
    return 0;  // handles the failure anyway
}
```

Think of it as a faulty alarm on a door, but the door itself is still locked. Even if the alarm fires at the wrong time or not at all, you still can't open the door. The bug in the alarm is real, but it has no visible consequence because the lock always works correctly.

FuzzMing cannot write an invariant that fails here because there is no state where the buggy pre-check produces a different outcome than if it were correct. The try/catch absorbs every case.

**How we could fix it**

This class of bug — a redundant check that uses the wrong variable — is not detectable through property fuzzing at all. It requires **static analysis**: reading the code structure and spotting that two consecutive code paths produce identical results. FuzzMing could add a static analysis step that flags: "this early-return condition and the catch block below it return the same value — the condition is redundant." That would be a new capability beyond fuzzing, closer to a code linter or formal verifier. Integrating a tool like Slither or Semgrep as a pre-analysis step would catch this class of finding.

---

### L-02 — `MIN_SECONDS_AGO` wrong for BNB Chain (not found)

**Why we missed it**

The contract has:
```solidity
uint32 public constant MIN_SECONDS_AGO = 2;
```

The comment says *"it must be set to the block time."* BNB Chain's block time is 0.45 seconds, not 2. This constant is used to calculate observation thresholds — getting it wrong makes the cardinality check wildly inaccurate for BNB.

FuzzMing copied this constant directly from the contract and used it in tests. It had no way to know the constant was wrong because that knowledge lives outside the contract — it's a fact about the real-world blockchain this code will run on. The fuzzer tests the contract as written, not as it should be written for a specific chain.

**How we could fix it**

FuzzMing needs **chain context as input**. Concretely:

- Add a `--chain` flag (`fuzzming run --chain bnb`) that loads known parameters: block time, block gas limit, oracle cardinality patterns for typical pools on that chain
- When chain context is provided, the AI analysis stage could compare hardcoded constants against the known chain values and flag mismatches
- The LLM already has knowledge of major chains' properties — it just needs to be told which chain to validate against

This would be a small configuration addition that unlocks an entire class of chain-specific finding.

---

### L-03 — `slot0` price manipulation (not found)

**Why we missed it**

The dynamic fee formula computes:
```
fee = |currentTick - twAvgTick| × scalingFactor
```

`currentTick` is the spot price right now. `twAvgTick` is the 10-minute average price. Normally they are close and the dynamic fee is small. But an attacker can execute a large swap in the same block as a victim's swap, pushing `currentTick` far from `twAvgTick`, which inflates the fee the victim pays. The attacker loses money on the swap but causes the victim to overpay.

This attack requires two actors: an attacker who moves the price, and a victim who swaps right after. FuzzMing's invariant testing works by one actor calling functions randomly and checking that properties hold. It does not model "actor A deliberately tries to hurt actor B." No matter how many rounds run, the single-actor model cannot discover this.

**How we could fix it**

This requires a fundamentally different testing mode: **adversarial multi-actor scenario testing**. Concretely:

- Generate a second actor (the attacker) whose explicit goal is to maximize fees for the victim
- The attacker gets handler functions that move pool state adversarially: large swaps, price manipulation, front-running
- The victim actor then executes a swap and checks they did not pay above a reasonable fee threshold
- This is closer to a game-theoretic simulation than a property invariant

FuzzMing could add a `griefing` mode that generates this two-actor pattern. The AI would be prompted to think in terms of "what can actor A do before actor B's transaction to cause B to pay more/receive less?" This is a significant product extension — essentially adding economic attack simulation on top of property testing.

---

## Conclusion

On a 161-line contract, in 15 minutes, at $2.32:

- FuzzMing confirmed **1 of 5** Shieldify severity findings (L-04)
- FuzzMing found **3 additional bugs Shieldify missed**, all with reproducible call sequences
- FuzzMing produced **0 false positives**
- Every finding includes a **shrunk Forge call sequence** usable directly as a regression test

The professional audit excels at findings that require chain-specific knowledge (L-02), adversarial ordering reasoning (L-03), and subtle economic design review (L-01). FuzzMing excels at state interaction bugs, missing validation, and cap-bypass paths that emerge from combinations of valid operations — bugs that are easy to miss in linear code review but obvious to a fuzzer.

Used together: the auditor covers the threat model and design review; FuzzMing covers state space and combinatorial path coverage.
