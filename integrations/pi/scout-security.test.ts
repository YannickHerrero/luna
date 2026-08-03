import { randomUUID } from 'node:crypto'
import { mkdirSync, mkdtempSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import {
  findScoutGitRoot,
  isAllowedScoutTool,
  isScoutPathAllowed,
  scoutEnvironment,
} from './scout-security.js'

const directories: Array<{ remove: () => void }> = []

function temporaryDirectory() {
  const root = mkdtempSync(join(tmpdir(), `.scout-security-${randomUUID()}-`))
  directories.push({ remove: () => rmSync(root, { recursive: true, force: true }) })
  return root
}

afterEach(() => {
  for (const directory of directories.splice(0)) directory.remove()
})

describe('scout security', () => {
  it('allows only read-only tools', () => {
    expect(['read', 'grep', 'find', 'ls'].every(isAllowedScoutTool)).toBe(true)
    expect(isAllowedScoutTool('bash')).toBe(false)
    expect(isAllowedScoutTool('edit')).toBe(false)
  })

  it('finds the containing Git root and refuses unrelated directories', () => {
    const parent = temporaryDirectory()
    const root = join(parent, 'repository')
    const nested = join(root, 'apps', 'client')
    mkdirSync(join(root, '.git'), { recursive: true })
    mkdirSync(nested, { recursive: true })

    expect(findScoutGitRoot(nested)).toBe(realpathSync(root))
    expect(findScoutGitRoot(parent)).toBeUndefined()
  })

  it('contains reads within the repository even through symlinks and @ paths', () => {
    const parent = temporaryDirectory()
    const root = join(parent, 'repository')
    const outside = join(parent, 'private')
    mkdirSync(root)
    mkdirSync(outside)
    writeFileSync(join(root, 'inside.txt'), 'inside')
    writeFileSync(join(outside, 'secret.txt'), 'secret')
    symlinkSync(outside, join(root, 'escape'))

    expect(isScoutPathAllowed(root, root, 'inside.txt')).toBe(true)
    expect(isScoutPathAllowed(root, root, 'missing/subtree')).toBe(true)
    expect(isScoutPathAllowed(root, root, '../private/secret.txt')).toBe(false)
    expect(isScoutPathAllowed(root, root, join(root, 'escape', 'secret.txt'))).toBe(false)
    expect(isScoutPathAllowed(root, root, `@${join(outside, 'secret.txt')}`)).toBe(false)
  })

  it('passes only runtime and provider variables to children', () => {
    const root = temporaryDirectory()
    const environment = scoutEnvironment(
      {
        HOME: '/home/test',
        PATH: '/usr/bin',
        OPENAI_API_KEY: 'provider-key',
        LUNA_APNS_KEY_ID: 'private-metadata',
        UNRELATED_SECRET: 'do-not-copy',
      },
      root,
    )

    expect(environment.HOME).toBe('/home/test')
    expect(environment.OPENAI_API_KEY).toBe('provider-key')
    expect(environment.LUNA_APNS_KEY_ID).toBeUndefined()
    expect(environment.UNRELATED_SECRET).toBeUndefined()
    expect(environment.LUNA_SCOUT_ROOT).toBe(realpathSync(root))
    expect(environment.PI_TELEMETRY).toBe('0')
  })
})
