# Adversarial Review — Round 2 (verification of round-1 fix delta)

You are a hostile senior reviewer. Round 1 of this review found 9 issues (1 MAJOR, 3 MINOR, 5 NIT) in the branch `fix/chart-buffer-seed-1h-domain` of this repo (spark-dashboard). All 9 were addressed; your job now is to try to BREAK the fixes and the code around them. Find regressions the fixes introduced.

Read these first:
- `.review/fixdelta_diff.txt` — the diff of the round-1 fixes (840 lines). This is your primary target.
- `src/history.rs` — full backend history code; the diff touches `query_timeseries` ≤1h path + tests.
- `frontend/src/components/charts/ChartWithTimeScale.tsx` — full file; diff touches `mergeSeedIntoLive`, `useBufferSeed`, render-path filtering.
- `frontend/src/__tests__/ChartWithTimeScale.test.tsx` — updated/added tests.
- `.review/review-output2.txt` — round-1 verdict (lines after "VERDICT") for the list of issues the delta claims to fix.

Current state (verified by author, re-verify if suspicious): cargo test 173/173, clippy -D warnings clean, cargo fmt clean; vitest 244/244; vite build ok.

Round-1 fixes to attack (verify each is actually fixed, then hunt for NEW bugs in the same areas):
1. MAJOR — straddle hour-rollup artifact: ≤1h path now fetches raw 1s FIRST and suppresses the completed-hour rollup bucket when raw 1s rows cover part of that hour inside the window (coverage check via MIN(ts) sample-true). Attack: pruned-data cases, hour-boundary geometry, dedupe between rollup and raw rows (same timestamp), clamp contract (no point outside [since, until]), bucket floor clamping via MAX((ts/b)*b, since).
2. MINOR — timestamp-merge: `mergeSeedIntoLive` now two-pointer merges by timestamp (live wins collisions, both pointers advance; seed fills before/between/after live; unsorted seed guarded). Attack: equal-timestamp handling, dense-path early return, valueMap application points, stale-seed windows, memory/perf at 60-300 points.
3. MINOR — per-metric seed retention on partial refetch failure (previous seeds kept for failed metrics, Map copied not mutated).
4. MINOR — unmount + stale-generation guards on async setState in `useBufferSeed` (genRef generation counter, mountedRef).
5. NIT — redundant second filter skipped when merge returns the dense-path array by reference (`merged === cutLive` identity check).
6. NIT — dead `SEED_OVERLAP_MS` removed; doc comments corrected.
7. SQL change: raw bucketing now `MAX((ts / bucket_ms) * bucket_ms, ?2)` — verify parameter binding still matches (?1 engine, ?2 since, ?3 until) and that SQLite scalar MAX(a,b) semantics are correct here.

Hunt specifically for:
- Any case where the chart can now show NO data for the current hour (over-suppression of the rollup bucket).
- Any case where a point lands outside [since, until] after the MAX() clamp (until-side? `ts <= ?3` floors could still exceed until? check).
- Regressions in the 24h path or the >1h paths (they share `bucket_size_ms` and the rollup query — the diff touched only the ≤1h branch, but verify).
- Race conditions in the async seed fetch (scale flip mid-flight, visibilitychange storm, React 18 strict-mode double effects).
- Test assertions that pin implementation details so hard they'd break on harmless refactors, or that don't actually verify the behavior they claim.

Deliver EXACTLY this format at the end:
VERDICT: APPROVE | REQUEST_CHANGES
FINDINGS:
[CRITICAL|MAJOR|MINOR|NIT] file:line — description
(If none: "None." — but earn it.)
