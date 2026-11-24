import { describe, it, expect } from 'vitest'
import {
  computeSavings,
  sumSavings,
} from '@/lib/savings'

const RATES = {
  cloud_prompt_rate: 1.5, // $/1M prompt tokens
  cloud_gen_rate: 6.0, // $/1M gen tokens
  electricity_rate: 0.12, // $/kWh
}

const SUMMARY = {
  delta_prompt_tokens: 2_000_000, // -> $3.00
  delta_gen_tokens: 1_000_000, // -> $6.00
  power_kwh: 10, // -> $1.20
}

describe('computeSavings', () => {
  it('cloud = prompt/1M*rate + gen/1M*rate; power = kWh*rate', () => {
    const r = computeSavings(SUMMARY, RATES)
    expect(r.cloudCost).toBeCloseTo(9.0, 10)
    expect(r.powerCost).toBeCloseTo(1.2, 10)
    expect(r.netSavings).toBeCloseTo(7.8, 10)
    expect(r.powerKwh).toBe(10)
  })

  it('null summary (no data) yields all zeros', () => {
    expect(computeSavings(null, RATES)).toEqual({
      cloudCost: 0,
      powerCost: 0,
      netSavings: 0,
      powerKwh: 0,
    })
  })

  it('negative net savings when electricity outweighs cloud cost', () => {
    const r = computeSavings(
      { delta_prompt_tokens: 0, delta_gen_tokens: 0, power_kwh: 100 },
      RATES,
    )
    expect(r.netSavings).toBeCloseTo(-12, 10)
  })

  it('zero rates yield zero savings for any usage', () => {
    expect(computeSavings(SUMMARY, { cloud_prompt_rate: 0, cloud_gen_rate: 0, electricity_rate: 0 }).netSavings).toBe(0)
  })
})

describe('sumSavings (grand total)', () => {
  it('sums net savings across clusters into the grand total', () => {
    const a = computeSavings(SUMMARY, RATES) // 7.80
    const b = computeSavings(
      { delta_prompt_tokens: 1_000_000, delta_gen_tokens: 0, power_kwh: 1 },
      RATES,
    ) // 1.5 - 0.12 = 1.38
    const total = sumSavings([a, b])
    expect(total.netSavings).toBeCloseTo(9.18, 10)
    expect(total.cloudCost).toBeCloseTo(10.5, 10)
    expect(total.powerCost).toBeCloseTo(1.32, 10)
    expect(total.powerKwh).toBe(11)
  })

  it('empty list -> zero total', () => {
    expect(sumSavings([]).netSavings).toBe(0)
  })

  it('mixed signs still add up', () => {
    const total = sumSavings([
      { cloudCost: 5, powerCost: 1, netSavings: 4, powerKwh: 2 },
      { cloudCost: 0, powerCost: 6, netSavings: -6, powerKwh: 10 },
    ])
    expect(total.netSavings).toBeCloseTo(-2, 10)
  })
})
