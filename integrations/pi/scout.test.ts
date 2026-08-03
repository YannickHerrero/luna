import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { runScoutTask } from './scout.js'

const directories: string[] = []

function fixture(source: string): { root: string; script: string } {
  const root = mkdtempSync(join(tmpdir(), 'luna-scout-test-'))
  const script = join(root, 'fake-pi.mjs')
  writeFileSync(script, source)
  directories.push(root)
  return { root, script }
}

afterEach(() => {
  for (const directory of directories.splice(0)) {
    rmSync(directory, { recursive: true, force: true })
  }
})

describe('read-only scout runner', () => {
  it('uses an isolated Pi invocation and returns bounded structured output', async () => {
    const { root, script } = fixture(`
      process.stdout.write(JSON.stringify({type:'tool_execution_start',toolName:'read'}) + '\\n')
      process.stdout.write(JSON.stringify({
        type:'message_end',
        message:{
          role:'assistant',
          content:[{type:'text',text:JSON.stringify({argv:process.argv.slice(2),root:process.env.LUNA_SCOUT_ROOT})}],
          model:'test/scout',
          usage:{input:12,output:4,cacheRead:3,cacheWrite:2,cost:{total:0.01}}
        }
      }) + '\\n')
    `)
    const progress: string[] = []
    const result = await runScoutTask(
      { label: 'runtime', task: 'Inspect runtime boundaries' },
      {
        cwd: root,
        invocation: { command: process.execPath, args: [script] },
        guardPath: join(root, 'guard.ts'),
        environment: { LUNA_SCOUT_ROOT: root },
        onProgress: (message) => progress.push(message),
      },
    )
    const output = JSON.parse(result.output) as { argv: string[]; root: string }

    expect(output.argv).toContain('--no-extensions')
    expect(output.argv).toContain('--no-context-files')
    expect(output.argv).toContain('--no-approve')
    expect(output.argv).toContain('read,grep,find,ls')
    expect(output.root).toBe(root)
    expect(progress).toEqual(['read'])
    expect(result.model).toBe('test/scout')
    expect(result.usage).toEqual({ input: 12, output: 4, cacheRead: 3, cacheWrite: 2, cost: 0.01 })
  })

  it('propagates cancellation to the child process', async () => {
    const { root, script } = fixture(`setInterval(() => {}, 1000)`)
    const controller = new AbortController()
    const pending = runScoutTask(
      { label: 'cancelled', task: 'Wait forever' },
      {
        cwd: root,
        invocation: { command: process.execPath, args: [script] },
        guardPath: join(root, 'guard.ts'),
        environment: { LUNA_SCOUT_ROOT: root },
        signal: controller.signal,
        timeoutMs: 5_000,
      },
    )

    setTimeout(() => controller.abort(), 50)
    await expect(pending).rejects.toThrow('Scout cancelled was cancelled')
  })

  it('caps model-visible output', async () => {
    const { root, script } = fixture(`
      process.stdout.write(JSON.stringify({
        type:'message_end',
        message:{role:'assistant',content:[{type:'text',text:'x'.repeat(30000)}]}
      }) + '\\n')
    `)
    const result = await runScoutTask(
      { label: 'large', task: 'Return a large result' },
      {
        cwd: root,
        invocation: { command: process.execPath, args: [script] },
        guardPath: join(root, 'guard.ts'),
        environment: { LUNA_SCOUT_ROOT: root },
      },
    )

    expect(Buffer.byteLength(result.output, 'utf8')).toBeLessThan(21 * 1024)
    expect(result.output).toContain('[Scout output truncated]')
  })
})
