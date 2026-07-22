# PR #47 — Maintainer Feedback (Niklas Frick)

## Status: Open, MIXED — half accepted, half rejected. Most rework needed.

## Scope Decision (Maintainer)

**ACCEPTED (in scope):**
- Persistent metrics history — SQLite-backed per-engine rollups (1s/1h/1d), background aggregation, time-range summary queries, and a Historical view in the frontend.
- The in-memory ring buffer was never going to answer "what happened yesterday" — this direction is accepted.
- The history schema/rollup design (1s → 1h → 1d with background aggregation) is a reasonable shape.
- The rusqlite in-memory tests for rollup logic are the right kind of test.

**REJECTED (durable project boundary — NOT personal to this PR):**
- Cost analysis is out of scope, and so is the pricing lookup.
- Cloud-vs-on-prem comparison, revenue/cost/profit modeling, electricity/cloud rate settings, and the OpenRouter `lookup-pricing` call are ALL rejected.
- The dashboard makes no third-party network calls.
- Money-math is a downstream concern built on exported history data.
- Full reasoning recorded in `.out-of-scope/cost-analysis.md` (landing via #50).

## CI Status
CI fails on this branch: `App.tsx` uses `filterView`/`fillHeight` props that only exist in #46, so the branch doesn't build against `main` on its own — and `rusqlite` isn't declared in `Cargo.toml`/`Cargo.lock` at all, so the Rust side can't compile either.

## Required Fixes

1. **Strip the rejected scope**: the `lookup-pricing` endpoint and OpenRouter client code, the cost/revenue/profit summary cards and comparison table in `HistoryView.tsx`, and the cloud-rate/electricity settings. What remains is history recording, rollups, summary/size queries, and the date-range UI.

2. **Declare the dependency**: add `rusqlite` to `Cargo.toml` (check crates.io for the latest stable and pin that) — the accompanying `Cargo.lock` change for a new dependency is legitimate and expected.

3. **Make CI green standalone or explicitly stacked**: right now the branch silently depends on #46. Either rebase it to stand alone against `main`, or rebase after #46 lands — but "does not compile against its base" can't be the resting state. NOTE: We have merged the #46 branch into this workspace so the App.tsx dependency is satisfied.

4. **Gate the endpoints**: follow the `--enable-log-viewer` pattern from #48 — an opt-in flag (e.g. `--enable-history`, default off) for the `/api/history/*` routes. `POST /api/history/prune` is destructive and `settings`/`toggle` are writes; none of these can be exposed unauthenticated by default on `0.0.0.0`.

5. **Fix the persistence path**: the distroless `nonroot` runtime can't write `/var/lib`, so the current code silently falls back to `/tmp` — which is ephemeral in the container. Make the DB path an explicit flag/env with a documented compose volume, and fail loudly if it isn't writable rather than falling back.

6. **Restore what was deleted**: the `healthz_returns_ok` test in `src/server.rs` must come back (repo rule: tests ship with changes, and existing tests don't get dropped), along with the removed comments; `src/cli/mod.rs` / `service.rs` have mode-only churn (`644` → `755`) to revert.

7. **Frontend tests**: `HistoryView` needs Vitest coverage per repo rules (date-range selection and summary rendering are enough once the cost UI is gone).

## Verification Before Pushing
```
cargo fmt --all -- --check && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked
cd frontend && npm run build && npm test -- --run
```

## PR URL
https://github.com/niklasfrick/spark-dashboard/pull/47
