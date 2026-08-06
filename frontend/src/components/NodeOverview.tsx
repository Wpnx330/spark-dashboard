import { formatBytes, formatGiB } from '@/lib/format'
import type { NodeInfo } from '@/hooks/useNodes'

interface NodeOverviewProps {
  nodes: NodeInfo[]
  onSelect: (index: number) => void
}

/** Circular gauge for a single metric — scales to fill its container.
 *  Uses relative sizing so it never overflows on narrow mobile cards. */
function MiniGauge({ value, max, unit, label, color }: { value: number; max: number; unit: string; label: string; color: string }) {
  const pct = Math.min(100, (value / max) * 100)
  const radius = 26
  const circumference = 2 * Math.PI * radius
  const offset = circumference - (pct / 100) * circumference

  return (
    <div className="flex flex-col items-center gap-0.5 min-w-0 flex-1">
      <div className="relative w-full aspect-square max-w-[64px] lg:max-w-[72px] mx-auto">
        <svg className="w-full h-full -rotate-90" viewBox="0 0 64 64">
          <circle cx="32" cy="32" r={radius} fill="none" stroke="rgba(255,255,255,0.06)" strokeWidth="5" />
          <circle
            cx="32" cy="32" r={radius} fill="none" stroke={color} strokeWidth="5"
            strokeDasharray={circumference} strokeDashoffset={offset}
            strokeLinecap="round"
            className="transition-[stroke-dashoffset] duration-500"
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span className="text-xs sm:text-sm lg:text-base font-mono font-bold tabular-nums text-zinc-100">
            {Math.round(value)}{unit}
          </span>
        </div>
      </div>
      <span className="text-[9px] lg:text-[10px] text-zinc-500 uppercase tracking-wide">{label}</span>
    </div>
  )
}

/** Large, rich card for a single node in the multi-node grid. */
function NodeCard({ node, index, onSelect }: { node: NodeInfo; index: number; onSelect: (i: number) => void }) {
  const snap = node.snapshot
  const gpu = snap?.gpu
  const mem = snap?.memory
  const cpu = snap?.cpu

  const memTotal = mem ? (mem.display_total_bytes ?? mem.total_bytes) : 0
  const memUsed = mem?.used_bytes ?? 0
  const memUsedPercent = memTotal > 0 ? Math.min(100, (memUsed / memTotal) * 100) : 0

  // Determine gauge colors
  const tempColor = gpu && gpu.temperature_celsius != null && gpu.temperature_celsius >= 80 ? '#ef4444' : gpu && gpu.temperature_celsius != null && gpu.temperature_celsius >= 70 ? '#f59e0b' : '#76B900'
  const utilColor = '#76B900'
  const powerColor = '#76B900'
  const cpuColor = cpu && cpu.aggregate_percent > 80 ? '#f59e0b' : '#3b82f6'

  return (
    <button
      type="button"
      onClick={() => onSelect(index)}
      aria-label={`${node.hostname} details`}
      className={`bg-[#111115] rounded-lg border border-white/[0.04] p-2 sm:p-3 lg:p-4 text-left cursor-pointer transition-all duration-150 hover:border-[#76B900]/30 hover:bg-[#131318] flex flex-col gap-2 sm:gap-3 min-w-0 overflow-hidden h-full ${
        node.online ? '' : 'opacity-50'
      }`}
    >
      {/* Header: hostname + status dot */}
      <div className="flex items-center justify-between gap-2 min-w-0">
        <div className="flex items-center gap-2 min-w-0">
          <span
            className={`inline-block w-2 h-2 rounded-full shrink-0 ${node.online ? 'bg-[#76B900]' : 'bg-red-500'}`}
            aria-label={node.online ? 'online' : 'offline'}
          />
          <span className="text-sm lg:text-base font-semibold text-zinc-100 truncate">
            {node.hostname}
          </span>
        </div>
        {gpu && (
          <span className="text-[10px] lg:text-[11px] text-zinc-500 font-mono shrink-0 hidden sm:block">
            {gpu.name}
          </span>
        )}
      </div>

      {/* Gauges row — GPU util, temp, power */}
      <div className="flex items-center justify-around gap-1 flex-1">
        {gpu && (
          <>
            <MiniGauge value={gpu.utilization_percent ?? 0} max={100} unit="%" label="GPU" color={utilColor} />
            <MiniGauge value={gpu.temperature_celsius ?? 0} max={100} unit="°" label="Temp" color={tempColor} />
            <MiniGauge value={gpu.power_watts ?? 0} max={100} unit="W" label="Power" color={powerColor} />
          </>
        )}
      </div>

      {/* CPU bar */}
      <div className="space-y-1">
        <div className="flex items-center justify-between text-[10px] lg:text-[11px] font-mono tabular-nums">
          <span className="text-zinc-500">CPU</span>
          <span className="text-zinc-300">{cpu ? `${Math.round(cpu.aggregate_percent)}%` : '--'}</span>
        </div>
        <div className="h-1.5 rounded-full bg-white/[0.06] overflow-hidden">
          <div
            className="h-full rounded-full transition-[width] duration-300"
            style={{ width: `${cpu?.aggregate_percent ?? 0}%`, backgroundColor: cpuColor }}
          />
        </div>
      </div>

      {/* Memory bar */}
      <div className="space-y-1">
        <div className="flex items-center justify-between text-[10px] lg:text-[11px] font-mono tabular-nums">
          <span className="text-zinc-500">Mem</span>
          <span className="text-zinc-300 truncate ml-2">
            {mem ? `${formatBytes(memUsed)} / ${formatGiB(memTotal)}` : '--'}
          </span>
        </div>
        <div className="h-1.5 rounded-full bg-white/[0.06] overflow-hidden">
          <div
            className="h-full bg-[#76B900] rounded-full transition-[width] duration-300"
            style={{ width: `${memUsedPercent}%` }}
          />
        </div>
      </div>
    </button>
  )
}

/** Grid of node summary cards shown in the multi-node hardware overview.
 *  Cards fill the available container height — responsive grid that
 *  uses 2 columns on mobile, 4 on desktop, and scales to any node count. */
export function NodeOverview({ nodes, onSelect }: NodeOverviewProps) {
  // For >4 nodes, the grid wraps naturally. For <=4, cards stretch to fill height.
  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-2 lg:gap-2.5 h-full auto-rows-fr">
      {nodes.map((node, i) => (
        <NodeCard key={node.hostname} node={node} index={i} onSelect={onSelect} />
      ))}
    </div>
  )
}
