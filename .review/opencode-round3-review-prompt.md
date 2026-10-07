# Round 3 — Adversarial Delta Review (verification of round-2 remediations)

You are an adversarial code reviewer. Rounds 1 and 2 already reviewed the seed/straddle work; round 2 (`.review/review-output3.txt`) returned APPROVE with 2 MINOR + 3 NIT. The working tree now contains remediations for all five. Your job: **try to break the remediations and the code around them.** Find regressions the remediations introduced. Do not re-litigate accepted design trade-offs unless the remediation changed their terms.

## What changed since round 2 (the delta under review)

1. **`src/history.rs` — coverage check rewritten (round-2 MINOR #1).**
   Old: straddle hour-rollup bucket suppressed when ANY raw 1s sample had `min_ts` inside that hour (sample-existential).
   New: suppressed only when raw rows REACH THE WINDOW START — `raw_points.iter().any(|(_, min_ts, _)| *min_ts <= since_ms)`. Otherwise the bucket is KEPT (clamped to `since_ms`) so partial raw coverage (post-prune, engine restart) still shows the hourly average instead of a hole. Loop var renamed `_bucket_ts` (now unused). Branch-top and inline comments updated.
   New test `test_timeseries_straddle_bucket_kept_when_raw_misses_window_start` pins the kept case (raw only in current hour → 42.0 bucket present at `since`); the existing `..._suppressed_when_raw_covers_window` pins the suppression case.

2. **`frontend/src/components/charts/ChartWithTimeScale.tsx` — `applySeedResults` extraction (round-2 NIT).**
   The setSeed updater body moved into an exported pure helper `applySeedResults(prev, results, needed)`: failed metrics keep previous seed; metrics absent from the needed set are EVICTED (map stays bounded to live configs); returns `prev` itself when nothing changed (React bail-out). The hook now calls it with `new Set(JSON.parse(neededKey))`.

3. **`frontend/src/__tests__/ChartWithTimeScale.test.tsx` — failure-path honesty (round-2 MINOR #2) + NITs.**
   - Global mock wrapper now passes `Response` instances through untouched (`if (body instanceof Response) return body`) so `!res.ok` paths actually run.
   - Visibility test: saves/restores the `visibilityState` property descriptor; explicitly unmounts and asserts the visibilitychange listener was removed (no extra fetch after unmount).
   - New integration test `tolerates a failed seed fetch round (!ok) on tab re-visibility` (round 1 ok → round 2 real 500).
   - Two direct `applySeedResults` unit tests (retention + eviction + identity-bail-out).

## Attack brief

- **Window-edge check**: construct geometries where the bucket is wrongly KEPT (old value-jump artifact returns?) or wrongly SUPPRESSED (hole returns?). Interactions: `since_hour` agg query, `MAX((ts/b)*b, ?2)` clamp, clamped bucket ts = `bucket_ts.max(since_ms)`, sort+dedupe with raw winning. Can the kept bucket now DUPLICATE a raw point at `since`? (dedupe: raw inserted first wins — verify.) Scalar 2-arg `MAX` vs the old row-level logic. `min_ts <= since_ms` with min_ts from a bucket floored before `since`.
- **`applySeedResults`**: eviction interplay with derived-series source metrics (needed set = collectNeededMetrics); concurrent rounds/gen guard; does it ever mutate `prev`; can eviction drop a metric that a *just-changed* configs array still needs (stale neededKey timing); `changed` flag correctness.
- **Mock passthrough**: does `Response` passthrough change behavior of existing tests that return plain objects? Node/jsdom `Response instanceof` correctness under the vitest environment.
- **Tests**: do the new tests actually pin what they claim? Timing flakes? Does the visibility test's descriptor restore work when jsdom defines `visibilityState` on the prototype?
- Re-run the gates yourself: `cargo test --locked`, `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cd frontend && npx vitest --run && npm run build`. Report real output.

## Deliverable format

For each finding: `[SEVERITY] file:line — description + concrete failure scenario + suggested fix`. Severities: CRITICAL / MAJOR / MINOR / NIT. Then a verification section (gate outputs). End with exactly one line:
`VERDICT: APPROVE` or `VERDICT: CHANGES REQUIRED`
