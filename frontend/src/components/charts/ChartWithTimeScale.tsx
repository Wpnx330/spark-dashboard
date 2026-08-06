import { useState, useMemo } from 'react'
import { TimeSeriesChart, type ChartSeries } from './TimeSeriesChart'
import { TimeScaleButton, type TimeScale } from './TimeScaleButton'
import { useHistoryTimeseries } from '@/hooks/useHistoryTimeseries'

interface DataPoint {
  timestamp: number
  value: number
}

interface ChartWithTimeScaleProps {
  /** Single-line buffer mode (1m/5m). */
  bufferData?: DataPoint[]
  /** Multi-line buffer mode (1m/5m). */
  bufferSeries?: ChartSeries[]
  /** Engine endpoint for history API calls (1h/24h). If null, button is hidden. */
  engineEndpoint: string | null
  /**
   * Map each series to a history metric. Only series with a defined metric
   * are shown in 1h/24h mode. For single-line: one entry. For multi-line:
   * array matching bufferSeries order. `undefined` = no history equivalent.
   */
  historyMetrics?: (string | undefined)[]
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

/** Hook helper: fetch history data for multiple metrics. */
function useHistoryForMetrics(
  engineEndpoint: string | null,
  metrics: (string | undefined)[],
  scale: TimeScale,
): Array<{ data: DataPoint[] | null; loading: boolean; error: string | null }> {
  // Call the hook unconditionally for each metric slot (including undefined)
  // so hook count stays stable across renders. For undefined metrics, the
  // hook returns null immediately (scale is irrelevant, but the call must
  // exist).
  //
  // We can't call hooks in a loop with dynamic count, so we dispatch to
  // fixed-slot helpers up to a max, then inline. The current max is 4
  // (Latency: TTFT + Queue + ITL + TPOT). We'll use individual calls.
  //
  // Actually, React rules require hooks to be called in the same order every
  // render, but we CAN call them in a loop as long as the array length is
  // constant. historyMetrics length is constant for a given chart instance.
  // eslint-disable-next-line react-hooks/rules-of-hooks
  return metrics.map((metric) => {
    // For undefined metrics, we still need to call the hook the same number
    // of times. Pass a dummy that returns null for buffer scales.
    const { data, loading, error } = useHistoryTimeseries(
      metric ? engineEndpoint : null,
      metric ?? '__none__',
      scale,
    )
    // Map API format { timestamp_ms, value } → chart format { timestamp, value }
    const mapped: DataPoint[] | null = data
      ? data.map((p) => ({ timestamp: p.timestamp_ms, value: p.value }))
      : null
    return { data: mapped, loading, error }
  })
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

  // For history mode, determine which metrics to fetch.
  // If historyMetrics not provided, use a single-element array from color mode.
  const effectiveHistoryMetrics = useMemo(() => {
    if (historyMetrics) return historyMetrics
    return undefined
  }, [historyMetrics])

  // Fetch history data for each metric (only used in 1h/24h mode).
  // The hook is called unconditionally; it returns null for buffer scales.
  const historyResults = effectiveHistoryMetrics
    ? useHistoryForMetrics(engineEndpoint, effectiveHistoryMetrics, scale)
    : []

  const handleCycle = () => setScale((s) => nextScale(s))

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
  // Build series from fetched history data. Only include series that have a
  // defined historyMetric AND received non-empty data.
  const isLoading = historyResults.some((r) => r.loading)

  let chartSeries: ChartSeries[] | undefined
  let chartData: DataPoint[] | undefined

  if (bufferSeries && effectiveHistoryMetrics) {
    // Multi-line mode
    chartSeries = []
    for (let i = 0; i < bufferSeries.length; i++) {
      const metric = effectiveHistoryMetrics[i]
      if (!metric) continue
      const result = historyResults[i]
      if (!result.data || result.data.length === 0) continue
      chartSeries.push({
        data: result.data,
        label: bufferSeries[i].label,
        color: bufferSeries[i].color,
        axis: bufferSeries[i].axis,
      })
    }
  } else if (bufferData && effectiveHistoryMetrics) {
    // Single-line mode
    const result = historyResults[0]
    if (result.data && result.data.length > 0) {
      chartData = result.data
    }
  }

  const allEmpty = !chartSeries?.length && !chartData?.length

  return (
    <div className={`relative ${className ?? ''}`}>
      {isLoading && allEmpty ? (
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
