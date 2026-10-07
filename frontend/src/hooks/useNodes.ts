import { useEffect, useRef, useState } from 'react'
import type { MetricsSnapshot } from '../types/metrics'

export interface NodeInfo {
  hostname: string
  url: string
  online: boolean
  last_seen_ms: number
  snapshot: MetricsSnapshot | null
}

/**
 * Polls `GET /api/nodes` every second and returns the latest list of nodes.
 *
 * Keeps the last known good data on fetch/parse errors so the UI never blanks
 * out during a transient blip — mirroring the resilience of {@link useMetrics}.
 *
 * The API returns a bare JSON array of {@link NodeInfo} objects.
 */
export function useNodes() {
  const [nodes, setNodes] = useState<NodeInfo[]>([])
  const [loading, setLoading] = useState(true)
  const lastGoodRef = useRef<NodeInfo[]>([])

  useEffect(() => {
    let cancelled = false

    async function poll() {
      try {
        const res = await fetch('/api/nodes')
        if (!res.ok) throw new Error(`HTTP ${res.status}`)
        const data = await res.json()
        if (cancelled) return
        const next = Array.isArray(data) ? data : []
        lastGoodRef.current = next
        setNodes(next)
        setLoading(false)
      } catch {
        // Keep last known data on error; only flip loading off once we've tried.
        if (!cancelled) setLoading(false)
      }
    }

    poll()
    const id = setInterval(poll, 1000)
    return () => {
      cancelled = true
      clearInterval(id)
    }
  }, [])

  return { nodes, loading }
}
