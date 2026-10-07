# TASK: Fix chart blink — stride flip-flop at maxPoints boundary (5m/1m buffer charts)

## Context (verified by TRON via live-DOM instrumentation on the running app)

The 1m/5m charts "blink"/"click over" every ~1 second since the buffer-seed feature
landed (commits a8c0ad6, f1f8b64 on branch `fix/chart-buffer-seed-1h-domain`).

**Measured evidence (live on http://192.168.10.188:3000):**
- Every recharts curve path is destroyed/recreated several times per second.
- The first path's geometry ALTERNATES between two states on a ~1s cycle:
  - State A (~900ms): ~157-point path, head drifting with the live window
  - State B (~110ms flash): ~310-point path, head frozen at the left edge
- 310 ≈ 2 × 157: this is stride-1 vs stride-2 downsampling alternating.

**Root cause (mechanism, precise):**

`frontend/src/components/charts/TimeSeriesChart.tsx` → `padData()`:
```ts
if (data.length > maxPoints) {
  const stride = Math.ceil(data.length / maxPoints)
  ...
}
```
ChartWithTimeScale buffer mode passes `maxPoints={scale === '5m' ? 300 : 60}`.

The seed merge produces a merged array whose length is pinned EXACTLY at the
maxPoints threshold (5m: ~300 points; 1m: ~60 points):
- seed covers [mount−window, mount]; each second one seed point drains out the
  left of the time window (t0 = now − windowMs) while one live sample arrives,
  so merged length oscillates 300 ↔ 301 (and exactly at 300 vs >300 depending
  on collisions between WS tick timing and the time-window cut).
- When merged length ≤ 300 → stride 1 → full-density render (State B).
- When merged length = 301+ → stride 2 → half-density render (State A).
- The WS tick and the window slide land ~100ms apart each second → the render
  alternates A/B/A/B — the visible "click over" blink.

Secondary symptom explained by the same root: 1m x-axis ticks appear to
alternate "2 seconds apart / 1 second apart / 3 seconds apart" — density
aliasing from the same stride flip.

The buffer `cut()` pre-trim (`pts.length > sliceCount ? pts.slice(-sliceCount*2) : pts`)
keeps up to 600 points before the time filter, so the merged array regularly
lands just above 300 → the flip fires constantly. It also fires for the
fully-live dense path once the buffer holds 301 points (steady state), which
is why the blink persists even after the seed is fully drained.

## The Fix (small, buffer-mode only — do NOT touch history mode)

In `frontend/src/components/charts/ChartWithTimeScale.tsx` buffer branch
(`if (isBuffer) { ... }`, around lines 427-504):

1. After producing each final per-series data array (the merged/filtered
   result assigned to `chartSeries[i].data` and the single-line `chartData`),
   trim it to at most `sliceCount` points from the NEWEST end:
   `if (arr.length > sliceCount) arr = arr.slice(-sliceCount)`.
   - 5m: sliceCount=300, 1m: sliceCount=60 (reuse BUFFER_SLICE — do not
     hardcode a second source of truth).
   - This guarantees `padData` never sees length > maxPoints, so the stride
     flip can never engage. One stable full-density rendering; the chart
     just slides like pre-seed.
2. Apply the same trim to the dense path (when `mergeSeedIntoLive` returns
   `cutLive` by reference). The steady-state live buffer can hold 301+
   points and blinks too — trim it identically. Keep the existing
   "skip second filter" optimization semantics intact if trivial; correctness
   of the trim matters more than preserving the micro-optimization — if
   trimming requires a copy anyway, just always run the time filter + trim
   for clarity and note it in the report.
3. Do NOT change: history mode (1h/24h), `padData()` itself,
   `mergeSeedIntoLive()` merge semantics, seed fetch behavior, the
   `bufferTimeDomain`, or any non-buffer chart path.

## Tests (Gate 2) — extend `frontend/src/__tests__/ChartWithTimeScale.test.tsx`

Add unit tests (pure logic, no external services):
- merged array of exactly 301 points (300 seed + 1 live, or construct via
  mergeSeedIntoLive) trimmed to 300 → rendered point count stays 300 and
  does NOT halve (assert the data array handed onward has length 300, and
  that length stays 300 after a simulated second tick adds one more live
  point while one seed point ages out of the window).
- array of 601 points trimmed to 300.
- array of ≤300 points passes through unchanged (identity, not copy, is fine).
- 1m variant: 61 points → 60.
- Regression intent documented in a comment: "prevents stride-1/stride-2
  alternation at the maxPoints boundary (chart blink)".

## Constraints

- TypeScript strict; match existing code style exactly (this codebase uses
  concise helpers + comments explaining WHY).
- Run from repo root `/home/cwykel/spark-dashboard-prs/pr46`:
  - `cd frontend && npx vitest run` → ALL tests must pass (existing 247 + new)
  - `cd frontend && npm run build` → exit 0
- Do NOT commit. Do NOT push. TRON reviews and commits.
- Do NOT touch any file other than:
  - `frontend/src/components/charts/ChartWithTimeScale.tsx`
  - `frontend/src/__tests__/ChartWithTimeScale.test.tsx`

## Report format (final message)

1. Exactly what you changed (function/lines).
2. vitest summary line (N files, N tests passed).
3. `npm run build` exit status.
4. Documentation Updates Needed section (Gate 4) — likely "none, internal
   rendering fix; README chart section unchanged" but state it explicitly.
5. Any risks/edge cases you noticed but did not change.
