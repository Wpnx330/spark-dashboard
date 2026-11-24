import { describe, it, expect } from 'vitest'
import {
  commandWindowRange,
  localMidnightMs,
  COMMAND_WINDOWS,
  type CommandWindowKind,
} from '@/lib/commandWindow'

describe('localMidnightMs', () => {
  it('returns UTC midnight when offset is 0', () => {
    // 2026-11-24T15:30:00Z
    const now = Date.UTC(2026, 10, 24, 15, 30)
    expect(localMidnightMs(now, 0)).toBe(Date.UTC(2026, 10, 24))
  })

  it('honors a negative UTC offset (e.g. US Eastern, -4)', () => {
    // 2026-11-24T03:00:00Z is still 2026-11-23 23:00 in UTC-4.
    const now = Date.UTC(2026, 10, 24, 3)
    const midnight = localMidnightMs(now, -4)
    // Local midnight 2026-11-23 (UTC-4) == 2026-11-24T04:00:00Z? No:
    // local midnight = local day start = 04:00Z for day 23? Verify:
    // local time = UTC - 4 -> local day 23 starts at 23T04:00Z.
    expect(midnight).toBe(Date.UTC(2026, 10, 23, 4))
  })

  it('honors a positive UTC offset (e.g. +5)', () => {
    // 2026-11-24T20:00:00Z is 2026-11-25 01:00 local (+5): day 25 starts
    // at 24T19:00Z.
    const now = Date.UTC(2026, 10, 24, 20)
    expect(localMidnightMs(now, 5)).toBe(Date.UTC(2026, 10, 24, 19))
  })
})

describe('commandWindowRange', () => {
  const now = Date.UTC(2026, 10, 24, 12)

  it('live: isLive=true and savings window is TODAY (local midnight -> now)', () => {
    const r = commandWindowRange('live', -4, now)
    expect(r.isLive).toBe(true)
    expect(r.since_ms).toBe(localMidnightMs(now, -4))
    expect(r.until_ms).toBe(now)
    expect(r.label).toBe('Live')
  })

  it('live matches the Historical "today" preset for the same inputs', () => {
    // Same algorithm as HistoryView presetRange('today'): both derive
    // local midnight from now + offset; equality here pins D5.
    const utcOffset = -4
    const offsetMs = utcOffset * 3_600_000
    const localDate = new Date(now + offsetMs)
    const histMidnight =
      Date.UTC(
        localDate.getUTCFullYear(),
        localDate.getUTCMonth(),
        localDate.getUTCDate(),
      ) - offsetMs
    expect(commandWindowRange('live', utcOffset, now).since_ms).toBe(
      histMidnight,
    )
  })

  it.each<[CommandWindowKind, number]>([
    ['24h', 24 * 3_600_000],
    ['7d', 7 * 86_400_000],
    ['30d', 30 * 86_400_000],
    ['1y', 365 * 86_400_000],
  ])('%s: rolling window ending at now', (kind, dur) => {
    const r = commandWindowRange(kind, 0, now)
    expect(r.isLive).toBe(false)
    expect(r.until_ms).toBe(now)
    expect(r.since_ms).toBe(now - dur)
  })

  it('exposes exactly the five windows in order', () => {
    expect(COMMAND_WINDOWS).toEqual(['live', '24h', '7d', '30d', '1y'])
  })
})
