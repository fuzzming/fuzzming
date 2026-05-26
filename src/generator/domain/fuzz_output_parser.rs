pub struct FailingInvariant {
    pub name: String,
    pub failure_message: String,
    pub call_sequence: Vec<String>,
}

/// Turn raw forge output into a compact, LLM-friendly summary.
///
/// Forge output already filtered to one contract's section by `filter_output`.
/// We further strip passing tests, box-drawing stack frames, sequence markers,
/// and the suite result line — leaving only the signal the LLM needs.
pub fn format_for_llm(raw: &str) -> String {
    // Pass compile errors and dev-test failures through — they are already
    // concise and specially formatted by run_fuzzer_session.
    if raw.starts_with("COMPILATION ERROR") || raw.starts_with("TEST FAILED") {
        return raw.to_string();
    }

    let failures = parse_failing_invariants(raw);

    if failures.is_empty() {
        return "All invariants passed.".to_string();
    }

    let mut out = format!("{} failing invariant(s):\n", failures.len());
    for f in &failures {
        out.push('\n');
        out.push_str(&format!("  {}\n", f.name));
        out.push_str(&format!("  Failure: {}\n", f.failure_message));
        if !f.call_sequence.is_empty() {
            out.push_str(&format!("  Call sequence ({} step(s)):\n", f.call_sequence.len()));
            for step in &f.call_sequence {
                out.push_str(&format!("    {}\n", step));
            }
        }
    }
    out.trim_end().to_string()
}

fn parse_failing_invariants(raw: &str) -> Vec<FailingInvariant> {
    let mut results: Vec<FailingInvariant> = Vec::new();
    let mut failure_message: Option<String> = None;
    let mut collecting_sequence = false;
    let mut sequence_lines: Vec<String> = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();

        // Stop at the duplicate summary block forge appends.
        if trimmed.starts_with("Failing tests:") || trimmed.starts_with("Suite result:") {
            break;
        }

        // Skip passing tests.
        if trimmed.starts_with("[PASS]") {
            continue;
        }

        // Skip box-drawing characters from assertion stack traces.
        if trimmed.starts_with('╭')
            || trimmed.starts_with('╰')
            || trimmed.starts_with('├')
            || trimmed.starts_with('│')
        {
            continue;
        }

        // New failure block: [FAIL: <assertion message>]
        if trimmed.starts_with("[FAIL") {
            failure_message = Some(extract_fail_message(trimmed));
            collecting_sequence = false;
            sequence_lines.clear();
            continue;
        }

        // Sequence marker line — start collecting call steps.
        if trimmed.contains("[Sequence]") || trimmed.contains("[Shrunk sequence]") {
            collecting_sequence = true;
            continue;
        }

        // Call step: sender=0x... calldata=fn() args=[...]
        if collecting_sequence && trimmed.starts_with("sender=") {
            sequence_lines.push(format_call_step(trimmed));
            continue;
        }

        // Summary line for this invariant: "invariant_name() (runs: N, ...)"
        if trimmed.contains("invariant_") && trimmed.contains("(runs:") {
            if let Some(msg) = failure_message.take() {
                if let Some(name) = extract_invariant_name(trimmed) {
                    results.push(FailingInvariant {
                        name,
                        failure_message: msg,
                        call_sequence: sequence_lines.clone(),
                    });
                }
            }
            collecting_sequence = false;
            sequence_lines.clear();
        }
    }

    results
}

/// Extract the assertion message from a [FAIL: <msg>] line.
fn extract_fail_message(line: &str) -> String {
    // "[FAIL: count should never exceed 100: 1000 > 100]"
    //   → "count should never exceed 100: 1000 > 100"
    let inner = line
        .trim_start_matches('[')
        .trim_end_matches(']');
    inner
        .trim_start_matches("FAIL:")
        .trim()
        .to_string()
}

/// Format a forge call step into a readable string.
///
/// Input:  "sender=0xAAA calldata=handler_deposit(uint256) args=[1000]"
/// Output: "handler_deposit(1000)  (sender: 0xAAA)"
fn format_call_step(line: &str) -> String {
    let sender = extract_field(line, "sender=", " ");
    let calldata = extract_field(line, "calldata=", " ");
    let args_raw = extract_field(line, "args=[", "]");

    let call = match (calldata, args_raw) {
        (Some(fn_sig), Some(args)) if !args.is_empty() => {
            // Replace the parameter types with the actual arg values.
            // "handler_deposit(uint256)" + "1000" → "handler_deposit(1000)"
            let base = fn_sig.split('(').next().unwrap_or(fn_sig);
            format!("{base}({args})")
        }
        (Some(fn_sig), _) => fn_sig.to_string(),
        _ => line.to_string(),
    };

    match sender {
        Some(s) => format!("{call}  (sender: {s})"),
        None => call,
    }
}

fn extract_field<'a>(line: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    let start = line.find(prefix)? + prefix.len();
    let rest = &line[start..];
    let end = rest.find(suffix).unwrap_or(rest.len());
    Some(&rest[..end])
}

fn extract_invariant_name(line: &str) -> Option<String> {
    let start = line.find("invariant_")?;
    let rest = &line[start..];
    let end = rest.find('(').unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORGE_OUTPUT: &str = "\
Ran 3 tests for test/fuzzming/Vault/VaultInvariantTest.sol:VaultInvariantTest
[FAIL: count should never exceed 100: 1000 > 100]
\t[Sequence] (original: 1, shrunk: 1)
\t\tsender=0xAAAA calldata=handler_bigJump(uint256) args=[9999]
 invariant_bounded() (runs: 1, calls: 1, reverts: 0)
[FAIL: solvency broken: 50 < 100]
\t[Shrunk sequence] (original: 5, shrunk: 2)
\t\tsender=0xBBBB calldata=handler_deposit(uint256) args=[100]
\t\tsender=0xBBBB calldata=handler_withdraw(uint256) args=[200]
 invariant_solvency() (runs: 2, calls: 2, reverts: 0)
[PASS] invariant_something() (runs: 256, calls: 512, reverts: 10)
Suite result: FAILED. 1 passed; 2 failed;";

    #[test]
    fn extracts_two_failures_skips_pass() {
        let failures = parse_failing_invariants(FORGE_OUTPUT);
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].name, "invariant_bounded");
        assert_eq!(failures[1].name, "invariant_solvency");
    }

    #[test]
    fn extracts_failure_messages() {
        let failures = parse_failing_invariants(FORGE_OUTPUT);
        assert!(failures[0].failure_message.contains("count should never exceed 100"));
        assert!(failures[1].failure_message.contains("solvency broken"));
    }

    #[test]
    fn formats_call_steps_with_args_and_sender() {
        let failures = parse_failing_invariants(FORGE_OUTPUT);
        assert_eq!(failures[0].call_sequence.len(), 1);
        assert!(failures[0].call_sequence[0].contains("handler_bigJump(9999)"));
        assert!(failures[0].call_sequence[0].contains("sender: 0xAAAA"));
    }

    #[test]
    fn multi_step_sequence_preserved() {
        let failures = parse_failing_invariants(FORGE_OUTPUT);
        assert_eq!(failures[1].call_sequence.len(), 2);
        assert!(failures[1].call_sequence[0].contains("handler_deposit(100)"));
        assert!(failures[1].call_sequence[1].contains("handler_withdraw(200)"));
    }

    #[test]
    fn format_for_llm_passthrough_compile_error() {
        let raw = "COMPILATION ERROR — fix the Solidity before fuzzing can proceed:\nerror[...]";
        assert_eq!(format_for_llm(raw), raw);
    }

    #[test]
    fn format_for_llm_all_passed() {
        let raw = "[PASS] invariant_solvency() (runs: 256, calls: 512, reverts: 0)\nSuite result: ok.";
        assert_eq!(format_for_llm(raw), "All invariants passed.");
    }

    #[test]
    fn format_for_llm_produces_compact_output() {
        let out = format_for_llm(FORGE_OUTPUT);
        assert!(out.contains("2 failing invariant(s)"));
        assert!(out.contains("invariant_bounded"));
        assert!(out.contains("invariant_solvency"));
        assert!(!out.contains("[PASS]"));
        assert!(!out.contains("Suite result"));
        assert!(!out.contains("[Sequence]"));
    }
}
