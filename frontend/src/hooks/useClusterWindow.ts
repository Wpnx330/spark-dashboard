import { useEffect, useState } from 'react'

/** Subset of `/api/meta` the Command tab consumes. */
export interface ClusterMeta {
  title?: string
  /** History `engine_key` of the primary engine (auto-detected endpoint). */
  engine_key?: string
  /** Name of the model the primary engine is serving, if known. */
  model?: string | null
  /** Peer dashboard URL from SPARK_DASHBOARD_PEER_URL (null = unset). */
  peer_url?: string | null
}

/** Subset of `/api/history/settings` the Command tab consumes. */
export interface ClusterRates {
  cloud_prompt_rate: string
  cloud_gen_rate: string
  electricity_rate: string
  /** Hours offset from UTC used to define calendar days (e.g. "-4"). */
  utc_offset?: string
}

/** Subset of `/api/history/summary` the Command tab consumes. */
export interface ClusterSummary {
  delta_prompt_tokens: number
  delta_gen_tokens: number
  avg_decode_tps: number
  avg_prompt_tps: number
  peak_active_requests: number
  peak_queued_requests: number
  total_requests: number
  total_preemptions: number | null
  power_kwh: number
  total_seconds: number | null
  source_table: string
}

export interface ClusterWindowState {
  /** Present only for peer clusters (`base !== ''`): peer's `/api/meta`. */
  meta: ClusterMeta | null
  rates: ClusterRates | null
  summary: ClusterSummary | null
  /** Max TTFT-p95 over the window, from the `ttft_ms` timeseries. */
  ttftP95MaxMs: number | null
  /** Max GPU temperature over the window, from the `gpu_temp` timeseries. */
  gpuTempMaxC: number | null
  loading: boolean
  error: string | null
}

interface UseClusterWindowOptions {
  /**
   * Resolve the history `engine_key` for summary queries. Precedence:
   * meta.engine_key (works for auto-detected engines once meta loads) ->
   * `fallbackEngineKey` -> 'vllm'.
   */
  fallbackEngineKey: string | null
  since_ms: number
  until_ms: number
  /** Skip all fetching (e.g. a peer slot when no peer is configured). */
  enabled: boolean
  /** Poll interval for summary/timeseries (ms); 0 disables polling. */
  refreshMs?: number
}

/**
 * Fetch one cluster's history window (summary + p95/temp series + rate
 * settings) directly from `base` — empty string for the local dashboard,
 * a peer dashboard URL for the peer. Same endpoints, same params, whether
 * local or remote: the Command tab never forks the data path.
 *
 * All requests are plain GETs against the fixed base URL (no user input in
 * the URL), so the SSRF surface is bounded to the configured peer host.
 * `fetch` is read off `globalThis` so tests can stub it.
 */
export function useClusterWindow(
  base: string,
  opts: UseClusterWindowOptions,
): ClusterWindowState {
  const { fallbackEngineKey, since_ms, until_ms, enabled, refreshMs = 10_000 } =
    opts
  const [meta, setMeta] = useState<ClusterMeta | null>(null)
  const [rates, setRates] = useState<ClusterRates | null>(null)
  const [summary, setSummary] = useState<ClusterSummary | null>(null)
  const [ttftP95MaxMs, setTtftP95MaxMs] = useState<number | null>(null)
  const [gpuTempMaxC, setGpuTempMaxC] = useState<number | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Cluster meta (title, peer_url, engine_key, model) + rate settings:
  // fetched once per base URL. For the local dashboard (base === ''),
  // these hit the relative /api/meta and /api/history/settings.
  useEffect(() => {
    if (!enabled) return
    let cancelled = false
    async function load() {
      try {
        const m = await fetch(`${base}/api/meta`)
        const metaData = m.ok ? ((await m.json()) as ClusterMeta) : null
        const s = await fetch(`${base}/api/history/settings`)
        const ratesData: ClusterRates | null = s.ok ? await s.json() : null
        if (cancelled) return
        setMeta(metaData)
        setRates(ratesData)
      } catch {
        if (!cancelled) setMeta(null)
      }
    }
    load()
    return () => {
      cancelled = true
    }
  }, [base, enabled])

  // Windowed data: summary + TTFT-p95/temp series, on range change + poll.
  useEffect(() => {
    if (!enabled) return
    let cancelled = false
    const engine = meta?.engine_key ?? fallbackEngineKey ?? 'vllm'

    async function loadWindow() {
      setLoading(true)
      setError(null)
      try {
        const qs =
          `engine=${encodeURIComponent(engine)}` +
          `&since_ms=${since_ms}&until_ms=${until_ms}`
        const [sumRes, ttftRes, tempRes] = await Promise.all([
          fetch(`${base}/api/history/summary?${qs}`),
          fetch(`${base}/api/history/timeseries?metric=ttft_ms&${qs}`),
          fetch(`${base}/api/history/timeseries?metric=gpu_temp&${qs}`),
        ])
        if (cancelled) return
        const sumData = await sumRes.json()
        if (sumData?.error) {
          setSummary(null)
          setError(sumData.error as string)
        } else {
          setSummary(sumData as ClusterSummary)
        }
        const maxOf = (
          points: { value: number }[] | null | undefined,
        ): number | null =>
          points && points.length > 0
            ? Math.max(...points.map((p) => p.value))
            : null
        const ttftData = ttftRes.ok ? await ttftRes.json() : null
        setTtftP95MaxMs(maxOf(ttftData?.points))
        const tempData = tempRes.ok ? await tempRes.json() : null
        setGpuTempMaxC(maxOf(tempData?.points))
      } catch {
        if (!cancelled) setError('Failed to fetch history')
      } finally {
        if (!cancelled) setLoading(false)
      }
    }

    loadWindow()
    if (refreshMs <= 0) return
    const id = setInterval(loadWindow, refreshMs)
    return () => {
      cancelled = true
      clearInterval(id)
    }
  }, [base, enabled, meta, fallbackEngineKey, since_ms, until_ms, refreshMs])

  return {
    meta,
    rates,
    summary,
    ttftP95MaxMs,
    gpuTempMaxC,
    loading,
    error,
  }
}
