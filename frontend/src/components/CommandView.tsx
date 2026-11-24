import { useEffect, useMemo, useState } from 'react'
import type { MetricsSnapshot } from '@/types/metrics'
import type { ConnectionStatus } from '@/hooks/useMetrics'
import {
  useClusterWindow,
  type ClusterSummary,
} from '@/hooks/useClusterWindow'
import {
  COMMAND_WINDOWS,
  commandWindowRange,
  localMidnightMs,
  type CommandWindowKind,
} from '@/lib/commandWindow'
import { computeSavings, sumSavings } from '@/lib/savings'

/** Physics: 1 kWh = 3.6e6 J; avg watts = kWh * 3.6e6 / seconds. */
function avgWatts(summary: ClusterSummary | null): number | null {
  if (!summary || !summary.total_seconds || summary.total_seconds <= 0) {
    return null
  }
  return (summary.power_kwh * 3_600_000) / summary.total_seconds
}

const fmtInt = (n: number | null | undefined): string =>
  n == null ? '—' : Math.round(n).toLocaleString('en-US')
const fmtRate = (n: number | null | undefined): string =>
  n == null ? '—' : n.toFixed(1)
const fmtMs = (n: number | null | undefined): string =>
  n == null ? '—' : `${Math.round(n)}ms`
const fmtTemp = (n: number | null | undefined): string =>
  n == null ? '—' : `${Math.round(n)}°C`
const fmtWatts = (n: number | null | undefined): string =>
  n == null ? '—' : `${Math.round(n)}W`
const fmtUsd = (n: number): string =>
  `${n < 0 ? '-' : ''}$${Math.abs(n).toFixed(4)}`

interface CardValues {
  /** Card title: meta-driven (runtime config), never hardcoded. */
  title: string
  servedModel: string | null
  /** 'ok' | 'degraded' | 'down' — drives the status dot. */
  status: 'ok' | 'degraded' | 'down'
  active: number | null
  queued: number | null
  activeLabel: string
  prefillTps: number | null
  decodeTps: number | null
  ttftP95Ms: number | null
  totalRequests: number | null
  powerWatts: number | null
  gpuTempC: number | null
  /** True when this card renders placeholder content (no peer configured). */
  placeholder?: boolean
  /** Shown inside placeholder cards instead of an empty shell. */
  placeholderNote?: string
}

/**
 * One cluster card. `values.title` always comes from `/api/meta` (local or
 * peer) — D2 forbids hardcoding hostnames/IPs/titles.
 */
function ClusterCard({ values }: { values: CardValues }) {
  const statusColor =
    values.status === 'ok'
      ? 'bg-[#76B900] shadow-[0_0_6px_rgba(118,185,0,0.7)]'
      : values.status === 'degraded'
        ? 'bg-amber-400'
        : 'bg-zinc-600'
  return (
    <div
      className={`bg-[#111115] rounded-lg border border-white/[0.06] p-4 flex flex-col gap-3 ${
        values.placeholder ? 'opacity-50' : ''
      }`}
    >
      <div className="flex items-center gap-2 min-w-0">
        <span className={`w-2.5 h-2.5 rounded-full shrink-0 ${statusColor}`} />
        <h3
          className="text-sm font-semibold text-zinc-200 truncate"
          data-testid="cluster-title"
        >
          {values.title}
        </h3>
      </div>
      {values.servedModel && (
        <div className="text-[10px] text-zinc-500 truncate -mt-2">
          Serving: {values.servedModel}
        </div>
      )}
      {values.placeholderNote && (
        <div className="text-xs text-zinc-500">
          {values.placeholderNote}
        </div>
      )}

      <div className="grid grid-cols-2 gap-3">
        <div>
          <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
            {values.activeLabel}
          </span>
          <div className="text-4xl font-bold text-[#76B900] font-mono leading-tight">
            {fmtInt(values.active)}
          </div>
        </div>
        <div>
          <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
            Queued
          </span>
          <div className="text-4xl font-bold text-zinc-100 font-mono leading-tight">
            {fmtInt(values.queued)}
          </div>
        </div>
      </div>

      <div className="text-xs text-zinc-400 font-mono flex flex-wrap gap-x-3 gap-y-0.5">
        <span>Prefill {fmtRate(values.prefillTps)} tok/s</span>
        <span>Decode {fmtRate(values.decodeTps)} tok/s</span>
        <span>TTFT p95 {fmtMs(values.ttftP95Ms)}</span>
        <span>Req {fmtInt(values.totalRequests)}</span>
      </div>

      <div className="mt-auto pt-2 border-t border-white/[0.04] flex justify-between text-xs font-mono text-zinc-500">
        <span>Power {fmtWatts(values.powerWatts)}</span>
        <span>GPU {fmtTemp(values.gpuTempC)}</span>
      </div>
    </div>
  )
}

function SavingsBand({
  parts,
  label,
}: {
  parts: { title: string; net: number | null }[]
  label: string
}) {
  const total = sumSavings(
    parts.map((p) => ({
      cloudCost: 0,
      powerCost: 0,
      powerKwh: 0,
      netSavings: p.net ?? 0,
    })),
  ).netSavings
  return (
    <div className="bg-[#111115] rounded-lg border border-white/[0.06] p-4">
      <div className="flex items-baseline justify-between gap-3 flex-wrap">
        <h3 className="text-xs font-semibold text-zinc-300">
          Savings — {label}
        </h3>
        <div className="text-right">
          <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
            Grand Total
          </span>
          <div
            className={`text-2xl font-bold font-mono ${
                total >= 0 ? 'text-[#76B900]' : 'text-red-400'
              }`}
            data-testid="grand-total"
          >
            {fmtUsd(total)}
          </div>
        </div>
      </div>
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 mt-3">
        {parts.map((p) => (
          <div
            key={p.title}
            className="flex items-baseline justify-between gap-2 bg-[#0d0d11] rounded px-3 py-2"
          >
            <span className="text-[11px] text-zinc-500 truncate">{p.title}</span>
            <span
              className={`text-base font-bold font-mono ${
                (p.net ?? 0) >= 0 ? 'text-[#76B900]' : 'text-red-400'
              }`}
            >
              {p.net == null ? '—' : fmtUsd(p.net)}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

export interface CommandViewProps {
  /** Dashboard title from local /api/meta (App already holds it). */
  localTitle: string
  /** Live WS snapshot for the local cluster (instantaneous values). */
  metrics: MetricsSnapshot | null
  connectionStatus: ConnectionStatus
  isStale: boolean
}

export function CommandView({
  localTitle,
  metrics,
  connectionStatus,
  isStale,
}: CommandViewProps) {
  const [kind, setKind] = useState<CommandWindowKind>('live')
  // Re-evaluate "now" periodically so rolling windows and Live's
  // today-bounds advance without user interaction.
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 10_000)
    return () => clearInterval(id)
  }, [])

  // UTC offset comes from the dashboard's own settings (same input the
  // Historical view uses), so day boundaries agree across tabs.
  const localBase = useClusterWindow('', {
    fallbackEngineKey: metrics?.engines?.[0]?.endpoint ?? null,
    since_ms: 0,
    until_ms: now,
    enabled: true,
    refreshMs: 0,
  })
  const utcOffset = parseInt(localBase.rates?.utc_offset ?? '0') || 0

  const range = commandWindowRange(kind, utcOffset, now)
  const today = useMemo(
    () => ({
      since_ms: localMidnightMs(now, utcOffset),
      until_ms: now,
    }),
    [now, utcOffset],
  )
  const quick60 = useMemo(
    () => ({ since_ms: now - 60_000, until_ms: now }),
    [now],
  )

  // Local cluster: windowed aggregate (or today-only savings when live).
  const localWindow = useClusterWindow('', {
    fallbackEngineKey: metrics?.engines?.[0]?.endpoint ?? null,
    since_ms: range.isLive ? today.since_ms : range.since_ms,
    until_ms: range.until_ms,
    enabled: true,
  })
  const peerUrl = localBase.meta?.peer_url ?? null

  // Peer engine-key fallback: peers running an older binary report no
  // meta.engine_key; both hosts deploy vLLM locally, so the local engine
  // key is the best guess before the 'vllm' default.
  const peerFallbackEngine = metrics?.engines?.[0]?.endpoint ?? null

  const peerWindow = useClusterWindow(peerUrl ?? '', {
    fallbackEngineKey: peerFallbackEngine,
    since_ms: range.isLive ? today.since_ms : range.since_ms,
    until_ms: range.until_ms,
    enabled: peerUrl != null,
  })
  // Live-mode peer counters: last-60s aggregate ≈ instantaneous, and the
  // only way to see the peer "now" without a cross-origin WebSocket.
  const peerLive = useClusterWindow(peerUrl ?? '', {
    fallbackEngineKey: peerFallbackEngine,
    since_ms: quick60.since_ms,
    until_ms: quick60.until_ms,
    enabled: peerUrl != null && range.isLive,
    refreshMs: 5_000,
  })

  const liveMetrics = metrics?.engines?.[0]?.metrics ?? null
  const liveModel = metrics?.engines?.[0]?.model?.name ?? null

  const localStatus: CardValues['status'] =
    connectionStatus === 'connected'
      ? isStale
        ? 'degraded'
        : 'ok'
      : 'down'
  const peerStatus: CardValues['status'] = peerUrl
      ? peerWindow.error
        ? 'degraded'
        : peerWindow.summary || peerWindow.meta
          ? 'ok'
          : 'down'
      : 'down'

  // --- Card values per mode -------------------------------------------------
  const localCard: CardValues = range.isLive
    ? {
        title: localTitle || localBase.meta?.title || 'Local Cluster',
        servedModel: liveModel ?? localBase.meta?.model ?? null,
        status: localStatus,
        active: liveMetrics?.active_requests ?? null,
        queued: liveMetrics?.queued_requests ?? null,
        activeLabel: 'Active Now',
        prefillTps: liveMetrics?.prompt_tokens_per_sec ?? null,
        decodeTps: liveMetrics?.tokens_per_sec ?? null,
        ttftP95Ms: liveMetrics?.ttft_percentiles?.p95_ms ?? null,
        totalRequests: liveMetrics?.total_requests ?? null,
        powerWatts: metrics?.gpu?.power_watts ?? null,
        gpuTempC: metrics?.gpu?.temperature_celsius ?? null,
      }
    : {
        title: localTitle || localBase.meta?.title || 'Local Cluster',
        servedModel: liveModel ?? localBase.meta?.model ?? null,
        status: localStatus,
        active: localWindow.summary?.peak_active_requests ?? null,
        queued: localWindow.summary?.peak_queued_requests ?? null,
        activeLabel: 'Peak Active',
        prefillTps: localWindow.summary?.avg_prompt_tps ?? null,
        decodeTps: localWindow.summary?.avg_decode_tps ?? null,
        ttftP95Ms: localWindow.ttftP95MaxMs,
        totalRequests: localWindow.summary?.total_requests ?? null,
        powerWatts: avgWatts(localWindow.summary),
        gpuTempC: localWindow.gpuTempMaxC,
      }

  const peerCard: CardValues = !peerUrl
    ? {
        title: 'Peer Cluster',
        servedModel: null,
        status: 'down',
        active: null,
        queued: null,
        activeLabel: 'Active Now',
        prefillTps: null,
        decodeTps: null,
        ttftP95Ms: null,
        totalRequests: null,
        powerWatts: null,
        gpuTempC: null,
        placeholder: true,
        placeholderNote:
          'Peer not configured (SPARK_DASHBOARD_PEER_URL)',
      }
    : range.isLive
      ? {
          title: peerWindow.meta?.title ?? peerLive.meta?.title ?? 'Peer Cluster',
          servedModel: peerWindow.meta?.model ?? peerLive.meta?.model ?? null,
          status: peerStatus,
          active: peerLive.summary?.peak_active_requests ?? null,
          queued: peerLive.summary?.peak_queued_requests ?? null,
          activeLabel: 'Active Now',
          prefillTps: peerLive.summary?.avg_prompt_tps ?? null,
          decodeTps: peerLive.summary?.avg_decode_tps ?? null,
          ttftP95Ms: peerLive.ttftP95MaxMs,
          totalRequests: peerLive.summary?.total_requests ?? null,
          powerWatts: avgWatts(peerLive.summary),
          gpuTempC: peerLive.gpuTempMaxC,
        }
      : {
          title: peerWindow.meta?.title ?? 'Peer Cluster',
          servedModel: peerWindow.meta?.model ?? null,
          status: peerStatus,
          active: peerWindow.summary?.peak_active_requests ?? null,
          queued: peerWindow.summary?.peak_queued_requests ?? null,
          activeLabel: 'Peak Active',
          prefillTps: peerWindow.summary?.avg_prompt_tps ?? null,
          decodeTps: peerWindow.summary?.avg_decode_tps ?? null,
          ttftP95Ms: peerWindow.ttftP95MaxMs,
          totalRequests: peerWindow.summary?.total_requests ?? null,
          powerWatts: avgWatts(peerWindow.summary),
          gpuTempC: peerWindow.gpuTempMaxC,
        }

  // --- Savings band (same math as Historical — lib/savings) ----------------
  const localRates = localWindow.rates
  const peerRates = peerWindow.rates
  const localSavings = computeSavings(localWindow.summary, {
    cloud_prompt_rate: parseFloat(localRates?.cloud_prompt_rate || '0') || 0,
    cloud_gen_rate: parseFloat(localRates?.cloud_gen_rate || '0') || 0,
    electricity_rate: parseFloat(localRates?.electricity_rate || '0') || 0,
  }).netSavings
  const peerSavings = peerUrl
    ? computeSavings(peerWindow.summary, {
        cloud_prompt_rate:
          parseFloat(peerRates?.cloud_prompt_rate || '0') || 0,
        cloud_gen_rate: parseFloat(peerRates?.cloud_gen_rate || '0') || 0,
        electricity_rate:
          parseFloat(peerRates?.electricity_rate || '0') || 0,
      }).netSavings
    : null

  return (
    <div className="flex flex-col gap-4">
      {/* Control row: dropdown full-width above cards on phones, top-right
          on md+ (D4). Applies to the WHOLE page. */}
      <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-2">
        <h2 className="text-sm font-semibold text-zinc-200">Command</h2>
        <div className="flex bg-[#1a1a1f] rounded-lg p-0.5 border border-white/[0.05] w-full md:w-auto">
          {COMMAND_WINDOWS.map((w) => (
            <button
              key={w}
              onClick={() => setKind(w)}
              data-testid={`cmd-window-${w}`}
              className={`flex-1 md:flex-none px-2.5 py-1 rounded-md text-xs md:text-[11px] font-medium transition-colors ${
                kind === w
                  ? 'bg-[#76B900] text-black'
                  : 'text-zinc-400 hover:text-zinc-200'
              }`}
            >
              {w === 'live'
                ? 'Live'
                : w === '24h'
                  ? '24h'
                  : w === '7d'
                    ? '7d'
                    : w === '30d'
                      ? '30d'
                      : '1y'}
            </button>
          ))}
        </div>
      </div>

      {/* Row 1 — the two clusters (D2: NOT a 2x2 grid of full panels). */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <ClusterCard values={localCard} />
        <ClusterCard values={peerCard} />
      </div>

      {/* Row 2 — single full-width savings band. */}
      <SavingsBand
        label={range.isLive ? "Today (Live)" : range.label}
        parts={[
          { title: localCard.title, net: localSavings },
          { title: peerCard.title, net: peerSavings },
        ]}
      />

      {!peerUrl && (
        <div className="text-[10px] text-zinc-600">
          Set SPARK_DASHBOARD_PEER_URL on this host to enable the peer card.
        </div>
      )}
    </div>
  )
}
