import { describe, expect, it, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { NodeOverview } from '../components/NodeOverview'
import type { NodeInfo } from '../hooks/useNodes'
import type { MetricsSnapshot } from '../types/metrics'

const GIB = 1_073_741_824

function makeSnapshot(overrides: Partial<MetricsSnapshot> = {}): MetricsSnapshot {
  return {
    timestamp_ms: 1000,
    gpu: {
      index: 0,
      name: 'NVIDIA Test',
      utilization_percent: 15,
      memory_total_bytes: 48 * GIB,
      memory_used_bytes: 24 * GIB,
      temperature_celsius: 38,
      power_watts: 42,
      power_limit_watts: 300,
      clock_graphics_mhz: 1800,
      clock_sm_mhz: 1800,
      clock_memory_mhz: 9000,
      fan_speed_percent: 30,
    },
    cpu: { name: 'CPU', aggregate_percent: 12, per_core: [] },
    memory: {
      total_bytes: 128 * GIB,
      display_total_bytes: 128 * GIB,
      used_bytes: 64 * GIB,
      available_bytes: 64 * GIB,
      cached_bytes: 8 * GIB,
      gpu_estimated_bytes: null,
      gpu_memory_total_bytes: null,
      gpu_memory_used_bytes: null,
      is_unified: false,
    },
    disk: { name: 'disk', read_bytes_per_sec: 1, write_bytes_per_sec: 2 },
    network: { name: 'net', rx_bytes_per_sec: 3, tx_bytes_per_sec: 4 },
    engines: [],
    gpu_events: [],
    ...overrides,
  }
}

function makeNode(hostname: string, overrides: Partial<NodeInfo> = {}): NodeInfo {
  return {
    hostname,
    url: `http://${hostname}`,
    online: true,
    last_seen_ms: Date.now(),
    snapshot: makeSnapshot(),
    ...overrides,
  }
}

describe('NodeOverview', () => {
  it('renders one card per node', () => {
    const nodes: NodeInfo[] = [
      makeNode('DGX1'),
      makeNode('DGX2'),
      makeNode('DGX3'),
      makeNode('DGX4'),
    ]
    render(<NodeOverview nodes={nodes} onSelect={vi.fn()} />)

    expect(screen.getByRole('button', { name: /DGX1/ })).toBeTruthy()
    expect(screen.getByRole('button', { name: /DGX2/ })).toBeTruthy()
    expect(screen.getByRole('button', { name: /DGX3/ })).toBeTruthy()
    expect(screen.getByRole('button', { name: /DGX4/ })).toBeTruthy()
  })

  it('calls onSelect with the node index when a card is clicked', () => {
    const onSelect = vi.fn()
    const nodes: NodeInfo[] = [makeNode('DGX1'), makeNode('DGX2')]
    render(<NodeOverview nodes={nodes} onSelect={onSelect} />)

    fireEvent.click(screen.getByRole('button', { name: /DGX2/ }))
    expect(onSelect).toHaveBeenCalledWith(1)

    fireEvent.click(screen.getByRole('button', { name: /DGX1/ }))
    expect(onSelect).toHaveBeenCalledWith(0)
  })

  it('reduces opacity for offline nodes', () => {
    const nodes: NodeInfo[] = [
      makeNode('DGX1', { online: true }),
      makeNode('DGX2', { online: false }),
    ]
    render(<NodeOverview nodes={nodes} onSelect={vi.fn()} />)

    const offlineCard = screen.getByRole('button', { name: /DGX2/ })
    const onlineCard = screen.getByRole('button', { name: /DGX1/ })

    expect(offlineCard.className).toContain('opacity-50')
    expect(onlineCard.className).not.toContain('opacity-50')
  })

  it('shows the online/offline status dot', () => {
    const nodes: NodeInfo[] = [
      makeNode('DGX1', { online: true }),
      makeNode('DGX2', { online: false }),
    ]
    render(<NodeOverview nodes={nodes} onSelect={vi.fn()} />)

    expect(screen.getByLabelText('online')).toBeTruthy()
    expect(screen.getByLabelText('offline')).toBeTruthy()
  })

  it('handles nodes with null snapshots gracefully', () => {
    const nodes: NodeInfo[] = [
      makeNode('DGX1', { snapshot: null }),
    ]
    render(<NodeOverview nodes={nodes} onSelect={vi.fn()} />)

    // Card still renders
    expect(screen.getByRole('button', { name: /DGX1/ })).toBeTruthy()
  })
})
