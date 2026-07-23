import { useState, type ReactNode } from 'react'

interface FlipCardProps {
  front: ReactNode
  back: ReactNode
  className?: string
  /** Minimum height in px so the card has a tappable target on mobile
   *  even when content is sparse. */
  minHeight?: number
}

/**
 * A tappable card that toggles between front and back content.
 * Uses simple state-based show/hide (no CSS 3D transforms) so the
 * back-face chart fills the card correctly without position/overflow
 * issues.
 *
 * - Front: metric values (the default view)
 * - Back: time-series chart (shown after tapping)
 *
 * Accessible: keyboard (Enter/Space), ARIA label, role="button".
 */
export function FlipCard({ front, back, className = '', minHeight = 60 }: FlipCardProps) {
  const [flipped, setFlipped] = useState(false)

  const handleClick = (e: React.MouseEvent) => {
    // Don't flip when the user clicks on an interactive child (button, input, link,
    // select, textarea, or another role="button" element). This prevents nested
    // controls like SloSettingsControl's gear button from also triggering the flip.
    // We compare against e.currentTarget (the FlipCard wrapper itself) so the
    // FlipCard's own role="button" doesn't cause a false-positive short-circuit.
    const target = e.target as HTMLElement
    const interactive = target.closest('button, [role="button"], input, select, textarea, a')
    if (interactive && interactive !== e.currentTarget) return
    setFlipped((f) => !f)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      setFlipped((f) => !f)
    }
  }

  return (
    <div
      className={`cursor-pointer select-none ${className}`}
      style={{ touchAction: 'manipulation' } as React.CSSProperties}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      role="button"
      tabIndex={0}
      aria-label={flipped ? 'Tap to show metric values' : 'Tap to show chart'}
    >
      <div style={{ minHeight }} className="relative">
        {/* Small tap-hint in the top-right corner */}
        <span
          className="absolute top-0.5 right-1 text-[9px] pointer-events-none z-[1]"
          style={{ color: 'rgba(255,255,255,0.15)' }}
        >
          {flipped ? '✕' : '📈'}
        </span>
        {/* Keyed so React unmounts/remounts on toggle, triggering the CSS fade */}
        <div className="animate-[flipFadeIn_0.2s_ease-out]" key={flipped ? 'back' : 'front'}>
          {flipped ? back : front}
        </div>
      </div>
    </div>
  )
}
