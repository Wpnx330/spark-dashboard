# Commit plan — fix/chart-buffer-seed-1h-domain (rebase-merge repo: every commit builds standalone)

## Commit 1 — backend
fix(history): clamp straddling hour bucket to range start + tighten rollup suppression

- query_timeseries: floor-clamp the completed hour bucket that straddles the
  query window start (MAX((ts/bucket_ms)*bucket_ms, since)) so the 1h X-axis
  domain stretches exactly to the requested range instead of up to 55min past it
- suppress the straddling hour-avg only when raw 1s rows actually REACH the
  window start (min_raw_ts <= since), not merely because any sample exists in
  the hour — data pruned mid-hour or engine restart no longer punches a hole
- tests: straddle bucket suppressed when raw covers window; straddle bucket
  KEPT (clamped to since) when raw misses window start; both assert the
  clamped bucket lands exactly at since
- gates: cargo test 174/174, fmt clean, clippy -D warnings clean

Files: src/history.rs

## Commit 2 — frontend
feat(ui): seed 1m/5m chart buffers from SQLite history on mount and tab return

- ChartWithTimeScale: on mount, timescale switch, and visibilitychange
  (visible), backfill the in-memory rolling buffers from
  /api/history/timeseries so charts no longer reset on refresh/tab-away
- extract pure applySeedResults(): per-metric retention on partial fetch
  failure, stale metric-key eviction, two-pointer timestamp merge, identity
  return when nothing changed (React bail-out)
- EngineCard: latency unit conversion mapping for seeded values
- tests: merge/collision/gap-filling, applySeedResults retention + eviction,
  real Response objects so !ok and catch paths execute, unmount removes the
  visibility listener, second-round failure retains prior seed
- gates: vitest 247/247, npm run build clean

Files: frontend/src/components/charts/ChartWithTimeScale.tsx,
frontend/src/components/engines/EngineCard.tsx,
frontend/src/__tests__/ChartWithTimeScale.test.tsx

## Commit 3 — docs
docs: note 1m/5m chart backfill from history in README

Files: README.md

All commits: Co-authored-by: TRON <tron-agent@agentmail.to>
