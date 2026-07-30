import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const tokens = JSON.parse(
  readFileSync(new URL('../tokens.json', import.meta.url), 'utf8'),
) as Record<string, unknown>

describe('design tokens', () => {
  it('contains both required Catppuccin themes', () => {
    expect(tokens).toHaveProperty('latte')
    expect(tokens).toHaveProperty('mocha')
  })
})
