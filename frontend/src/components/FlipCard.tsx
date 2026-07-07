import { useState, type ReactNode } from 'react'

interface FlipCardProps {
  front: ReactNode
  back: ReactNode
  className?: string
  /** Minimum height in px for the card so the touch target is always tappable
   *  on mobile (where auto-height cards can collapse to near-zero). */
  minHeight?: number
}

/**
 * A CSS 3D card-flip container. Tapping/clicking the card rotates it to
 * reveal the back face. Works with keyboard (Enter/Space on focus).
 *
 * Uses `touch-action: manipulation` to prevent iOS double-tap zoom from
 * stealing the tap event. Requires an explicit minHeight so the card has
 * a tappable area on mobile even when content is minimal.
 */
export function FlipCard({ front, back, className = '', minHeight = 60 }: FlipCardProps) {
  const [flipped, setFlipped] = useState(false)

  const handleClick = () => setFlipped((f) => !f)

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      setFlipped((f) => !f)
    }
  }

  return (
    <div
      className={`flip-card-container ${className}`}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      role="button"
      tabIndex={0}
      aria-label={flipped ? 'Tap to show metric values' : 'Tap to show chart'}
    >
      <style>{`
        .flip-card-container {
          perspective: 1000px;
          cursor: pointer;
          touch-action: manipulation;
          -webkit-tap-highlight-color: transparent;
          user-select: none;
          -webkit-user-select: none;
        }
        .flip-card-inner {
          position: relative;
          width: 100%;
          min-height: ${minHeight}px;
          transition: transform 0.4s cubic-bezier(0.4, 0, 0.2, 1);
          transform-style: preserve-3d;
        }
        .flip-card-inner.flipped {
          transform: rotateY(180deg);
        }
        .flip-card-front,
        .flip-card-back {
          position: relative;
          width: 100%;
          min-height: ${minHeight}px;
          backface-visibility: hidden;
          -webkit-backface-visibility: hidden;
        }
        .flip-card-back {
          position: absolute;
          top: 0;
          left: 0;
          width: 100%;
          height: 100%;
          transform: rotateY(180deg);
          display: flex;
          align-items: center;
          justify-content: center;
        }
        /* Tiny hint in top-right corner showing the card is tappable */
        .flip-card-container .flip-hint {
          position: absolute;
          top: 2px;
          right: 4px;
          font-size: 9px;
          color: rgba(255,255,255,0.15);
          pointer-events: none;
          transition: opacity 0.2s;
          z-index: 1;
        }
        .flip-card-container:hover .flip-hint {
          opacity: 0.5;
        }
      `}</style>
      <div className={`flip-card-inner ${flipped ? 'flipped' : ''}`}>
        <div className="flip-card-front">
          <span className="flip-hint">{flipped ? '✕' : '📈'}</span>
          {front}
        </div>
        <div className="flip-card-back">
          {back}
        </div>
      </div>
    </div>
  )
}
