import { useState, useEffect, useRef } from 'react'

export type TimeScale = '1m' | '5m' | '1h' | '24h'

export interface TimeSeriesPoint {
  timestamp_ms: number
  value: number
}

interface UseHistoryTimeseriesResult {
  data: TimeSeriesPoint[] | null
  loading: boolean
  error: string | null
}

/**
 * Fetch historical timeseries data from the `/api/history/timeseries` endpoint.
 *
 * - For `1m` and `5m`: returns `{ data: null, loading: false, error: null }`
 *   to signal the caller should use in-memory buffer data instead.
 * - For `1h` and `24h`: fetches from the API and refetches on an interval
 *   (every 30s for 1h, every 5min for 24h).
 *
 * The returned `data` is in the raw `{ timestamp_ms, value }` format from the
 * API. The caller (ChartWithTimeScale) maps it to `{ timestamp, value }`
 * (DataPoint) for the chart.
 */
export function useHistoryTimeseries(
  engineEndpoint: string | null,
  metric: string,
  scale: TimeScale,
): UseHistoryTimeseriesResult {
  const [data, setData] = useState<TimeSeriesPoint[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // Track the fetch "generation" so stale responses from a previous
  // metric/scale don't overwrite a newer one.
  const genRef = useRef(0)

  const isHistory = scale === '1h' || scale === '24h'

  useEffect(() => {
    // Buffer modes — caller uses in-memory data instead.
    if (!isHistory) {
      setData(null)
      setLoading(false)
      setError(null)
      return
    }

    // No endpoint or metric → can't fetch.
    if (!engineEndpoint || !metric) {
      setData(null)
      setLoading(false)
      setError(null)
      return
    }

    const intervalMs = scale === '1h' ? 30_000 : 300_000
    const rangeMs = scale === '1h' ? 3_600_000 : 86_400_000
    const gen = ++genRef.current

    let cancelled = false

    async function fetchData() {
      const untilMs = Date.now()
      const sinceMs = untilMs - rangeMs
      const url =
        `/api/history/timeseries?engine=${encodeURIComponent(engineEndpoint!)}` +
        `&metric=${encodeURIComponent(metric)}` +
        `&since_ms=${sinceMs}&until_ms=${untilMs}`

      try {
        const res = await fetch(url)
        if (!res.ok) {
          const body = await res.json().catch(() => ({}))
          throw new Error(body.error ?? `HTTP ${res.status}`)
        }
        const json = await res.json()
        if (cancelled || gen !== genRef.current) return
        setData(json.points ?? [])
        setError(null)
        setLoading(false)
      } catch (err) {
        if (cancelled || gen !== genRef.current) return
        setError(err instanceof Error ? err.message : String(err))
        setLoading(false)
      }
    }

    setLoading(true)
    fetchData()
    const interval = setInterval(fetchData, intervalMs)

    return () => {
      cancelled = true
      clearInterval(interval)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [engineEndpoint, metric, scale, isHistory])

  return { data, loading, error }
}
