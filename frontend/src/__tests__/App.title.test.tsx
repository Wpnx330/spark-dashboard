import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import App from '../App'
import { DEFAULT_DASHBOARD_TITLE } from '../hooks/useMeta'

// Minimal inert WebSocket: useMetrics and LogViewer only construct it in
// these tests — no snapshot/log traffic is exchanged.
class StubWebSocket {
  close() {}
  send() {}
}
;(globalThis as unknown as { WebSocket: unknown }).WebSocket = StubWebSocket

type MetaPayload = { title: string }

// --- Mock fetch, dispatched by URL path like useHistoryTimeseries.test.ts ---
let metaHandler: () => Promise<{ ok: boolean; status: number; json: () => Promise<unknown> }>
const fetchMock = vi.fn(async (url: string) => {
  if (String(url).includes('/api/meta')) return metaHandler()
  // /api/nodes and any other call: empty node list.
  return { ok: true, status: 200, json: async () => [] }
})

beforeEach(() => {
  metaHandler = async () => ({
    ok: true,
    status: 200,
    json: async () => ({ title: DEFAULT_DASHBOARD_TITLE }),
  })
  vi.stubGlobal('fetch', fetchMock)
})

afterEach(() => {
  vi.unstubAllGlobals()
  document.title = ''
})

describe('App dashboard title', () => {
  it('renders the title fetched from /api/meta in the header and document.title', async () => {
    metaHandler = async () => ({
      ok: true,
      status: 200,
      json: async () => ({ title: 'RTX PRO Cluster' } as MetaPayload),
    })

    render(<App />)

    const heading = await screen.findByRole('heading', { level: 1 })
    await waitFor(() => expect(heading).toHaveTextContent('RTX PRO Cluster'))
    expect(document.title).toBe('RTX PRO Cluster')
    expect(fetchMock).toHaveBeenCalledWith('/api/meta')
  })

  it('falls back to the default title when /api/meta is unreachable', async () => {
    metaHandler = async () => {
      throw new Error('connection refused')
    }

    render(<App />)

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith('/api/meta'))
    const heading = screen.getByRole('heading', { level: 1 })
    expect(heading).toHaveTextContent(DEFAULT_DASHBOARD_TITLE)
    // A failed fetch must not clobber the static index.html <title>.
    expect(document.title).toBe('')
  })

  it('falls back to the default title on an HTTP error response', async () => {
    metaHandler = async () => ({
      ok: false,
      status: 500,
      json: async () => ({ title: 'should be ignored' }),
    })

    render(<App />)

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith('/api/meta'))
    const heading = screen.getByRole('heading', { level: 1 })
    expect(heading).toHaveTextContent(DEFAULT_DASHBOARD_TITLE)
  })

  it('keeps the default title when the backend sends a blank title', async () => {
    metaHandler = async () => ({
      ok: true,
      status: 200,
      json: async () => ({ title: '   ' }),
    })

    render(<App />)

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith('/api/meta'))
    const heading = screen.getByRole('heading', { level: 1 })
    expect(heading).toHaveTextContent(DEFAULT_DASHBOARD_TITLE)
  })
})
