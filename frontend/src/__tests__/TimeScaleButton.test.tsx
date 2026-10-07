import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { TimeScaleButton } from '@/components/charts/TimeScaleButton'
import type { TimeScale } from '@/components/charts/TimeScaleButton'

describe('TimeScaleButton', () => {
  it('renders with correct scale label', () => {
    const scales: TimeScale[] = ['5m', '1h', '24h', '1m']
    for (const scale of scales) {
      const { unmount } = render(<TimeScaleButton scale={scale} onCycle={() => {}} />)
      expect(screen.getByText(scale)).toBeInTheDocument()
      unmount()
    }
  })

  it('calls onCycle when clicked', () => {
    const onCycle = vi.fn()
    render(<TimeScaleButton scale="5m" onCycle={onCycle} />)
    const btn = screen.getByRole('button')
    fireEvent.click(btn)
    expect(onCycle).toHaveBeenCalledTimes(1)
  })

  it('does not propagate click to parent', () => {
    const onParentClick = vi.fn()
    const onCycle = vi.fn()
    render(
      <div onClick={onParentClick}>
        <TimeScaleButton scale="5m" onCycle={onCycle} />
      </div>,
    )
    const btn = screen.getByRole('button')
    fireEvent.click(btn)
    // onCycle should fire, but parent onClick should NOT (stopPropagation)
    expect(onCycle).toHaveBeenCalledTimes(1)
    expect(onParentClick).not.toHaveBeenCalled()
  })

  it('is a <button> element (for FlipCard click-interception)', () => {
    render(<TimeScaleButton scale="1h" onCycle={() => {}} />)
    const btn = screen.getByRole('button')
    expect(btn.tagName).toBe('BUTTON')
  })

  it('renders the clock SVG icon', () => {
    const { container } = render(<TimeScaleButton scale="5m" onCycle={() => {}} />)
    const svg = container.querySelector('svg')
    expect(svg).not.toBeNull()
  })
})
