import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const tokens = JSON.parse(
  readFileSync(new URL('../tokens.json', import.meta.url), 'utf8'),
) as Record<string, unknown>
const swift = readFileSync(new URL('../generated/LunaColors.swift', import.meta.url), 'utf8')

describe('design tokens', () => {
  it('contains both required Catppuccin themes', () => {
    expect(tokens).toHaveProperty('latte')
    expect(tokens).toHaveProperty('mocha')
  })

  it('generates shared Swift colors, shape, and motion constants', () => {
    expect(swift).toContain('enum LunaColors')
    expect(swift).toContain('enum LunaShape')
    expect(swift).toContain('static let minimumTarget: CGFloat = 44.0')
    expect(swift).toContain('static let standardDuration = 0.200')
  })
})
