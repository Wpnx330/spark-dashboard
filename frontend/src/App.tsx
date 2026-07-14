import { useState, useMemo, useEffect } from 'react'
import { useMetrics } from './hooks/useMetrics'
import { useMetricsHistory } from './hooks/useMetricsHistory'
import { ConnectionBadge } from './components/ConnectionBadge'
import { Dashboard } from './components/views/Dashboard'
import type { GpuEvent, InferenceRequest } from './types/events'

type MobileView = 'both' | 'model' | 'hardware'

function App() {
  const { metrics, connectionStatus, isStale } = useMetrics()
  const [mobileView, setMobileView] = useState<MobileView>('both')

  const history = useMetricsHistory(metrics)

  const { getEvents, getRequests } = history

  // Track narrow screen for layout adjustments. Both view on mobile
  // shouldn't force flex-1 so content determines height.
  const [isNarrow, setIsNarrow] = useState(() => window.innerWidth < 768)
  useEffect(() => {
    const onResize = () => setIsNarrow(window.innerWidth < 768)
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  }, [])

  const events = useMemo((): GpuEvent[] =>
    getEvents().map((e) => ({
      timestamp_ms: e.timestamp_ms,
      event_type: e.event_type as GpuEvent['event_type'],
      detail: e.detail,
    })),
    [getEvents],
  )

  const requests = useMemo((): InferenceRequest[] =>
    getRequests().map((r) => ({
      start_ms: r.start_ms,
      end_ms: r.end_ms,
      tps: r.tokens_per_sec,
      ttft_ms: r.ttft_ms,
    })),
    [getRequests],
  )

  return (
    <div className="h-dvh flex flex-col bg-[#08080a] overflow-hidden">
      <header className="shrink-0 flex items-center justify-between px-3 lg:px-4 2xl:px-5 min-[1920px]:px-6 py-2 border-b border-white/5">
        <div className="flex items-center gap-2">
          <h1 className="text-sm lg:text-base font-bold text-zinc-100">Spark Dashboard</h1>

          {/* Mobile view toggle — visible on narrow screens only */}
          <div className="flex items-center gap-0.5 ml-3 bg-white/5 rounded-md p-0.5 text-xs">
            <button
              onClick={() => setMobileView('both')}
              className={`px-2.5 py-1 rounded-md text-[11px] font-medium transition-colors ${
                mobileView === 'both'
                  ? 'bg-[#76B900] text-black shadow-sm'
                  : 'text-zinc-400 hover:text-zinc-200'
              }`}
            >
              Both
            </button>
            <button
              onClick={() => setMobileView('model')}
              className={`px-2.5 py-1 rounded-md text-[11px] font-medium transition-colors ${
                mobileView === 'model'
                  ? 'bg-[#76B900] text-black shadow-sm'
                  : 'text-zinc-400 hover:text-zinc-200'
              }`}
            >
              Model
            </button>
            <button
              onClick={() => setMobileView('hardware')}
              className={`px-2.5 py-1 rounded-md text-[11px] font-medium transition-colors ${
                mobileView === 'hardware'
                  ? 'bg-[#76B900] text-black shadow-sm'
                  : 'text-zinc-400 hover:text-zinc-200'
              }`}
            >
              Hardware
            </button>
          </div>
        </div>

        <ConnectionBadge status={connectionStatus} isStale={isStale} />
      </header>

      <main className={`flex-1 min-h-0 flex flex-col p-3 lg:p-4 2xl:p-5 min-[1920px]:p-6 ${isStale ? 'opacity-50' : ''}`}>
        {!metrics && connectionStatus !== 'connected' && (
          <div className="flex-1 flex items-center justify-center">
            <div className="text-center">
              <h2 className="text-xl font-bold text-zinc-50 mb-2">Waiting for metrics</h2>
              <p className="text-sm text-zinc-400">Connecting to backend...</p>
            </div>
          </div>
        )}

        {(() => {
          const filter = mobileView === 'hardware' ? 'hardware' : mobileView === 'model' ? 'model' : undefined
          return (
            <Dashboard
              metrics={metrics}
              history={history}
              events={events}
              requests={requests}
              filterView={filter}
              fillHeight={!isNarrow || filter !== undefined}
            />
          )
        })()}
      </main>
    </div>
  )
}

export default App