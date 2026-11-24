/**
 * Time-slice reducer for the Command tab.
 *
 * The Command tab's single dropdown (Live / 24h / 7d / 30d / 1y) drives BOTH
 * cluster cards AND the savings band, so the window -> time-range mapping
 * lives here, not in any component.
 *
 * Live semantics (owner-confirmed): the card counters show the current
 * instantaneous values, and savings accrued TODAY — i.e. the same
 * local-midnight -> now window the Historical view's "Today" preset uses,
 * with the dashboard's configured UTC offset honored, so the two views can
 * never disagree for the same window (task constraint D5).
 */

export type CommandWindowKind = 'live' | '24h' | '7d' | '30d' | '1y'

export interface CommandWindowRange {
  /** True for `live` — cards use instantaneous values, savings use TODAY. */
  isLive: boolean
  /** Window start in Unix ms (for `live`: local midnight per UTC offset). */
  since_ms: number
  /** Window end in Unix ms (always the fetch moment). */
  until_ms: number
  /** Dropdown label. */
  label: string
}

export const COMMAND_WINDOWS: readonly CommandWindowKind[] = [
  'live',
  '24h',
  '7d',
  '30d',
  '1y',
] as const

/** Duration of each non-live window in milliseconds. */
const WINDOW_DURATION_MS: Record<Exclude<CommandWindowKind, 'live'>, number> = {
  '24h': 24 * 3_600_000,
  '7d': 7 * 86_400_000,
  '30d': 30 * 86_400_000,
  '1y': 365 * 86_400_000,
}

/**
 * Midnight (in the user's configured UTC-offset timezone) that contains
 * `now`, expressed in UTC Unix ms — the exact algorithm HistoryView's
 * `presetRange('today')` uses, kept in sync deliberately.
 */
export function localMidnightMs(now: number, utcOffsetHours: number): number {
  const offsetMs = utcOffsetHours * 3_600_000
  const localDate = new Date(now + offsetMs)
  return (
    Date.UTC(
      localDate.getUTCFullYear(),
      localDate.getUTCMonth(),
      localDate.getUTCDate(),
    ) - offsetMs
  )
}

/**
 * Reduce a Command tab window selection to concrete time bounds.
 *
 * - `live` -> isLive, savings window = today (local midnight -> now)
 * - others -> a rolling aggregate window ending at `now`
 */
export function commandWindowRange(
  kind: CommandWindowKind,
  utcOffsetHours: number,
  now: number = Date.now(),
): CommandWindowRange {
  if (kind === 'live') {
    return {
      isLive: true,
      since_ms: localMidnightMs(now, utcOffsetHours),
      until_ms: now,
      label: 'Live',
    }
  }
  return {
    isLive: false,
    since_ms: now - WINDOW_DURATION_MS[kind],
    until_ms: now,
    label: kind,
  }
}
