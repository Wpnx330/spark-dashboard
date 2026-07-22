import { useState, useEffect, useCallback } from 'react'
import { useMetrics } from '@/hooks/useMetrics'

interface HistorySummary {
  delta_prompt_tokens: number
  delta_gen_tokens: number
  avg_decode_tps: number
  avg_prompt_tps: number
  peak_active_requests: number
  peak_queued_requests: number
  total_requests: number
  power_kwh: number
  total_seconds: number | null
  source_table: string
}

interface HistorySettings {
  enabled: boolean
  utc_offset: string
}

type WindowPreset =
  | 'today'
  | 'this_week'
  | 'this_month'
  | 'last_month'
  | 'this_year'
  | 'all_time'
  | 'custom'

interface DateRange {
  since_ms: number
  until_ms: number
  label: string
  description: string
}

function presetRange(preset: WindowPreset, utcOffsetHours: number): DateRange {
  const now = Date.now()
  const offsetMs = utcOffsetHours * 3600000
  // Convert "now" to user-local time for calendar calculations
  const localNow = now + offsetMs
  const localDate = new Date(localNow)
  const y = localDate.getUTCFullYear()
  const m = localDate.getUTCMonth()
  const d = localDate.getUTCDate()

  // Midnight in user's timezone, converted back to UTC ms
  const localMidnight = Date.UTC(y, m, d) - offsetMs
  const localSunday = Date.UTC(y, m, d - localDate.getUTCDay()) - offsetMs
  const localMonthStart = Date.UTC(y, m, 1) - offsetMs

  const fmtFull = (ms: number) => {
    const d = new Date(ms)
    return d.toLocaleDateString('en-US', {
      weekday: 'short',
      month: 'short',
      day: 'numeric',
      year: 'numeric',
      timeZone: 'UTC',
    })
  }

  switch (preset) {
    case 'today':
      return {
        since_ms: localMidnight,
        until_ms: now,
        label: 'Today',
        description: `Showing: data from ${fmtFull(localMidnight)} to present`,
      }
    case 'this_week': {
      const weekEnd = localSunday + 7 * 86400000
      return {
        since_ms: localSunday,
        until_ms: Math.min(weekEnd, now),
        label: 'This Week',
        description: `Showing: data from ${fmtFull(localSunday)} — ${fmtFull(Math.min(weekEnd, now))}`,
      }
    }
    case 'this_month': {
      let monthEnd: number
      if (m === 11) {
        monthEnd = Date.UTC(y + 1, 0, 1) - offsetMs
      } else {
        monthEnd = Date.UTC(y, m + 1, 1) - offsetMs
      }
      return {
        since_ms: localMonthStart,
        until_ms: Math.min(monthEnd, now),
        label: 'This Month',
        description: `Showing: data from ${fmtFull(localMonthStart)} — ${fmtFull(Math.min(monthEnd, now))}`,
      }
    }
    case 'last_month': {
      const lastMonthStart = Date.UTC(y, m - 1, 1) - offsetMs
      let lastMonthEnd: number
      if (m === 0) {
        lastMonthEnd = Date.UTC(y, 0, 1) - offsetMs
      } else {
        lastMonthEnd = Date.UTC(y, m, 1) - offsetMs
      }
      return {
        since_ms: lastMonthStart,
        until_ms: lastMonthEnd,
        label: 'Last Month',
        description: `Showing: data from ${fmtFull(lastMonthStart)} — ${fmtFull(lastMonthEnd)}`,
      }
    }
    case 'this_year': {
      const yearStart = Date.UTC(y, 0, 1) - offsetMs
      return {
        since_ms: yearStart,
        until_ms: now,
        label: 'This Year',
        description: `Showing: data from ${fmtFull(yearStart)} to present`,
      }
    }
    case 'all_time':
      return {
        since_ms: 0,
        until_ms: now,
        label: 'All Time',
        description: 'Showing: all recorded data',
      }
    default:
      return {
        since_ms: localMidnight,
        until_ms: now,
        label: 'Today',
        description: `Showing: data from ${fmtFull(localMidnight)} to present`,
      }
  }
}

export function HistoryView() {
  const [preset, setPreset] = useState<WindowPreset>('today')
  const [customSince, setCustomSince] = useState('')
  const [customUntil, setCustomUntil] = useState('')
  const [summary, setSummary] = useState<HistorySummary | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [settings, setSettings] = useState<HistorySettings | null>(null)
  const [showSettings, setShowSettings] = useState(false)

  // Load settings on mount, add utcOffset
  useEffect(() => {
    fetch('/api/history/settings')
      .then((r) => r.json())
      .then((data) => {
        setSettings(data)
        setUtcOffset(parseInt(data.utc_offset) || 0)
      })
      .catch(() => {})
  }, [])

  const [utcOffset, setUtcOffset] = useState(0)

  // Derive the engine key from live metrics (first engine's endpoint).
  const { metrics: liveMetrics } = useMetrics()
  const engineKey = liveMetrics?.engines?.[0]?.endpoint ?? 'vllm'

  // Fetch summary when preset or custom range changes
  const fetchSummary = useCallback(
    async (s: number, u: number, eng?: string) => {
      const key = eng ?? engineKey
      setLoading(true)
      setError(null)
      try {
        const res = await fetch(
          `/api/history/summary?engine=${encodeURIComponent(key)}&since_ms=${s}&until_ms=${u}`,
        )
        const data = await res.json()
        if (data.error) {
          setError(data.error)
          setSummary(null)
        } else {
          setSummary(data as HistorySummary)
        }
      } catch {
        setError('Failed to fetch history')
      } finally {
        setLoading(false)
      }
    },
    [engineKey],
  )

  // Auto-refresh the summary every 10 seconds while this view is active
  useEffect(() => {
    const id = setInterval(() => {
      const range = presetRange(preset, utcOffset)
      if (preset !== 'custom') {
        fetchSummary(range.since_ms, range.until_ms)
      }
    }, 10_000)
    return () => clearInterval(id)
  }, [preset, utcOffset, fetchSummary])

  // When preset changes, fetch
  useEffect(() => {
    const range = presetRange(preset, utcOffset)
    fetchSummary(range.since_ms, range.until_ms)
  }, [preset, fetchSummary])

  const handleCustomFetch = () => {
    const since = new Date(customSince).getTime()
    const until = customUntil ? new Date(customUntil).getTime() : Date.now()
    if (!isNaN(since) && !isNaN(until)) {
      fetchSummary(since, until)
    }
  }

  const hours = summary?.total_seconds ? (summary.total_seconds / 3600).toFixed(1) : '—'

  return (
    <div className="flex flex-col gap-4">
      {/* Header bar */}
      <div className="flex items-center gap-2 flex-wrap">
        <h2 className="text-sm font-semibold text-zinc-200 mr-2">Historical</h2>
        <div className="flex bg-[#1a1a1f] rounded-lg p-0.5 border border-white/[0.05] flex-wrap">
          {(
            [
              'today',
              'this_week',
              'this_month',
              'last_month',
              'this_year',
              'all_time',
            ] as WindowPreset[]
          ).map((p) => (
            <button
              key={p}
              onClick={() => setPreset(p)}
              className={`px-2.5 py-1 rounded-md text-[11px] font-medium transition-colors ${
                preset === p
                  ? 'bg-[#76B900] text-black shadow-sm'
                  : 'text-zinc-400 hover:text-zinc-200'
              }`}
            >
              {presetRange(p, utcOffset).label}
            </button>
          ))}
          <button
            onClick={() => setPreset('custom')}
            className={`px-2.5 py-1 rounded-md text-[11px] font-medium transition-colors ${
              preset === 'custom'
                ? 'bg-[#76B900] text-black shadow-sm'
                : 'text-zinc-400 hover:text-zinc-200'
            }`}
          >
            Custom
          </button>
        </div>
        {/* Date range description */}
        {preset !== 'custom' && (
          <div className="text-[10px] text-zinc-600 ml-2">
            {presetRange(preset, utcOffset).description}
          </div>
        )}
        <button
          onClick={() => setShowSettings(!showSettings)}
          className="ml-auto text-[11px] text-zinc-500 hover:text-zinc-300 transition-colors"
          title="Settings"
        >
          ⚙ Settings
        </button>
      </div>

      {/* Custom date inputs */}
      {preset === 'custom' && (
        <div className="flex items-center gap-2">
          <input
            type="datetime-local"
            value={customSince}
            onChange={(e) => setCustomSince(e.target.value)}
            className="bg-[#111115] border border-white/[0.06] rounded px-2 py-1 text-[11px] text-zinc-300"
          />
          <span className="text-zinc-500 text-[11px]">to</span>
          <input
            type="datetime-local"
            value={customUntil}
            onChange={(e) => setCustomUntil(e.target.value)}
            className="bg-[#111115] border border-white/[0.06] rounded px-2 py-1 text-[11px] text-zinc-300"
          />
          <button
            onClick={handleCustomFetch}
            className="px-2.5 py-1 rounded text-[11px] font-medium bg-[#76B900] text-black"
          >
            Go
          </button>
        </div>
      )}

      {/* Settings panel */}
      {showSettings && (
        <div className="bg-[#111115] rounded-lg border border-white/[0.06] p-4 space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="text-xs font-semibold text-zinc-200">History Settings</h3>
            <label className="flex items-center gap-2 cursor-pointer">
              <span className="text-[11px] text-zinc-400">Logging</span>
              <div
                className={`w-8 h-4 rounded-full transition-colors ${settings?.enabled ? 'bg-[#76B900]' : 'bg-zinc-600'}`}
                onClick={async () => {
                  const next = !settings?.enabled
                  await fetch('/api/history/toggle', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ enabled: next }),
                  })
                  setSettings({ ...settings!, enabled: next })
                }}
              >
                <div
                  className={`w-3.5 h-3.5 rounded-full bg-white mt-0.5 transition-transform ${settings?.enabled ? 'translate-x-4' : 'translate-x-0.5'}`}
                />
              </div>
            </label>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div>
              <label className="text-[10px] text-zinc-500 uppercase tracking-wider">
                UTC Offset
              </label>
              <input
                type="number"
                step="1"
                value={settings?.utc_offset !== undefined ? settings.utc_offset : '-4'}
                onChange={async (e) => {
                  const v = e.target.value
                  setSettings((s) => ({ ...s!, utc_offset: v }))
                  setUtcOffset(parseInt(v) || 0)
                  await fetch('/api/history/settings', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ utc_offset: v }),
                  })
                }}
                className="w-full bg-[#0d0d11] border border-white/[0.06] rounded px-2 py-1 text-[11px] text-zinc-300 mt-1"
                placeholder="-5"
              />
              <div className="text-[9px] text-zinc-500 mt-1 space-y-0.5">
                <div>
                  Server (UTC): {new Date().toISOString().slice(0, 19).replace('T', ' ')}
                </div>
                <div>
                  Your time (UTC{utcOffset > 0 ? '+' : ''}
                  {utcOffset}h):{' '}
                  {new Date(Date.now() + utcOffset * 3600000)
                    .toISOString()
                    .slice(0, 19)
                    .replace('T', ' ')}
                </div>
              </div>
            </div>
          </div>

          <p className="text-[10px] text-zinc-600 italic">
            Historical logging is off by default. Toggle it on above to start recording metrics.
            Data is auto-rolled up from 1s → hourly → daily — storage is minimal.
          </p>
        </div>
      )}

      {/* Main stats grid */}
      {loading && (
        <div className="text-xs text-zinc-500 py-8 text-center">Loading history...</div>
      )}
      {error && (
        <div className="text-xs text-yellow-400 py-8 text-center">
          {error === 'no data'
            ? 'No historical data yet. Enable logging in Settings and wait a moment.'
            : error}
        </div>
      )}

      {summary && (
        <>
          {/* Token metrics */}
          <div className="bg-[#111115] rounded-lg border border-white/[0.06] p-4">
            <h3 className="text-xs font-semibold text-zinc-300 mb-3">Token Throughput</h3>
            <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-4">
              <div>
                <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
                  Prompt Tokens
                </span>
                <div className="text-base font-bold text-zinc-100 font-mono">
                  {summary.delta_prompt_tokens.toLocaleString()}
                </div>
              </div>
              <div>
                <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
                  Generated Tokens
                </span>
                <div className="text-base font-bold text-zinc-100 font-mono">
                  {summary.delta_gen_tokens.toLocaleString()}
                </div>
              </div>
              <div>
                <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
                  Avg Prompt Tok/s
                </span>
                <div className="text-base font-bold text-zinc-100 font-mono">
                  {summary.avg_prompt_tps.toFixed(1)}
                </div>
              </div>
              <div>
                <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
                  Avg Decode Tok/s
                </span>
                <div className="text-base font-bold text-zinc-100 font-mono">
                  {summary.avg_decode_tps.toFixed(1)}
                </div>
              </div>
              <div>
                <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
                  Total Requests
                </span>
                <div className="text-base font-bold text-zinc-100 font-mono">
                  {summary.total_requests.toLocaleString()}
                </div>
              </div>
              <div>
                <span className="text-[10px] text-zinc-500 uppercase tracking-wider">Uptime</span>
                <div className="text-base font-bold text-zinc-100 font-mono">{hours}h</div>
              </div>
              <div>
                <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
                  Data Source
                </span>
                <div className="text-base font-bold text-zinc-100 font-mono">
                  {summary.source_table}
                </div>
              </div>
            </div>
          </div>

          {/* Peak metrics */}
          <div className="bg-[#111115] rounded-lg border border-white/[0.06] p-4">
            <h3 className="text-xs font-semibold text-zinc-300 mb-3">Peak Load</h3>
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
              <div>
                <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
                  Peak Active Requests
                </span>
                <div className="text-base font-bold text-zinc-100 font-mono">
                  {summary.peak_active_requests}
                </div>
              </div>
              <div>
                <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
                  Peak Queued Requests
                </span>
                <div className="text-base font-bold text-zinc-100 font-mono">
                  {summary.peak_queued_requests}
                </div>
              </div>
              <div>
                <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
                  Energy Used
                </span>
                <div className="text-base font-bold text-zinc-100 font-mono">
                  {summary.power_kwh.toFixed(4)} kWh
                </div>
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  )
}
