import { useState, useMemo, useCallback } from 'react'
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
const BUFFER_SLICE: Record<'1m' | '5m', number> = {
  '1m': 60,
  '5m': 300,
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

  const handleCycle = useCallback(() => setScale((s) => nextScale(s)), [])

  // ── Buffer mode (1m / 5m) ──
  if (isBuffer) {
    const sliceCount = BUFFER_SLICE[scale]

    let chartData: DataPoint[] | undefined
    let chartSeries: ChartSeries[] | undefined

    if (bufferSeries) {
      chartSeries = bufferSeries.map((s) => ({
        ...s,
        data: s.data.slice(-sliceCount),
      }))
    } else if (bufferData) {
      chartData = bufferData.slice(-sliceCount)
    }

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
