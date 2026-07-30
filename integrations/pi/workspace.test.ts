import { describe, expect, it } from 'vitest'
import { instrumentBash, quoteForShell, rewriteToolPath } from './workspace.js'

describe('Pi workspace bridge', () => {
  it('resolves relative tool paths against the logical workspace', () => {
    const input: Record<string, unknown> = { path: 'Sources/App.swift' }
    expect(rewriteToolPath('read', input, '/tmp/project')).toBe('/tmp/project/Sources/App.swift')
    expect(input.path).toBe('/tmp/project/Sources/App.swift')
  })

  it('leaves absolute paths absolute', () => {
    const input: Record<string, unknown> = { path: '/var/tmp/file.txt' }
    expect(rewriteToolPath('write', input, '/tmp/project')).toBe('/var/tmp/file.txt')
  })

  it('instruments bash without changing the original command text', () => {
    const command = "cd packages && printf '%s' okay"
    const instrumented = instrumentBash(command, "/tmp/report's", '/tmp/project')
    expect(instrumented).toContain(command)
    expect(instrumented).toContain("cd '/tmp/project'")
    expect(instrumented).toContain(quoteForShell("/tmp/report's"))
    expect(instrumented).toContain('trap')
  })
})
