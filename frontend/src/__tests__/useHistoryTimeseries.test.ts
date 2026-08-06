import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { useHistoryTimeseries } from '@/hooks/useHistoryTimeseries'

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

describe('useHistoryTimeseries', () => {
  it('returns null data for 1m scale (buffer mode)', () => {
    const { result } = renderHook(() =>
      useHistoryTimeseries('http://localhost:8000', 'gpu_util', '1m'),
    )
    expect(result.current.data).toBeNull()
    expect(result.current.loading).toBe(false)
    expect(result.current.error).toBeNull()
  })

  it('returns null data for 5m scale (buffer mode)', () => {
    const { result } = renderHook(() =>
      useHistoryTimeseries('http://localhost:8000', 'gpu_util', '5m'),
    )
    expect(result.current.data).toBeNull()
    expect(result.current.loading).toBe(false)
    expect(result.current.error).toBeNull()
  })

  it('returns null data when engineEndpoint is null', () => {
    const { result } = renderHook(() =>
      useHistoryTimeseries(null, 'gpu_util', '1h'),
    )
    expect(result.current.data).toBeNull()
    expect(result.current.loading).toBe(false)
  })

  it('fetches from API for 1h scale', async () => {
    fetchHandler = async (url: string) => {
      expect(url).toContain('/api/history/timeseries')
      expect(url).toContain('engine=http%3A%2F%2Flocalhost%3A8000')
      expect(url).toContain('metric=gpu_util')
      expect(url).toContain('since_ms=')
      expect(url).toContain('until_ms=')
      return {
        points: [
          { timestamp_ms: 1700000000000, value: 50 },
          { timestamp_ms: 1700000001000, value: 55 },
        ],
      }
    }

    const { result } = renderHook(() =>
      useHistoryTimeseries('http://localhost:8000', 'gpu_util', '1h'),
    )

    // Initially loading
    expect(result.current.loading).toBe(true)

    await waitFor(() => {
      expect(result.current.loading).toBe(false)
    })

    expect(result.current.data).not.toBeNull()
    expect(result.current.data).toHaveLength(2)
    expect(result.current.data![0]).toEqual({ timestamp_ms: 1700000000000, value: 50 })
    expect(result.current.data![1]).toEqual({ timestamp_ms: 1700000001000, value: 55 })
    expect(result.current.error).toBeNull()
    // Verify fetch was called
    expect(fetch).toHaveBeenCalledTimes(1)
  })

  it('fetches from API for 24h scale', async () => {
    fetchHandler = async () => ({
      points: [{ timestamp_ms: 1700000000000, value: 42 }],
    })

    const { result } = renderHook(() =>
      useHistoryTimeseries('http://localhost:8000', 'gpu_temp', '24h'),
    )

    await waitFor(() => {
      expect(result.current.loading).toBe(false)
    })

    expect(result.current.data).not.toBeNull()
    expect(result.current.data).toHaveLength(1)
    expect(result.current.data![0]).toEqual({ timestamp_ms: 1700000000000, value: 42 })
  })

  it('handles API error', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: false,
        status: 500,
        json: async () => ({ error: 'Internal error' }),
      })),
    )

    const { result } = renderHook(() =>
      useHistoryTimeseries('http://localhost:8000', 'gpu_util', '1h'),
    )

    await waitFor(() => {
      expect(result.current.loading).toBe(false)
    })

    expect(result.current.error).not.toBeNull()
    expect(result.current.data).toBeNull()
  })

  it('handles empty points array', async () => {
    fetchHandler = async () => ({ points: [] })

    const { result } = renderHook(() =>
      useHistoryTimeseries('http://localhost:8000', 'mem_used_pct', '1h'),
    )

    await waitFor(() => {
      expect(result.current.loading).toBe(false)
    })

    expect(result.current.data).toEqual([])
    expect(result.current.error).toBeNull()
  })
})
