# Fix 1h chart X-axis domain stretch + seed 1m/5m chart buffers from history

## Problems

**1h chart domain stretch** — when a query window started mid-hour, the completed hour bucket that straddled the window start was reported at its raw timestamp (up to 55 min before `since`), so the 1h chart's fixed X-domain stretched to ~2h with data scrunched into the right half.

**1m/5m chart resets** — the in-memory rolling circular buffers started empty on every page load, tab switch, or timescale change. Charts showed only data sampled after mount, losing up to a full retention window of history.

## Changes

### Backend — `src/history.rs`
- Floor-clamp the straddling completed-hour bucket to the query window start (`MAX((ts/bucket_ms)*bucket_ms, since_ms)`), so the 1h X-axis domain stretches exactly to the requested range.
- Suppress the straddling hour-average only when raw 1s rows **reach the window start within one bucket width** (`min_raw_ts < since_ms + bucket_ms`), not merely when any sample exists in the hour:
  - The raw query filters `ts >= since_ms`, so an exact `min_ts <= since_ms` comparison degenerates to millisecond equality — which continuous 1s sampling (arbitrary sub-second phase vs the client's `Date.now()`) essentially never produces. This was caught by adversarial review round 3 with an empirical probe (hour-avg 42.0 planted next to raw 99.0 at the window edge).
  - Continuous coverage → hour-avg suppressed (no artificial value-jump at the window's left edge).
  - Post-prune / engine-restart late start → hour-avg kept (no data hole).
- Both straddle test fixtures are deliberately placed to **fail under the pre-fix checks** (suppression test fails the degenerate equality check; kept test fails the old per-bucket existence check) — discrimination empirically probed both ways, tree restored hash-verified.

### Frontend — `ChartWithTimeScale.tsx`, `EngineCard.tsx`
- On mount, timescale switch, and `visibilitychange` (visible), backfill the in-memory rolling buffers from `/api/history/timeseries`, so 1m/5m charts no longer reset on refresh or tab-away.
- Extracted pure `applySeedResults()`: per-metric retention on partial fetch failure, stale metric-key eviction, two-pointer timestamp merge, identity return when nothing changed (React bail-out).
- EngineCard: latency unit conversion mapping for seeded values.
- Failed seed rounds (HTTP !ok or network rejection) retain the prior seed — no blanking.

### Tests
- Backend: straddle suppressed/kept geometries, clamped-timestamp assertions, bucket-grid-unaligned fixtures (174/174, fmt, clippy `-D warnings`).
- Frontend: merge/collision/gap-filling, `applySeedResults` retention + eviction, real `Response` objects so `!ok` and network-reject paths execute, unmount removes the visibility listener, second-round failure retains prior seed (247/247, build clean).

## Verification
- `cargo fmt --all -- --check` ✓ · `cargo clippy --all-targets --locked -- -D warnings` ✓ · `cargo test --locked` 174/174 ✓
- `npx vitest --run` 247/247 ✓ · `npm run build` ✓
- 4 adversarial review rounds (OpenCode); round 3 caught a degenerate boundary check in an earlier iteration of this fix — final round APPROVE.

Reviewed by TRON — Execution arm of the Wpnx330 system.
