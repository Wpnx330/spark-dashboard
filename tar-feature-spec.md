# Add TAR (Token Acceptance Rate) to Cache Chart + Fix X-Axis

## Task 1: X-Axis Fix (ALREADY DONE — just verify after deploy)
In `frontend/src/components/charts/TimeSeriesChart.tsx`, XAxis now has:
```jsx
type="number"
scale="time"
domain={['dataMin', 'dataMax']}
```
This makes recharts treat timestamps as continuous numeric values instead of categories, producing consistent hourly ticks across all charts.

## Task 2: Add TAR as Yellow Line to Cache Chart

TAR = Token Acceptance Rate = accepted_tokens / draft_tokens * 100 (percentage 0-100)

### Backend Changes (Rust):

#### 2a. `src/history.rs` — Schema migration
Add `spec_decode_acceptance_rate REAL` column to:
- `snapshots_1s` table
- `snapshots_1h` table (use AVG in rollup)  
- `snapshots_1d` table (use AVG in rollup)

Add migration like existing columns (ALTER TABLE ... ADD COLUMN IF NOT EXISTS).

#### 2b. `src/history.rs` — insert_1s()
Add `spec_decode_acceptance_rate: Option<f64>` parameter to `insert_1s()`.
Add it to the INSERT statement and params array.

#### 2c. `src/history.rs` — rollup_1s_to_1h() and rollup_1h_to_1d()
Add `AVG(spec_decode_acceptance_rate)` to the rollup SELECT/INSERT queries.

#### 2d. `src/history.rs` — timeseries query
Add 'spec_decode_acceptance_rate' to the supported metrics list in the timeseries API handler. It should query the column from the appropriate table (1s/1h/1d based on range).

#### 2e. `src/metrics/mod.rs`
Pass `m.spec_decode_acceptance_rate_live` (the live windowed TAR) to `insert_1s()`.

### Frontend Changes (TypeScript/React):

#### 2f. `frontend/src/hooks/useMetricsHistory.ts`
Add a buffer array for TAR:
- Add `tar: ChartDataPoint[]` to the engine buffer interface
- Push `{ timestamp: ts, value: engine.metrics.spec_decode_acceptance_rate_live }` when not null
- Add it to the chartData output

#### 2g. `frontend/src/components/engines/EngineCard.tsx`
- Add `tar: ChartDataPoint[]` to the chartData interface
- Add `'spec_decode_acceptance_rate'` to `CACHE_HISTORY` array:
  ```ts
  const CACHE_HISTORY: HistorySeriesConfig[] = ['kv_cache_pct', 'prefix_cache_hit', 'spec_decode_acceptance_rate']
  ```
- Add TAR as 3rd yellow series to BOTH Cache chart instances (FlipCard back + full chart):
  ```tsx
  { data: chartData.tar, label: 'TAR', color: '#f59e0b' }
  ```

### Important Notes:
- The live buffer (5m/1m) uses `spec_decode_acceptance_rate_live` (windowed, fluctuates)
- The history API (1h/24h) uses `spec_decode_acceptance_rate` column (stored value)
- TAR is 0-100%, same y-domain as KV Cache and Prefix Hit
- Yellow color: `#f59e0b` (matches existing amber/yellow used elsewhere)
- If spec decode is not configured, TAR values will be null → chart should skip the line gracefully (mergeSeries already handles missing series)
