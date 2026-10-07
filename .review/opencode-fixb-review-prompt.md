# Adversarial Code Review Request — spark-dashboard FIX B (buffer seeding)

You are an adversarial code reviewer. Find real defects. Do NOT rubber-stamp.
Do NOT rewrite the code. Report findings with file:line, severity (CRITICAL/MAJOR/MINOR/NIT),
and a one-line fix suggestion each. If the diff is genuinely clean, say so explicitly.

## Repo
/home/cwykel/spark-dashboard-prs/pr46 (branch main, on top of dfda989)

Diff under review: .review/fixB_review_diff.txt (git diff, ~525 lines)
Files changed:
- src/history.rs (backend: clamp straddling 1h-bucket timestamp into the requested window — FIX A)
- frontend/src/components/charts/ChartWithTimeScale.tsx (frontend: seed 1m/5m buffer charts from history API — FIX B)
- frontend/src/components/engines/EngineCard.tsx (two call sites: new seedValueMap prop)
- frontend/src/__tests__/ChartWithTimeScale.test.tsx (new tests)

## Context
- Bug: 1m/5m charts (in-memory WebSocket buffers) reset when the user switches tabs, navigates, or refreshes. Fix = seed from `/api/history/timeseries` when the live buffer is sparse.
- Backend: snapshots_1s table holds raw 1s data (~1h retention); snapshots_1h = completed-hour rollups; rollup only writes COMPLETE hours (ts < current_hour_start), so a 60s/300s seed window cannot hit partial rollup rows. query_timeseries for ranges <= 1h returns 1s raw data bucketed by bucket_size_ms(range) = (range/360).max(1000) ms.
- Constraints from the user: ZERO regressions to 24h and 5m history views; 1h chart must span exactly 60 min; live streaming behavior when data is dense must be byte-identical to before.
- Tests already pass: 241/241 vitest, 172/172 cargo, clippy -D warnings, fmt. Your job is to find what tests do NOT catch.

## Adversarial checklist (attack these specifically)
1. React hook rules: useBufferSeed/useAllHistoryData/useMetricSlot — conditional hooks, deps arrays, stale closures, fetch races (gen counter), unmount safety, setSeed after unmount.
2. Infinite render/fetch loops: any state update that retriggers an effect that updates that state again (neededKey JSON round-trip, configs identity changes from inline arrays in Dashboard.tsx at ~1Hz).
3. mergeSeedIntoLive correctness: duplicate timestamps at the seam, unsorted seed arrays (backend returns sorted? verify against src/history.rs query_timeseries ORDER BY), dense threshold boundary (live.length == denseThreshold), valueMap applied consistently.
4. FIX A backend clamp: `ts.max(since_ms)` — any way this distorts data semantics (point now claims a bucket value at a timestamp it does not cover)? Interaction with the dedupe/sort below it? Effect on the 1h summary path (query_summary untouched — confirm)?
5. Regression risk to 24h/7d paths and to the dense-live-buffer streaming path (must be byte-identical: mergeSeedIntoLive returns `live` by reference when dense — confirm no copy, no reorder).
6. Security: URL construction (encodeURIComponent coverage), JSON.parse of neededKey, any injection via metric/engine strings.
7. Performance: seed fetch fan-out per chart (up to 8 metrics × N charts on mount), localStorage/network storms, repeated visibilitychange refetch cost.
8. Anything else you find — you are adversarial; go hunting.

## Output format
Severity-ordered list. Each: [SEVERITY] file:line — issue — suggested fix.
End with verdict line: "VERDICT: CLEAN" or "VERDICT: N issues (X critical, Y major)".
