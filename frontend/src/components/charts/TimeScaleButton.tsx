export type TimeScale = '1m' | '5m' | '1h' | '24h'

interface TimeScaleButtonProps {
  scale: TimeScale
  onCycle: () => void
  className?: string
}

/**
 * Small unobtrusive button that shows a clock icon and the current time scale
 * label. Cycles through 5m → 1h → 24h → 1m → 5m on click.
 *
 * Must be a `<button>` so FlipCard's click-interception (which checks for
 * interactive children) treats it as an interactive element and does NOT
 * toggle the flip.
 */
export function TimeScaleButton({ scale, onCycle, className = '' }: TimeScaleButtonProps) {
  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation()
        onCycle()
      }}
      aria-label={`Time scale: ${scale}. Click to change.`}
      className={`pointer-events-auto flex items-center gap-1 text-[10px] text-zinc-400 hover:text-zinc-200 transition-colors duration-150 cursor-pointer ${className}`}
    >
      <svg
        width="10"
        height="10"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        className="shrink-0"
      >
        <circle cx="12" cy="12" r="10" />
        <polyline points="12 6 12 12 16 14" />
      </svg>
      <span className="tabular-nums">{scale}</span>
    </button>
  )
}
