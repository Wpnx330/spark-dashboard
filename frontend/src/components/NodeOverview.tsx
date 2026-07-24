import { formatBytes, formatGiB } from '@/lib/format'
import type { NodeInfo } from '@/hooks/useNodes'

interface NodeOverviewProps {
  nodes: NodeInfo[]
  onSelect: (index: number) => void
}

/** Compact summary card for a single node in the multi-node grid. */
function NodeCard({ node, index, onSelect }: { node: NodeInfo; index: number; onSelect: (i: number) => void }) {
  const snap = node.snapshot
  const gpu = snap?.gpu
  const mem = snap?.memory

  const memUsedPercent = mem && (mem.display_total_bytes ?? mem.total_bytes) > 0
    ? Math.min(100, (mem.used_bytes / (mem.display_total_bytes ?? mem.total_bytes)) * 100)
    : 0

  return (
    <button
      type="button"
      onClick={() => onSelect(index)}
      aria-label={`${node.hostname} details`}
      className={`bg-[#111115] rounded-md sm:rounded-lg border border-white/[0.04] px-2 py-1.5 lg:px-2.5 lg:py-2 text-left cursor-pointer transition-colors duration-150 hover:border-[#76B900]/30 flex flex-col gap-1 min-w-0 overflow-hidden ${
        node.online ? '' : 'opacity-50'
      }`}
    >
      {/* Header: hostname + status dot */}
      <div className="flex items-center gap-1.5 min-w-0">
        <span
          className={`inline-block w-1.5 h-1.5 rounded-full shrink-0 ${
            node.online ? 'bg-[#76B900]' : 'bg-red-500'
          }`}
          aria-label={node.online ? 'online' : 'offline'}
        />
        <span className="text-[11px] lg:text-xs font-semibold text-zinc-200 truncate">
          {node.hostname}
        </span>
      </div>

      {/* GPU metrics row */}
      <div className="grid grid-cols-3 gap-1 text-[10px] lg:text-[11px] font-mono tabular-nums text-zinc-300">
        <span title="GPU Power">{gpu?.power_watts !== null && gpu?.power_watts !== undefined ? `${Math.round(gpu.power_watts)}W` : '--'}</span>
        <span title="GPU Temp">{gpu?.temperature_celsius !== null && gpu?.temperature_celsius !== undefined ? `${gpu.temperature_celsius}°C` : '--'}</span>
        <span title="GPU Util">{gpu?.utilization_percent !== null && gpu?.utilization_percent !== undefined ? `${gpu.utilization_percent}%` : '--'}</span>
      </div>

      {/* CPU + Memory row */}
      <div className="flex items-center justify-between gap-2 text-[10px] lg:text-[11px] font-mono tabular-nums text-zinc-400 min-w-0">
        <span title="CPU">{snap ? `${Math.round(snap.cpu.aggregate_percent)}%` : '--'}</span>
        <span className="truncate" title="Memory">
          {mem ? `${formatBytes(mem.used_bytes)} / ${formatGiB(mem.display_total_bytes ?? mem.total_bytes)}` : '--'}
        </span>
      </div>

      {/* Memory usage bar */}
      <div className="h-1 rounded-full bg-white/[0.06] overflow-hidden">
        <div
          className="h-full bg-[#76B900] rounded-full transition-[width] duration-300"
          style={{ width: `${memUsedPercent}%` }}
        />
      </div>
    </button>
  )
}

/** Grid of node summary cards shown in the multi-node hardware overview. */
export function NodeOverview({ nodes, onSelect }: NodeOverviewProps) {
  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-1 lg:gap-1.5">
      {nodes.map((node, i) => (
        <NodeCard key={node.hostname} node={node} index={i} onSelect={onSelect} />
      ))}
    </div>
  )
}
