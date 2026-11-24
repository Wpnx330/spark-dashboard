import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import type { MetricsSnapshot } from '@/types/metrics'

// --- Mock useMetrics so CommandView doesn't open a real WebSocket ---
const mockMetricsRef = { current: null as MetricsSnapshot | null }
const mockConn = { status: 'connected' as 'connected' | 'disconnected' }

vi.mock('@/hooks/useMetrics', async (importOriginal) => {
  const orig =
    await importOriginal<typeof import('@/hooks/useMetrics')>()
  return {
    ...orig,
    useMetrics: () => ({
      metrics: mockMetricsRef.current,
      connectionStatus: mockConn.status,
      isStale: false,
    }),
  }
})

import { CommandView } from '@/components/CommandView'

type Route =
  | { match: RegExp; body: unknown }
  | { match: RegExp; body: () => unknown }

/** Minimal routed fetch stub: first matching route wins. */
function installFetch(routes: Route[]) {
  return vi.fn(async (url: string) => {
    const hit = routes.find((r) => r.match.test(url))
    const body = !hit ? { error: 'unmocked' } : typeof hit.body === 'function' ? hit.body() : hit.body
    return {
      ok: true,
      status: 200,
      json: async () => body,
    }
  })
}

const SETTINGS = {
  enabled: true,
  cloud_prompt_rate: '1.5',
  cloud_gen_rate: '6.0',
  electricity_rate: '0.12',
  utc_offset: '-4',
}

const SUMMARY = {
  delta_prompt_tokens: 2_000_000,
  delta_gen_tokens: 1_000_000,
  avg_decode_tps: 45.5,
  avg_prompt_tps: 120.2,
  peak_active_requests: 3,
  peak_queued_requests: 1,
  total_requests: 50,
  power_kwh: 0.0123,
  total_seconds: 3600,
  source_table: 'raw',
}

const TIMESERIES = { points: [{ timestamp_ms: 1, value: 250.5 }] }

function baseRoutes(overrides: Record<string, unknown> = {}): Route[] {
  return [
    { match: /\/api\/meta$/, body: { title: 'DGX Spark', peer_url: null, engine_key: 'http://localhost:8000', model: 'glm-4.7' } },
    { match: /\/api\/history\/settings$/, body: SETTINGS },
    { match: /\/api\/history\/summary/, body: SUMMARY },
    { match: /\/api\/history\/timeseries/, body: TIMESERIES },
    ...Object.entries(overrides).map(([key, body]) => ({
      match: new RegExp(key),
      body,
    })),
  ]
}

beforeEach(() => {
  mockMetricsRef.current = null
  mockConn.status = 'connected'
})
afterEach(() => {
  vi.unstubAllGlobals()
})

describe('CommandView — peer unset (default deployment)', () => {
  it('renders the local card live plus a dimmed "not configured" peer card', async () => {
    const fetchMock = installFetch(baseRoutes())
    vi.stubGlobal('fetch', fetchMock)
    render(
      <CommandView
        localTitle="DGX Spark"
        metrics={null}
        connectionStatus="connected"
        isStale={false}
      />,
    )
    // Local card title comes from /api/meta (meta-driven, not hardcoded).
    await waitFor(() => {
      expect(screen.getAllByTestId('cluster-title')[0]).toHaveTextContent('DGX Spark')
    })
    // Peer card: dimmed placeholder naming the env var.
    expect(screen.getAllByTestId('cluster-title')[1]).toBeDefined()
    expect(screen.getByText(/Peer not configured \(SPARK_DASHBOARD_PEER_URL\)/)).toBeInTheDocument()
  })

  it('shows the grand total (local-only) in the savings band', async () => {
    vi.stubGlobal('fetch', installFetch(baseRoutes()))
    render(
      <CommandView
        localTitle="DGX Spark"
        metrics={null}
        connectionStatus="connected"
        isStale={false}
      />,
    )
    // 2M prompt * 1.5 + 1M gen * 6 - 0.0123 kWh * 0.12 = 8.998524
    await waitFor(() => {
      expect(screen.getByTestId('grand-total')).toHaveTextContent('$8.9985')
    })
  })

  it('queries the local summary with the WS-derived engine key', async () => {
    const fetchMock = installFetch(baseRoutes())
    vi.stubGlobal('fetch', fetchMock)
    render(
      <CommandView
        localTitle="X"
        metrics={null}
        connectionStatus="connected"
        isStale={false}
      />,
    )
    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([u]) => String(u).includes('engine=http%3A%2F%2Flocalhost%3A8000'),
        ),
      ).toBe(true)
    })
  })
})

describe('CommandView — peer configured', () => {
  const PEER = 'http://192.168.10.188:3000'
  function peerRoutes(): Route[] {
    return [
      // PEER routes first: the local /api/meta regex would otherwise
      // shadow the peer's (both URLs end in /api/meta).
      { match: new RegExp(`^${PEER}/api/meta$`), body: { title: 'RTX PRO Cluster', peer_url: null, engine_key: 'http://localhost:8000', model: 'qwen3.6' } },
      { match: new RegExp(`^${PEER}/api/history/settings$`), body: { ...SETTINGS, electricity_rate: '0.30' } },
      { match: new RegExp(`^${PEER}/api/history/summary`), body: SUMMARY },
      { match: new RegExp(`^${PEER}/api/history/timeseries`), body: TIMESERIES },
      // Local meta points at the peer; peer meta brands itself differently.
      { match: /\/api\/meta$/, body: { title: 'DGX Spark', peer_url: PEER, engine_key: 'http://localhost:8000', model: 'glm-4.7' } },
      { match: /\/api\/history\/settings$/, body: SETTINGS },
      { match: /\/api\/history\/summary/, body: SUMMARY },
      { match: /\/api\/history\/timeseries/, body: TIMESERIES },
    ]
  }

  it('uses the peer meta title + peer settings for the peer savings row', async () => {
    const fetchMock = installFetch(peerRoutes())
    vi.stubGlobal('fetch', fetchMock)
    render(
      <CommandView
        localTitle="DGX Spark"
        metrics={null}
        connectionStatus="connected"
        isStale={false}
      />,
    )
    await waitFor(() => {
      // Peer card title AND its savings-band row are both meta-driven.
      expect(screen.getAllByText('RTX PRO Cluster').length).toBeGreaterThanOrEqual(1)
    })
    // Peer net: same summary but 0.30/kWh electricity -> 9 - 0.00369.
    expect(screen.getByText('$8.9963')).toBeInTheDocument()
    // Grand total = local 8.998524 + peer 8.99631.
    expect(screen.getByTestId('grand-total')).toHaveTextContent('$17.9948')
    // Peer fetches went directly to the peer URL (no backend proxy).
    expect(
      fetchMock.mock.calls.some(([u]) => String(u).startsWith(`${PEER}/api/history/summary`)),
    ).toBe(true)
  })

  it('uses the peer meta engine_key for peer summary queries', async () => {
    const fetchMock = installFetch(peerRoutes())
    vi.stubGlobal('fetch', fetchMock)
    render(
      <CommandView
        localTitle="X"
        metrics={null}
        connectionStatus="connected"
        isStale={false}
      />,
    )
    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([u]) =>
            String(u).startsWith(`${PEER}/api/history/summary`) &&
            String(u).includes('engine=http%3A%2F%2Flocalhost%3A8000'),
        ),
      ).toBe(true)
    })
  })
})
