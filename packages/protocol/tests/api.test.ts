import { Value } from 'typebox/value'
import { describe, expect, it } from 'vitest'
import { OpenAiWeeklyUsageSchema } from '../src/api.js'

describe('OpenAI weekly usage', () => {
  it('accepts only the sanitized account-level snapshot', () => {
    expect(
      Value.Check(OpenAiWeeklyUsageSchema, {
        availability: 'available',
        usedPercent: 63,
        resetsAt: '2030-03-17T17:46:40Z',
        collectedAt: '2026-08-01T00:00:00Z',
      }),
    ).toBe(true)
    expect(
      Value.Check(OpenAiWeeklyUsageSchema, {
        availability: 'available',
        usedPercent: 101,
      }),
    ).toBe(false)
    expect(
      Value.Check(OpenAiWeeklyUsageSchema, {
        availability: 'unavailable',
        accountId: 'must-not-leak',
      }),
    ).toBe(false)
  })
})
