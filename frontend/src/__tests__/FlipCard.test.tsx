import { render, screen, fireEvent } from '@testing-library/react'
import { describe, it, expect } from 'vitest'
import { FlipCard } from '@/components/FlipCard'

describe('FlipCard', () => {
  it('renders front content by default', () => {
    render(
      <FlipCard
        front={<div data-testid="front">Front Content</div>}
        back={<div data-testid="back">Back Content</div>}
      />,
    )
    expect(screen.getByTestId('front')).toBeInTheDocument()
    expect(screen.queryByTestId('back')).not.toBeInTheDocument()
  })

  it('flips to back content on click', () => {
    render(
      <FlipCard
        front={<div data-testid="front">Front Content</div>}
        back={<div data-testid="back">Back Content</div>}
      />,
    )
    const card = screen.getByRole('button')
    fireEvent.click(card)
    expect(screen.getByTestId('back')).toBeInTheDocument()
    expect(screen.queryByTestId('front')).not.toBeInTheDocument()
  })

  it('does not flip when clicking an interactive child button', () => {
    render(
      <FlipCard
        front={
          <div>
            <span>Front</span>
            <button data-testid="inner-button" type="button">Inner</button>
          </div>
        }
        back={<div data-testid="back">Back Content</div>}
      />,
    )
    const innerButton = screen.getByTestId('inner-button')
    fireEvent.click(innerButton)
    // The card should NOT have flipped
    expect(screen.queryByTestId('back')).not.toBeInTheDocument()
  })

  it('does not flip when clicking an element with role="button"', () => {
    render(
      <FlipCard
        front={
          <div>
            <span>Front</span>
            <div data-testid="pseudo-button" role="button" tabIndex={0}>Pseudo</div>
          </div>
        }
        back={<div data-testid="back">Back Content</div>}
      />,
    )
    const pseudoButton = screen.getByTestId('pseudo-button')
    fireEvent.click(pseudoButton)
    expect(screen.queryByTestId('back')).not.toBeInTheDocument()
  })

  it('does not flip when clicking an input element', () => {
    render(
      <FlipCard
        front={
          <div>
            <span>Front</span>
            <input data-testid="inner-input" type="text" />
          </div>
        }
        back={<div data-testid="back">Back Content</div>}
      />,
    )
    const input = screen.getByTestId('inner-input')
    fireEvent.click(input)
    expect(screen.queryByTestId('back')).not.toBeInTheDocument()
  })

  it('flips back on second click', () => {
    render(
      <FlipCard
        front={<div data-testid="front">Front Content</div>}
        back={<div data-testid="back">Back Content</div>}
      />,
    )
    const card = screen.getByRole('button')
    // First click — shows back
    fireEvent.click(card)
    expect(screen.getByTestId('back')).toBeInTheDocument()
    // Second click — shows front again
    fireEvent.click(card)
    expect(screen.getByTestId('front')).toBeInTheDocument()
  })

  it('flips on Enter key', () => {
    render(
      <FlipCard
        front={<div data-testid="front">Front Content</div>}
        back={<div data-testid="back">Back Content</div>}
      />,
    )
    const card = screen.getByRole('button')
    fireEvent.keyDown(card, { key: 'Enter' })
    expect(screen.getByTestId('back')).toBeInTheDocument()
  })

  it('flips on Space key', () => {
    render(
      <FlipCard
        front={<div data-testid="front">Front Content</div>}
        back={<div data-testid="back">Back Content</div>}
      />,
    )
    const card = screen.getByRole('button')
    fireEvent.keyDown(card, { key: ' ' })
    expect(screen.getByTestId('back')).toBeInTheDocument()
  })

  it('does not flip on non-activation key', () => {
    render(
      <FlipCard
        front={<div data-testid="front">Front Content</div>}
        back={<div data-testid="back">Back Content</div>}
      />,
    )
    const card = screen.getByRole('button')
    fireEvent.keyDown(card, { key: 'Tab' })
    expect(screen.queryByTestId('back')).not.toBeInTheDocument()
  })

  it('applies custom className', () => {
    render(
      <FlipCard
        front={<div>Front</div>}
        back={<div>Back</div>}
        className="custom-class"
      />,
    )
    const card = screen.getByRole('button')
    expect(card.className).toContain('custom-class')
  })

  it('applies minHeight to the inner wrapper', () => {
    render(
      <FlipCard
        front={<div>Front</div>}
        back={<div>Back</div>}
        minHeight={120}
      />,
    )
    const card = screen.getByRole('button')
    // The inner div with minHeight should exist
    const innerDiv = card.querySelector('div > div')
    expect(innerDiv).not.toBeNull()
    expect((innerDiv as HTMLElement).style.minHeight).toBe('120px')
  })

  it('has correct ARIA label on front vs back', () => {
    render(
      <FlipCard
        front={<div>Front Content</div>}
        back={<div>Back Content</div>}
      />,
    )
    const card = screen.getByRole('button')
    expect(card).toHaveAttribute('aria-label', 'Tap to show chart')
    // Flip
    fireEvent.click(card)
    expect(card).toHaveAttribute('aria-label', 'Tap to show metric values')
  })
})