# Round 4 — Adversarial Delta Review (verify round-3 remediations)

You are an adversarial code reviewer. Round 3 (`.review/review-output4.txt`, verdict CHANGES REQUIRED) found 5 findings; all were remediated. Verify each remediation and hunt for regressions the remediations introduced. Repo: current working tree at /home/cwykel/spark-dashboard-prs/pr46, branch fix/chart-buffer-seed-1h-domain.

## Findings you must verify as fixed

1. **[MAJOR] src/history.rs ~line 780-810**: coverage check was degenerate (`min_ts <= since_ms` ⟺ exact-ms equality under the `ts >= since_ms` filter). Now: `raw_points.iter().any(|(_, min_ts, _)| *min_ts < since_ms + bucket_ms)`, hoisted loop-invariant above the `for` loop, comment rewritten. Attack the new check: continuous 1s coverage must suppress (no artifact); post-prune/restart late-start must keep (no hole); edge cases (bucket_ms == 1000 with 1s seed windows; first sample exactly at since; empty raw_points).
2. **[MINOR] kept-test geometry**: `test_timeseries_straddle_bucket_kept_when_raw_misses_window_start` now inserts raw at h2-300_000 / h2-299_500 / h2+1000 (inside straddling hour, after since). Prove it discriminates: temporarily restore the OLD per-bucket check (`min_ts >= since_hour && min_ts < since_hour + 3_600_000` — scratch worktree or byte-restore with hash check), run the straddle tests, confirm the kept-test FAILS under old and PASSES under new; restore byte-exact (git hash-object before/after).
3. **[NIT] suppression-test fixture de-aligned**: `…_suppressed_when_raw_covers_window` first raw row is now h1+3_000_400 (since = h1+3_000_000). Confirm the test still pins suppression under the NEW check with an unaligned fixture, and that it exercises the new semantics (not fixture-alignment luck).
4. **[NIT] dead descriptor save/restore removed** from ChartWithTimeScale.test.tsx (stateDesc gone). Confirm no visibilityState own-property override remains and no test depends on leaked state.
5. **[NIT] catch path covered**: failed-round test round 3 returns `Promise.reject(new TypeError('network unreachable'))`. Confirm the component's catch executes (rejected fetch → caught → seed retained) and the test genuinely awaits round 3.

## Gates to re-run (all must pass)

```
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cd frontend && npx vitest --run
cd frontend && npm run build
```

If you add a temporary probe to src/history.rs for finding 2, restore the file byte-exact and verify with `git hash-object` before finishing.

## Report format

- Per finding: FIXED VERIFIED / NOT FIXED / REGRESSION INTRODUCED, with evidence.
- Any NEW findings (severity-tagged, file:line).
- Summary paragraph.
- Final line: `VERDICT: APPROVE` or `VERDICT: CHANGES REQUIRED`
