import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { ChartWithTimeScale } from '@/components/charts/ChartWithTimeScale'

// --- Mock fetch ---
type FetchHandler = (url: string) => Promise<unknown>
let fetchHandler: FetchHandler = async () => ({ points: [] })

beforeEach(() => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string) => {
      const body = await fetchHandler(url)
      return {
        ok: true,
        status: 200,
        json: async () => body,
      }
    }),
  )
})

afterEach(() => {
  vi.unstubAllGlobals()
  fetchHandler = async () => ({ points: [] })
})

const sampleData = [
  { timestamp: 1700000000000, value: 50 },
  { timestamp: 1700000001000, value: 55 },
  { timestamp: 1700000002000, value: 60 },
  { timestamp: 1700000003000, value: 58 },
  { timestamp: 1700000004000, value: 62 },
  { timestamp: 1700000005000, value: 65 },
  { timestamp: 1700000006000, value: 63 },
  { timestamp: 1700000007000, value: 67 },
  { timestamp: 1700000008000, value: 70 },
  { timestamp: 1700000009000, value: 68 },
  { timestamp: 1700000010000, value: 72 },
  { timestamp: 1700000011000, value: 75 },
  { timestamp: 1700000012000, value: 73 },
  { timestamp: 1700000013000, value: 78 },
  { timestamp: 1700000014000, value: 80 },
  { timestamp: 1700000015000, value: 82 },
  { timestamp: 1700000016000, value: 79 },
  { timestamp: 1700000017000, value: 85 },
  { timestamp: 1700000018000, value: 88 },
  { timestamp: 1700000019000, value: 90 },
  { timestamp: 1700000020000, value: 87 },
  { timestamp: 1700000021000, value: 92 },
  { timestamp: 1700000022000, value: 95 },
  { timestamp: 1700000023000, value: 93 },
  { timestamp: 1700000024000, value: 98 },
  { timestamp: 1700000025000, value: 100 },
  { timestamp: 1700000026000, value: 97 },
  { timestamp: 1700000027000, value: 94 },
  { timestamp: 1700000028000, value: 91 },
  { timestamp: 1700000029000, value: 89 },
  { timestamp: 1700000030000, value: 86 },
  { timestamp: 1700000031000, value: 83 },
  { timestamp: 1700000032000, value: 81 },
  { timestamp: 1700000033000, value: 78 },
  { timestamp: 1700000034000, value: 76 },
  { timestamp: 1700000035000, value: 74 },
  { timestamp: 1700000036000, value: 71 },
  { timestamp: 1700000037000, value: 69 },
  { timestamp: 1700000038000, value: 67 },
  { timestamp: 1700000039000, value: 65 },
  { timestamp: 1700000040000, value: 63 },
  { timestamp: 1700000041000, value: 61 },
  { timestamp: 1700000042000, value: 59 },
  { timestamp: 1700000043000, value: 57 },
  { timestamp: 1700000044000, value: 55 },
  { timestamp: 1700000045000, value: 53 },
  { timestamp: 1700000046000, value: 51 },
  { timestamp: 1700000047000, value: 49 },
  { timestamp: 1700000048000, value: 47 },
  { timestamp: 1700000049000, value: 45 },
  { timestamp: 1700000050000, value: 43 },
  { timestamp: 1700000051000, value: 41 },
  { timestamp: 1700000052000, value: 39 },
  { timestamp: 1700000053000, value: 37 },
  { timestamp: 1700000054000, value: 35 },
  { timestamp: 1700000055000, value: 33 },
  { timestamp: 1700000056000, value: 31 },
  { timestamp: 1700000057000, value: 29 },
  { timestamp: 1700000058000, value: 27 },
  { timestamp: 1700000059000, value: 25 },
  { timestamp: 1700000060000, value: 23 },
  { timestamp: 1700000061000, value: 21 },
  { timestamp: 1700000062000, value: 19 },
  { timestamp: 1700000063000, value: 17 },
  { timestamp: 1700000064000, value: 15 },
  { timestamp: 1700000065000, value: 13 },
  { timestamp: 1700000066000, value: 11 },
  { timestamp: 1700000067000, value: 9 },
  { timestamp: 1700000068000, value: 7 },
  { timestamp: 1700000069000, value: 5 },
]

const sampleSeries = [
  { data: sampleData, label: 'Live', color: '#76B900' },
  { data: sampleData.slice(0, 5), label: 'Avg', color: '#3b82f6' },
  { data: sampleData.slice(0, 3), label: 'Per-req', color: '#a855f7' },
]

describe('ChartWithTimeScale', () => {
  it('renders TimeSeriesChart with buffer data in 5m mode (default)', () => {
    const { container } = render(
      <ChartWithTimeScale
        bufferData={sampleData}
        engineEndpoint="http://localhost:8000"
        historyMetrics={['gpu_util']}
        unit="%"
      />,
    )
    const chart = container.querySelector('[data-slot="chart"]')
    expect(chart).not.toBeNull()
  })

  it('shows TimeScaleButton with default 5m label', () => {
    render(
      <ChartWithTimeScale
        bufferData={sampleData}
        engineEndpoint="http://localhost:8000"
        historyMetrics={['gpu_util']}
        unit="%"
      />,
    )
    expect(screen.getByText('5m')).toBeInTheDocument()
  })

  it('hides TimeScaleButton when engineEndpoint is null', () => {
    render(
      <ChartWithTimeScale
        bufferData={sampleData}
        engineEndpoint={null}
        historyMetrics={['gpu_util']}
        unit="%"
      />,
    )
    expect(screen.queryByText('5m')).not.toBeInTheDocument()
  })

  it('hides TimeScaleButton when historyMetrics is undefined', () => {
    render(
      <ChartWithTimeScale
        bufferData={sampleData}
        engineEndpoint="http://localhost:8000"
        unit="%"
      />,
    )
    expect(screen.queryByText('5m')).not.toBeInTheDocument()
  })

  it('cycles through scales on button click', () => {
    render(
      <ChartWithTimeScale
        bufferData={sampleData}
        engineEndpoint="http://localhost:8000"
        historyMetrics={['gpu_util']}
        unit="%"
      />,
    )
    // Default: 5m
    expect(screen.getByText('5m')).toBeInTheDocument()
    // Click → 1h
    fireEvent.click(screen.getByRole('button'))
    // 1h triggers fetch, so we need to look for 1h label
    expect(screen.getByText('1h')).toBeInTheDocument()
  })

  it('renders multi-series chart in buffer mode', () => {
    const { container } = render(
      <ChartWithTimeScale
        bufferSeries={sampleSeries}
        engineEndpoint="http://localhost:8000"
        historyMetrics={['prompt_tps', undefined, undefined]}
        unit="tok/s"
      />,
    )
    const chart = container.querySelector('[data-slot="chart"]')
    expect(chart).not.toBeNull()
    expect(screen.getByText('Live')).toBeInTheDocument()
    expect(screen.getByText('Avg')).toBeInTheDocument()
  })

  it('fetches from API when switching to 1h', async () => {
    fetchHandler = async (url: string) => {
      expect(url).toContain('/api/history/timeseries')
      expect(url).toContain('metric=gpu_util')
      return {
        points: [
          { timestamp_ms: 1700000000000, value: 50 },
          { timestamp_ms: 1700000001000, value: 55 },
        ],
      }
    }

    const { container } = render(
      <ChartWithTimeScale
        bufferData={sampleData}
        engineEndpoint="http://localhost:8000"
        historyMetrics={['gpu_util']}
        unit="%"
      />,
    )

    // Switch to 1h
    fireEvent.click(screen.getByRole('button'))

    await waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(
        expect.stringContaining('/api/history/timeseries'),
      )
    })

    // After fetch, chart should render with historical data
    await waitFor(() => {
      const chart = container.querySelector('[data-slot="chart"]')
      expect(chart).not.toBeNull()
    })
  })

  it('shows "No historical data" when API returns empty', async () => {
    fetchHandler = async () => ({ points: [] })

    render(
      <ChartWithTimeScale
        bufferData={sampleData}
        engineEndpoint="http://localhost:8000"
        historyMetrics={['gpu_util']}
        unit="%"
      />,
    )

    // Switch to 1h
    fireEvent.click(screen.getByRole('button'))

    await waitFor(() => {
      expect(screen.getByText('No historical data for this range')).toBeInTheDocument()
    })
  })

  it('shows derived series in 1h mode when source metrics are fetched', async () => {
    // Throughput chart: Live (direct), Avg (derived from prompt_tps), Per-req (derived from prompt_tps + active_requests)
    fetchHandler = async (url: string) => {
      if (url.includes('metric=prompt_tps')) {
        return {
          points: [
            { timestamp_ms: 1700000000000, value: 100 },
            { timestamp_ms: 1700000001000, value: 110 },
          ],
        }
      }
      if (url.includes('metric=active_requests')) {
        return {
          points: [
            { timestamp_ms: 1700000000000, value: 2 },
            { timestamp_ms: 1700000001000, value: 2 },
          ],
        }
      }
      return { points: [] }
    }

    render(
      <ChartWithTimeScale
        bufferSeries={sampleSeries}
        engineEndpoint="http://localhost:8000"
        historyMetrics={[
          'prompt_tps',
          { sourceMetrics: ['prompt_tps'], compute: (src) => {
            const d = src['prompt_tps']
            if (!d || d.length === 0) return []
            const avg = d.reduce((a, p) => a + p.value, 0) / d.length
            return d.map((p) => ({ timestamp: p.timestamp, value: avg }))
          }},
          { sourceMetrics: ['prompt_tps', 'active_requests'], compute: (src) => {
            const tps = src['prompt_tps']
            const reqs = src['active_requests']
            if (!tps || tps.length === 0) return []
            if (!reqs || reqs.length === 0) return tps.map((p) => ({ timestamp: p.timestamp, value: 0 }))
            return tps.map((p, i) => ({ timestamp: p.timestamp, value: reqs[Math.min(i, reqs.length - 1)].value > 0 ? p.value / reqs[Math.min(i, reqs.length - 1)].value : 0 }))
          }},
        ]}
        unit="tok/s"
      />,
    )

    // Switch to 1h
    fireEvent.click(screen.getByRole('button'))

    // Wait for fetch — all 3 metrics should be fetched
    await waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(
        expect.stringContaining('metric=prompt_tps'),
      )
    })
    await waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(
        expect.stringContaining('metric=active_requests'),
      )
    })
  })

  it('series without historyMetric are hidden in 1h/24h mode', async () => {
    // Only the first series has a history metric; others are undefined → hidden
    fetchHandler = async (url: string) => {
      if (url.includes('metric=prompt_tps')) {
        return {
          points: [
            { timestamp_ms: 1700000000000, value: 100 },
            { timestamp_ms: 1700000001000, value: 110 },
          ],
        }
      }
      return { points: [] }
    }

    render(
      <ChartWithTimeScale
        bufferSeries={sampleSeries}
        engineEndpoint="http://localhost:8000"
        historyMetrics={['prompt_tps', undefined, undefined]}
        unit="tok/s"
      />,
    )

    // Switch to 1h
    fireEvent.click(screen.getByRole('button'))

    await waitFor(() => {
      expect(fetch).toHaveBeenCalled()
    })

    // Verify no crash and scale switched
    await waitFor(() => {
      expect(screen.queryByText('5m')).not.toBeInTheDocument()
    })
  })
})
