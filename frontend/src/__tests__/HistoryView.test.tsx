import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import type { MetricsSnapshot } from '@/types/metrics'

// --- Mock useMetrics so HistoryView doesn't open a real WebSocket ---
const mockMetricsRef = { current: null as MetricsSnapshot | null }

vi.mock('@/hooks/useMetrics', () => ({
  useMetrics: () => ({
    metrics: mockMetricsRef.current,
    connectionStatus: 'connected' as const,
    isStale: false,
  }),
}))

// --- Mock fetch for /api/history/* calls ---
type FetchHandler = (url: string, init?: RequestInit) => Promise<unknown>
let fetchHandler: FetchHandler = async () => ({ error: 'no data' })

beforeEach(() => {
  mockMetricsRef.current = null
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init?: RequestInit) => {
      const body = await fetchHandler(url, init)
      return {
        ok: true,
        status: 200,
        json: async () => body,
        text: async () => JSON.stringify(body),
      }
    }),
  )
})

afterEach(() => {
  vi.unstubAllGlobals()
  fetchHandler = async () => ({ error: 'no data' })
})

import { HistoryView } from '@/components/HistoryView'

/** A summary payload shaped like the backend's HistorySummary. */
function summaryPayload(overrides: Partial<Record<string, number | string>> = {}) {
  return {
    delta_prompt_tokens: 1000,
    delta_gen_tokens: 2000,
    avg_decode_tps: 45.5,
    avg_prompt_tps: 120.2,
    peak_active_requests: 3,
    peak_queued_requests: 1,
    total_requests: 50,
    power_kwh: 0.0123,
    total_seconds: 3600,
    source_table: 'raw',
    ...overrides,
  }
}

/** Default settings payload matching the monolithic server response. */
function settingsPayload(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    enabled: false,
    cloud_prompt_rate: '',
    cloud_gen_rate: '',
    electricity_rate: '',
    utc_offset: '-4',
    ...overrides,
  }
}

/** Route /api/history/* to the right handler based on the URL. */
function installFetch(routes: {
  settings?: () => unknown
  summary?: (url: string) => unknown
  toggle?: () => unknown
  pricing?: () => unknown
}) {
  fetchHandler = async (url, init) => {
    if (url.startsWith('/api/history/lookup-pricing')) {
      return routes.pricing?.() ?? { error: 'not found' }
    }
    if (url.startsWith('/api/history/settings')) {
      if (init?.method === 'POST') return { ok: true }
      return routes.settings?.() ?? settingsPayload()
    }
    if (url.startsWith('/api/history/summary')) {
      return routes.summary?.(url) ?? summaryPayload()
    }
    if (url.startsWith('/api/history/toggle')) {
      return routes.toggle?.() ?? { ok: true }
    }
    return { error: 'not found' }
  }
}

describe('HistoryView', () => {
  beforeEach(() => {
    installFetch({})
  })

  it('renders without crashing', async () => {
    render(<HistoryView />)
    // The header label is always present.
    expect(screen.getByText('Historical')).toBeInTheDocument()
  })

  it('renders the preset buttons and the Custom toggle', async () => {
    render(<HistoryView />)
    expect(screen.getByText('Today')).toBeInTheDocument()
    expect(screen.getByText('This Week')).toBeInTheDocument()
    expect(screen.getByText('This Month')).toBeInTheDocument()
    expect(screen.getByText('Last Month')).toBeInTheDocument()
    expect(screen.getByText('This Year')).toBeInTheDocument()
    expect(screen.getByText('All Time')).toBeInTheDocument()
    expect(screen.getByText('Custom')).toBeInTheDocument()
  })

  it('switching to Custom reveals the date-range inputs and Go button', async () => {
    render(<HistoryView />)
    expect(screen.queryByText('Go')).not.toBeInTheDocument()
    fireEvent.click(screen.getByText('Custom'))
    expect(screen.getByText('Go')).toBeInTheDocument()
    expect(screen.getAllByDisplayValue('')).toHaveLength(2)
  })

  it('shows the loading state while fetching summary', async () => {
    // Make the summary fetch hang so loading stays true.
    let resolveSummary: (v: unknown) => void = () => {}
    installFetch({
      summary: () =>
        new Promise((resolve) => {
          resolveSummary = resolve
        }),
    })
    render(<HistoryView />)
    await waitFor(() => {
      expect(screen.getByText('Loading history...')).toBeInTheDocument()
    })
    // Release the hanging promise so the test can clean up.
    resolveSummary(summaryPayload())
  })

  it('shows the "no data" message when the backend has no history', async () => {
    installFetch({ summary: () => ({ error: 'no data' }) })
    render(<HistoryView />)
    await waitFor(() => {
      expect(
        screen.getByText(
          'No historical data yet. Enable logging in Settings and wait a moment.',
        ),
      ).toBeInTheDocument()
    })
  })

  it('shows an error message on fetch failure', async () => {
    // Force a non-"no data" error by throwing inside the handler.
    installFetch({
      summary: () => {
        throw new Error('boom')
      },
    })
    render(<HistoryView />)
    await waitFor(() => {
      expect(screen.getByText('Failed to fetch history')).toBeInTheDocument()
    })
  })

  it('renders summary stats (token throughput + peak load) when data is present', async () => {
    installFetch({
      summary: () => summaryPayload({ delta_prompt_tokens: 4242, total_requests: 7 }),
    })
    render(<HistoryView />)
    await waitFor(() => {
      expect(screen.getByText('4,242')).toBeInTheDocument()
    })
    expect(screen.getByText('7')).toBeInTheDocument()
    // The monolithic component DOES show the Cost Savings card.
    expect(screen.getByText('Cost Savings')).toBeInTheDocument()
    expect(screen.getByText('Cloud Cost Avoided')).toBeInTheDocument()
    expect(screen.getByText('Net Savings')).toBeInTheDocument()
    expect(screen.getByText('Token Throughput')).toBeInTheDocument()
    expect(screen.getByText('Peak Load')).toBeInTheDocument()
  })

  it('opening settings shows the logging toggle, pricing fields, and UTC offset', async () => {
    installFetch({
      settings: () =>
        settingsPayload({
          enabled: false,
          utc_offset: '-4',
          cloud_prompt_rate: '1.50',
          cloud_gen_rate: '2.00',
          electricity_rate: '0.12',
        }),
    })
    render(<HistoryView />)
    fireEvent.click(screen.getByText('⚙ Settings'))
    expect(screen.getByText('History Settings')).toBeInTheDocument()
    expect(screen.getByText('Logging')).toBeInTheDocument()
    expect(screen.getByText('UTC Offset')).toBeInTheDocument()
    // Pricing-related fields ARE present in the monolithic component.
    expect(screen.getByText('$/1M Prompt Tokens')).toBeInTheDocument()
    expect(screen.getByText('$/1M Gen Tokens')).toBeInTheDocument()
    expect(screen.getByText('$/kWh Electricity')).toBeInTheDocument()
    expect(screen.getByText('⟳ Look up pricing')).toBeInTheDocument()
  })

  it('clicking a preset re-fetches the summary with a new range', async () => {
    const seenRanges: string[] = []
    installFetch({
      summary: (url) => {
        seenRanges.push(url)
        return summaryPayload()
      },
    })
    render(<HistoryView />)
    await waitFor(() => expect(seenRanges.length).toBeGreaterThanOrEqual(1))
    const before = seenRanges.length
    fireEvent.click(screen.getByText('All Time'))
    await waitFor(() => expect(seenRanges.length).toBeGreaterThan(before))
    // All Time uses since_ms=0.
    expect(seenRanges.some((u) => u.includes('since_ms=0'))).toBe(true)
  })
})
