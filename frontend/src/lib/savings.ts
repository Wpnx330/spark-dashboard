/**
 * Cost-savings math shared by the Historical view and the Command tab.
 *
 * This is THE single source of truth for the electricity-rate +
 * API-equivalent pricing model: cloud cost avoided is what the same tokens
 * would have cost at the configured per-1M cloud prompt/gen rates, minus the
 * electricity cost of the energy the cluster actually drew. Both views must
 * agree for the same window (Command-tab Live savings == Historical day view).
 */

export interface SavingsSummaryShape {
  delta_prompt_tokens: number
  delta_gen_tokens: number
  power_kwh: number
}

export interface SavingsRates {
  /** $ per 1M prompt tokens (cloud API equivalent). */
  cloud_prompt_rate: number
  /** $ per 1M generation tokens (cloud API equivalent). */
  cloud_gen_rate: number
  /** $ per kWh electricity. */
  electricity_rate: number
}

export interface SavingsBreakdown {
  /** What the window's tokens would have cost via a cloud API. */
  cloudCost: number
  /** What the window's energy cost at the local electricity rate. */
  powerCost: number
  /** cloudCost - powerCost; negative means the cluster cost more than cloud. */
  netSavings: number
  /** Raw energy used in the window (kWh), surfaced as-is. */
  powerKwh: number
}

/**
 * Compute the savings breakdown for one summary window. A null/zero summary
 * (no data recorded yet) yields all zeros, matching the previous inline math
 * in HistoryView exactly.
 *
 * Note: `power_kwh` is recorded from all online nodes automatically (see
 * `metrics_collector`), so no GPU-count multiplier is applied here.
 */
export function computeSavings(
  summary: SavingsSummaryShape | null,
  rates: SavingsRates,
): SavingsBreakdown {
  if (!summary) {
    return { cloudCost: 0, powerCost: 0, netSavings: 0, powerKwh: 0 }
  }
  const cloudCost =
    (summary.delta_prompt_tokens / 1_000_000) * rates.cloud_prompt_rate +
    (summary.delta_gen_tokens / 1_000_000) * rates.cloud_gen_rate
  const powerCost = summary.power_kwh * rates.electricity_rate
  return {
    cloudCost,
    powerCost,
    netSavings: cloudCost - powerCost,
    powerKwh: summary.power_kwh,
  }
}

/** Sum several clusters' breakdowns into the grand-total row. */
export function sumSavings(parts: SavingsBreakdown[]): SavingsBreakdown {
  return parts.reduce(
    (acc, p) => ({
      cloudCost: acc.cloudCost + p.cloudCost,
      powerCost: acc.powerCost + p.powerCost,
      netSavings: acc.netSavings + p.netSavings,
      powerKwh: acc.powerKwh + p.powerKwh,
    }),
    { cloudCost: 0, powerCost: 0, netSavings: 0, powerKwh: 0 },
  )
}
