import { useState, useMemo, useCallback, useEffect, useRef } from 'react'
import { TimeSeriesChart, type ChartSeries } from './TimeSeriesChart'
import { TimeScaleButton, type TimeScale } from './TimeScaleButton'
import { useHistoryTimeseries } from '@/hooks/useHistoryTimeseries'

interface DataPoint {
  timestamp: number
  value: number
}

/**
 * A series that is computed from one or more fetched history metrics
 * rather than having its own direct DB column.
 *
 * For example, "per-request throughput" = decode_tps / active_requests.
 * The component fetches each source metric from the history API, then
 * calls `compute` to produce the derived data points.
 */
export interface DerivedSeries {
  /** Metrics to fetch from the history API. */
  sourceMetrics: string[]
  /** Compute derived data from fetched source data. */
  compute: (sources: Record<string, DataPoint[] | null>) => DataPoint[]
}

/**
 * Per-series history config. Either:
 * - `metric`: a direct DB column name (string) → fetched and shown as-is
 * - `derived`: computed from other metrics via DerivedSeries
 * - `undefined`: no history support (series hidden at 1h/24h — should be rare now)
 */
export type HistorySeriesConfig = string | DerivedSeries | undefined

interface ChartWithTimeScaleProps {
  /** Single-line buffer mode (1m/5m). */
  bufferData?: DataPoint[]
  /** Multi-line buffer mode (1m/5m). */
  bufferSeries?: ChartSeries[]
  /** Engine endpoint for history API calls (1h/24h). If null, button is hidden. */
  engineEndpoint: string | null
  /**
   * Per-series history configuration. For single-line: one entry.
   * For multi-line: array matching bufferSeries order.
   * String = direct DB metric, DerivedSeries = computed, undefined = hidden.
   */
  historyMetrics?: HistorySeriesConfig[]
  /**
   * Transform applied to seeded (DB) values before display. Needed when the
   * buffer data is transformed at the call site (e.g. E2E chart divides ms
   * → s) so seed and live values share one unit. History mode is untouched.
   */
  seedValueMap?: (value: number) => number
  /** Pass-through to TimeSeriesChart. */
  color?: string
  yDomain?: [number, number]
  unit?: string
  height?: number | string
  title?: string
  compact?: boolean
  hideTooltipLabel?: boolean
  tooltipLabel?: string
  seriesLabel?: string
  className?: string
  events?: Array<{ timestamp: number; type: string; detail: string }>
  requests?: Array<{ start: number; end: number; tps: number; ttft: number }>
}

/** Scale cycle order: 5m → 1h → 24h → 1m → 5m. */
const SCALE_CYCLE: TimeScale[] = ['5m', '1h', '24h', '1m']

function nextScale(current: TimeScale): TimeScale {
  const idx = SCALE_CYCLE.indexOf(current)
  return SCALE_CYCLE[(idx + 1) % SCALE_CYCLE.length]
}

/**
 * Number of buffer samples to show for each buffer-based scale.
 * Buffer is 900 samples at 1/sec = 15 min.
 */
/** Window length per buffer scale, ms. TIME-cuts the buffer and pins the
 * x-domain to exactly [now-N, now] so span never depends on sample density. */
const BUFFER_WINDOW_MS: Record<'1m' | '5m', number> = {
  '1m': 60_000,
  '5m': 300_000,
}

const BUFFER_SLICE: Record<'1m' | '5m', number> = {
  '1m': 60,
  '5m': 300,
}

/**
 * Merge DB-seeded history points into a sparse live buffer window.
 *
 * 1m/5m charts render from an in-memory WebSocket buffer. When the tab is
 * hidden (background tabs throttle sample processing) or the page reloads,
 * that buffer goes stale or empty and the chart "resets" even though the
 * server still holds the 1s data. Seeding backfills the visible window from
 * `/api/history/timeseries` — the same source the 1h/24h charts use.
 *
 * Rules:
 * - Dense live buffer (≥ denseThreshold in-window points): returned as-is —
 *   streaming behavior is identical to the pre-seed implementation.
 * - Empty live buffer: seed fills the whole window (page refresh case).
 * - Sparse live buffer: merged by timestamp — live wins on collisions, seed
 *   fills everything else (holes before, between, and after live samples:
 *   background-tab throttling gaps, or a stalled WebSocket where the DB is
 *   the freshest source until the stream catches up, at which point the
 *   collision rule hands the tail back to live).
 */
export function mergeSeedIntoLive(
  live: DataPoint[],
  seed: DataPoint[] | null | undefined,
  opts: { denseThreshold: number; valueMap?: (v: number) => number },
): DataPoint[] {
  if (live.length >= opts.denseThreshold) return live
  if (!seed || seed.length === 0) return live
  const map = opts.valueMap
  const apply = (p: DataPoint): DataPoint =>
    map ? { timestamp: p.timestamp, value: map(p.value) } : p

  // No live data at all → the seed IS the chart (page refresh / WS down).
  if (live.length === 0) return seed.map(apply)

  // Seed must be ascending for the two-pointer merge (backend returns
  // ORDER BY timestamp; derived computes preserve order). Guard cheaply
  // instead of trusting every producer.
  let sorted = seed
  for (let k = 1; k < seed.length; k++) {
    if (seed[k].timestamp < seed[k - 1].timestamp) {
      sorted = [...seed].sort((a, b) => a.timestamp - b.timestamp)
      break
    }
  }

  // Two-pointer merge; on equal timestamps the live point wins (both
  // pointers advance so the seed duplicate is consumed, not re-emitted).
  const out: DataPoint[] = []
  let i = 0
  let j = 0
  while (i < sorted.length && j < live.length) {
    const s = sorted[i]
    const l = live[j]
    if (s.timestamp < l.timestamp) {
      out.push(apply(s))
      i++
    } else {
      out.push(l)
      j++
      if (s.timestamp === l.timestamp) i++
    }
  }
  while (j < live.length) {
    out.push(live[j])
    j++
  }
  // Seed points newer than every live sample: keep them. When the WebSocket
  // is down the DB holds the freshest seconds; once live catches up the
  // collision rule above hands those timestamps back to the stream.
  while (i < sorted.length) {
    out.push(apply(sorted[i]))
    i++
  }
  return out
}

/**
 * Merge one seed-fetch round into the seed map (pure — returns `prev`
 * itself when nothing changed so React can bail out of the re-render).
 *
 * - A metric that failed this round (network blip, one 500 among several)
 *   keeps its previous seed instead of vanishing for a cycle. Stale points
 *   are clipped by the time window at render anyway.
 * - Metrics no longer needed (configs changed) are evicted so the map
 *   stays bounded to the live config set.
 */
export function applySeedResults(
  prev: Map<string, DataPoint[]>,
  results: ReadonlyArray<readonly [string, DataPoint[]] | null>,
  needed: ReadonlySet<string>,
): Map<string, DataPoint[]> {
  let changed = false
  const next = new Map(prev)
  for (const r of results) {
    if (r && r[1].length > 0) {
      next.set(r[0], r[1])
      changed = true
    }
  }
  for (const key of next.keys()) {
    if (!needed.has(key)) {
      next.delete(key)
      changed = true
    }
  }
  return changed ? next : prev
}

/** Flattened metric list needed for history fetches (direct + derived sources). */
function collectNeededMetrics(configs: HistorySeriesConfig[]): string[] {
  const metrics = new Set<string>()
  for (const cfg of configs) {
    if (typeof cfg === 'string') {
      metrics.add(cfg)
    } else if (cfg && typeof cfg === 'object') {
      for (const m of cfg.sourceMetrics) metrics.add(m)
    }
  }
  return Array.from(metrics)
}

/** Resolve the seed points for one series config from the fetched seed map.
 * Direct configs look up their metric; derived configs compute from sources —
 * exactly how history mode resolves them. */
function seedPointsFor(
  cfg: HistorySeriesConfig,
  seedData: Map<string, DataPoint[]>,
): DataPoint[] | null {
  if (!cfg) return null
  if (typeof cfg === 'string') return seedData.get(cfg) ?? null
  if (typeof cfg === 'object') {
    const sources: Record<string, DataPoint[] | null> = {}
    let any = false
    for (const m of cfg.sourceMetrics) {
      const d = seedData.get(m) ?? null
      sources[m] = d
      if (d && d.length > 0) any = true
    }
    if (!any) return null
    return cfg.compute(sources)
  }
  return null
}

/** Hook: fetch all unique metrics needed for history mode. */

/** Individual hook slot — always called, returns null if metric is empty. */
function useMetricSlot(
  engineEndpoint: string | null,
  metric: string | null,
  scale: TimeScale,
): { metric: string; data: DataPoint[] | null; loading: boolean } {
  const { data, loading } = useHistoryTimeseries(
    metric ? engineEndpoint : null,
    metric ?? '__none__',
    scale,
  )
  const mapped: DataPoint[] | null = data
    ? data.map((p) => ({ timestamp: p.timestamp_ms, value: p.value }))
    : null
  return { metric: metric ?? '', data: mapped, loading }
}

/** Hook that fetches up to MAX_METRIC_SLOTS unique metrics. */
function useAllHistoryData(
  engineEndpoint: string | null,
  configs: HistorySeriesConfig[],
  scale: TimeScale,
): { data: Map<string, DataPoint[]>; loading: boolean } {
  const needed = useMemo(() => collectNeededMetrics(configs), [configs])

  // Always call exactly MAX_METRIC_SLOTS hooks (stable count per render).
  // Slots beyond the needed list pass null metric → hook returns null immediately.
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const slot0 = useMetricSlot(engineEndpoint, needed[0] ?? null, scale)
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const slot1 = useMetricSlot(engineEndpoint, needed[1] ?? null, scale)
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const slot2 = useMetricSlot(engineEndpoint, needed[2] ?? null, scale)
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const slot3 = useMetricSlot(engineEndpoint, needed[3] ?? null, scale)
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const slot4 = useMetricSlot(engineEndpoint, needed[4] ?? null, scale)
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const slot5 = useMetricSlot(engineEndpoint, needed[5] ?? null, scale)
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const slot6 = useMetricSlot(engineEndpoint, needed[6] ?? null, scale)
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const slot7 = useMetricSlot(engineEndpoint, needed[7] ?? null, scale)

  const slots = [slot0, slot1, slot2, slot3, slot4, slot5, slot6, slot7]

  const map = useMemo(() => {
    const m = new Map<string, DataPoint[]>()
    for (const s of slots) {
      if (s.metric && s.data) m.set(s.metric, s.data)
    }
    return m
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [slot0.data, slot1.data, slot2.data, slot3.data, slot4.data, slot5.data, slot6.data, slot7.data])

  const loading = slots.some((s) => s.loading && s.metric !== '')
  return { data: map, loading }
}

/** Hook: seed data for buffer modes (1m/5m) from the history API.
 *
 * Buffer charts otherwise rely solely on in-memory WebSocket samples, which
 * go stale when the tab is hidden or vanish on reload. This fetches the most
 * recent window from the DB (same endpoint as 1h/24h mode) on mount, on
 * scale/endpoint/config change, and whenever the tab becomes visible again.
 * The merge itself happens at render time via mergeSeedIntoLive — a dense
 * live buffer never touches the seed. */
function useBufferSeed(
  engineEndpoint: string | null,
  configs: HistorySeriesConfig[],
  scale: TimeScale,
): Map<string, DataPoint[]> {
  const isBuffer = scale === '1m' || scale === '5m'
  // Content-stable key: callers pass inline arrays (e.g. ['gpu_util']) whose
  // identity changes every parent render — keying on JSON avoids refetching
  // the seed on each metrics tick.
  const neededKey = useMemo(
    () => (isBuffer ? JSON.stringify(collectNeededMetrics(configs)) : '[]'),
    [configs, isBuffer],
  )
  const [seed, setSeed] = useState<Map<string, DataPoint[]>>(() => new Map())
  const genRef = useRef(0)
  const mountedRef = useRef(true)
  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  const fetchSeed = useCallback(async () => {
    if (!isBuffer || engineEndpoint === null || neededKey === '[]') return
    const needed: string[] = JSON.parse(neededKey)
    const gen = ++genRef.current
    const untilMs = Date.now()
    const windowMs = scale === '1m' ? BUFFER_WINDOW_MS['1m'] : BUFFER_WINDOW_MS['5m']
    const sinceMs = untilMs - windowMs

    const results = await Promise.all(
      needed.map(async (metric) => {
        const url =
          `/api/history/timeseries?engine=${encodeURIComponent(engineEndpoint)}` +
          `&metric=${encodeURIComponent(metric)}` +
          `&since_ms=${sinceMs}&until_ms=${untilMs}`
        try {
          const res = await fetch(url)
          if (!res.ok) return null
          const json = await res.json()
          const pts: DataPoint[] = (json.points ?? []).map(
            (p: { timestamp_ms: number; value: number }) => ({
              timestamp: p.timestamp_ms,
              value: p.value,
            }),
          )
          return [metric, pts] as const
        } catch {
          return null
        }
      }),
    )
    // A newer fetch (scale flip / visibility change) supersedes this one —
    // and an unmounted component must never set state.
    if (gen !== genRef.current || !mountedRef.current) return
    // Merge per metric via the pure helper; a metric that failed this
    // round (network blip, one 500 among several) keeps its previous
    // seed instead of vanishing for a cycle. Stale points are clipped
    // by the time window at render anyway.
    setSeed((prev) =>
      applySeedResults(
        prev,
        results,
        new Set(JSON.parse(neededKey) as string[]),
      ),
    )
  }, [engineEndpoint, isBuffer, neededKey, scale])

  useEffect(() => {
    void fetchSeed()
  }, [fetchSeed])

  useEffect(() => {
    if (!isBuffer) return
    const onVisible = () => {
      if (document.visibilityState === 'visible') void fetchSeed()
    }
    document.addEventListener('visibilitychange', onVisible)
    return () => document.removeEventListener('visibilitychange', onVisible)
  }, [isBuffer, fetchSeed])

  return seed
}

export function ChartWithTimeScale({
  bufferData,
  bufferSeries,
  engineEndpoint,
  historyMetrics,
  color,
  yDomain,
  unit,
  height = 160,
  title,
  compact = false,
  hideTooltipLabel,
  tooltipLabel,
  seriesLabel,
  seedValueMap,
  className,
  events,
  requests,
}: ChartWithTimeScaleProps) {
  const [scale, setScale] = useState<TimeScale>('5m')
  const isBuffer = scale === '1m' || scale === '5m'
  const showButton = engineEndpoint !== null && historyMetrics !== undefined

  const effectiveConfigs = useMemo(() => historyMetrics ?? [], [historyMetrics])

  // Fetch all needed history data (direct metrics + derived source metrics).
  const { data: historyData, loading: historyLoading } = useAllHistoryData(
    isBuffer ? null : engineEndpoint,
    isBuffer ? [] : effectiveConfigs,
    scale,
  )

  // DB seed for buffer modes (no-op at 1h/24h). Merged only when the live
  // buffer is sparse — see mergeSeedIntoLive.
  const seedData = useBufferSeed(engineEndpoint, effectiveConfigs, scale)

  const handleCycle = useCallback(() => setScale((s) => nextScale(s)), [])

  // ── Buffer mode (1m / 5m) ──
  if (isBuffer) {
    const sliceCount = BUFFER_SLICE[scale]
    const windowMs = BUFFER_WINDOW_MS[scale]
    const now = Date.now()
    const t0 = now - windowMs

    // TIME-cut (density-proof) with point-count as cheap pre-trim.
    const cut = (pts: DataPoint[]): DataPoint[] => {
      const pre = pts.length > sliceCount ? pts.slice(-sliceCount * 2) : pts
      return pre.filter((p) => p.timestamp >= t0)
    }

    let chartData: DataPoint[] | undefined
    let chartSeries: ChartSeries[] | undefined

    const denseThreshold = Math.floor(sliceCount * 0.75)

    if (bufferSeries) {
      chartSeries = bufferSeries.map((s, i) => {
        const cutLive = cut(s.data)
        const merged = mergeSeedIntoLive(cutLive, seedPointsFor(effectiveConfigs[i], seedData), {
          denseThreshold,
          valueMap: seedValueMap,
        })
        // Dense path returns `cutLive` by reference (already ≥ t0) — skip
        // the second filter/copy. The merged path can carry seed points
        // outside the window, so it always gets filtered.
        return { ...s, data: merged === cutLive ? merged : merged.filter((p) => p.timestamp >= t0) }
      })
    } else if (bufferData) {
      const cutLive = cut(bufferData)
      const merged = mergeSeedIntoLive(cutLive, seedPointsFor(effectiveConfigs[0], seedData), {
        denseThreshold,
        valueMap: seedValueMap,
      })
      chartData = merged === cutLive ? merged : merged.filter((p) => p.timestamp >= t0)
    }

    // Always render the FULL time domain, even when samples are sparse:
    // 1m == exactly 1 minute of x-axis, 5m == exactly 5.
    const bufferTimeDomain: [number, number] = [t0, now]

    return (
      <div className={`relative ${className ?? ''}`}>
        <TimeSeriesChart
          data={chartData}
          series={chartSeries}
          color={color}
          yDomain={yDomain}
          unit={unit}
          height={height}
          title={title}
          compact={compact}
          hideTooltipLabel={hideTooltipLabel}
          tooltipLabel={tooltipLabel}
          seriesLabel={seriesLabel}
          // 1m → 60 points (1/sec), 5m → 300 points (1/sec). Without this,
          // TimeSeriesChart defaults to maxPoints=60 and downsamples 5m to 60,
          // making 1m and 5m look identical.
          timeDomain={bufferTimeDomain}
          maxPoints={scale === '5m' ? 300 : 60}
          //pad={ false } // LIVE TOO: per-series padData fake-heads differ
          //  → mergeSeries() unions differing fake timestamps → 2× rows
          //  → x-domain doubles ( 5m shows 10 min ) + stale left half.
          //  Render real pts as-is (same lesson as history mode).
          pad={false}
          events={events}
          requests={requests}
        />
        {showButton && (
          <div className="absolute top-0.5 right-1 z-[2]">
            <TimeScaleButton scale={scale} onCycle={handleCycle} />
          </div>
        )}
      </div>
    )
  }

  // ── History mode (1h / 24h) ──
  // Compute a shared time domain so all charts in history mode span the
  // exact same range (e.g. 24h: [now-86400000, now]) and produce consistent
  // x-axis tick labels regardless of how many data points each metric has.
  const nowMs = Date.now()
  const rangeMs = scale === '1h' ? 3_600_000 : 86_400_000
  const historyTimeDomain: [number, number] = [nowMs - rangeMs, nowMs]

  // Build series from fetched history data. Each series is either:
  // - Direct: fetched by its own metric name
  // - Derived: computed from source metrics via compute()
  let chartSeries: ChartSeries[] | undefined
  let chartData: DataPoint[] | undefined

  if (bufferSeries && effectiveConfigs.length > 0) {
    // Multi-line mode
    chartSeries = []
    for (let i = 0; i < bufferSeries.length; i++) {
      const cfg = effectiveConfigs[i]
      if (!cfg) continue

      let dataPoints: DataPoint[] | null = null

      if (typeof cfg === 'string') {
        // Direct metric
        dataPoints = historyData.get(cfg) ?? null
      } else if (cfg && typeof cfg === 'object') {
        // Derived series — fetch sources and compute
        const sources: Record<string, DataPoint[] | null> = {}
        for (const m of cfg.sourceMetrics) {
          sources[m] = historyData.get(m) ?? null
        }
        // Only compute if at least one source has data
        const hasAnyData = Object.values(sources).some((d) => d && d.length > 0)
        if (hasAnyData) {
          dataPoints = cfg.compute(sources)
        }
      }

      if (!dataPoints || dataPoints.length === 0) continue
      chartSeries.push({
        data: dataPoints,
        label: bufferSeries[i].label,
        color: bufferSeries[i].color,
        axis: bufferSeries[i].axis,
      })
    }
  } else if (bufferData && effectiveConfigs.length > 0) {
    // Single-line mode
    const cfg = effectiveConfigs[0]
    if (cfg) {
      if (typeof cfg === 'string') {
        chartData = historyData.get(cfg) ?? undefined
      } else if (cfg && typeof cfg === 'object') {
        const sources: Record<string, DataPoint[] | null> = {}
        for (const m of cfg.sourceMetrics) {
          sources[m] = historyData.get(m) ?? null
        }
        const hasAnyData = Object.values(sources).some((d) => d && d.length > 0)
        if (hasAnyData) {
          chartData = cfg.compute(sources)
        }
      }
    }
  }

  const allEmpty = !chartSeries?.length && !chartData?.length

  return (
    <div className={`relative ${className ?? ''}`}>
      {historyLoading && allEmpty ? (
        <div className="flex items-center justify-center" style={{ height: typeof height === 'number' ? `${height}px` : height }}>
          <span className="text-xs text-zinc-500">Loading…</span>
        </div>
      ) : allEmpty ? (
        <div className="flex items-center justify-center" style={{ height: typeof height === 'number' ? `${height}px` : height }}>
          <span className="text-xs text-zinc-500">No historical data for this range</span>
        </div>
      ) : (
        <TimeSeriesChart
          data={chartData}
          series={chartSeries}
          color={color}
          yDomain={yDomain}
          unit={unit}
          height={height}
          title={title}
          compact={compact}
          hideTooltipLabel={hideTooltipLabel}
          tooltipLabel={tooltipLabel}
          seriesLabel={seriesLabel}
          maxPoints={scale === '1h' ? 360 : 240}
          pad={false}
          timeDomain={historyTimeDomain}
          // No events/requests in history mode — they're real-time only.
        />
      )}
      {showButton && (
        <div className="absolute top-0.5 right-1 z-[2]">
          <TimeScaleButton scale={scale} onCycle={handleCycle} />
        </div>
      )}
    </div>
  )
}
