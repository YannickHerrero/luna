import { spawn } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import type { ExtensionAPI } from '@earendil-works/pi-coding-agent'
import { Type } from 'typebox'
import { findScoutGitRoot, scoutEnvironment } from './scout-security.js'

const MAX_SCOUTS = 2
const MAX_OUTPUT_BYTES = 20 * 1024
const MAX_RECORD_BYTES = 2 * 1024 * 1024
const MAX_STDERR_BYTES = 16 * 1024
const DEFAULT_TIMEOUT_MS = 2 * 60 * 1000
const SCOUT_SYSTEM_PROMPT = `You are a read-only codebase scout working for a coordinating agent.
Inspect only the delegated repository using read, grep, find, and ls.
Return concise findings with exact file paths, symbols, evidence, risks, and unanswered questions.
Do not edit files, run commands, build, test, commit, push, deploy, or access paths outside the repository.
Do not delegate to another agent.`

type ScoutTask = {
  label: string
  task: string
}

type ScoutResult = {
  label: string
  output: string
  durationMs: number
  model?: string | undefined
  usage: {
    input: number
    output: number
    cacheRead: number
    cacheWrite: number
    cost: number
  }
}

type Invocation = {
  command: string
  args: string[]
}

type RunScoutOptions = {
  cwd: string
  model?: string | undefined
  signal?: AbortSignal | undefined
  timeoutMs?: number | undefined
  invocation?: Invocation | undefined
  guardPath?: string | undefined
  environment?: NodeJS.ProcessEnv | undefined
  onProgress?: ((message: string) => void) | undefined
}

type JsonEvent = {
  type?: string
  toolName?: string
  message?: {
    role?: string
    content?: Array<{ type?: string; text?: string }>
    model?: string
    usage?: {
      input?: number
      output?: number
      cacheRead?: number
      cacheWrite?: number
      cost?: { total?: number }
    }
  }
}

let activeScouts = 0
const scoutWaiters: Array<() => void> = []

export function registerReadOnlyScout(pi: ExtensionAPI) {
  pi.registerTool({
    name: 'luna_scout',
    label: 'Luna Scout',
    description:
      'Run one or two isolated read-only codebase investigations in parallel. Use for broad analysis across independent subsystems; do not use for small localized questions.',
    promptSnippet: 'Delegate broad read-only codebase investigation to at most two isolated scouts',
    promptGuidelines: [
      'Use luna_scout for broad investigations that can be split into independent read-only areas, then verify and synthesize the returned evidence yourself.',
      'Do not use luna_scout for implementation, commands, tests, Git operations, deployment, or simple localized questions.',
    ],
    parameters: Type.Object({
      tasks: Type.Array(
        Type.Object({
          label: Type.String({ minLength: 1, maxLength: 80 }),
          task: Type.String({ minLength: 1, maxLength: 12_000 }),
        }),
        { minItems: 1, maxItems: MAX_SCOUTS },
      ),
    }),
    async execute(_toolCallId, params, signal, onUpdate, ctx) {
      const statuses = params.tasks.map((task) => `${task.label}: queued`)
      const update = () =>
        onUpdate?.({
          content: [{ type: 'text', text: statuses.join('\n') }],
          details: { statuses: [...statuses] },
        })
      update()
      const root = findScoutGitRoot(ctx.cwd)
      if (!root) throw new Error('Luna scouts require a working directory inside a Git repository')
      const model = ctx.model ? `${ctx.model.provider}/${ctx.model.id}` : undefined
      const settled = await Promise.allSettled(
        params.tasks.map((task, index) =>
          withScoutSlot(signal, async () => {
            statuses[index] = `${task.label}: running`
            update()
            const result = await runScoutTask(task, {
              cwd: root,
              model,
              signal,
              onProgress: (message) => {
                statuses[index] = `${task.label}: ${message}`
                update()
              },
            })
            statuses[index] = `${task.label}: completed`
            update()
            return result
          }),
        ),
      )

      const results = settled.map((result, index) =>
        result.status === 'fulfilled'
          ? result.value
          : {
              label: params.tasks[index]?.label ?? `Scout ${index + 1}`,
              error: result.reason instanceof Error ? result.reason.message : String(result.reason),
            },
      )
      const text = results
        .map((result) =>
          'error' in result
            ? `## ${result.label} — failed\n\n${result.error}`
            : `## ${result.label}\n\n${result.output}`,
        )
        .join('\n\n---\n\n')
      return {
        content: [{ type: 'text', text }],
        details: { results },
      }
    },
  })
}

export async function runScoutTask(
  task: ScoutTask,
  options: RunScoutOptions,
): Promise<ScoutResult> {
  const started = performance.now()
  const promptDirectory = await mkdtemp(join(tmpdir(), 'luna-scout-'))
  const promptPath = join(promptDirectory, 'system.md')
  await writeFile(promptPath, SCOUT_SYSTEM_PROMPT, { mode: 0o600 })
  const guardPath = options.guardPath ?? fileURLToPath(new URL('./scout-guard.ts', import.meta.url))
  const invocation = options.invocation ?? getPiInvocation()
  const args = [
    ...invocation.args,
    '--mode',
    'json',
    '--print',
    '--no-session',
    '--no-extensions',
    '--extension',
    guardPath,
    '--no-skills',
    '--no-prompt-templates',
    '--no-context-files',
    '--no-approve',
    '--tools',
    'read,grep,find,ls',
    '--append-system-prompt',
    promptPath,
  ]
  if (options.model) args.push('--model', options.model)
  args.push(`Task: ${task.task}`)

  try {
    const result = await executeScoutProcess(task.label, invocation.command, args, {
      cwd: options.cwd,
      environment: options.environment ?? scoutEnvironment(process.env, options.cwd),
      signal: options.signal,
      timeoutMs: options.timeoutMs ?? DEFAULT_TIMEOUT_MS,
      onProgress: options.onProgress,
    })
    return {
      label: task.label,
      output: truncateUtf8(result.output, MAX_OUTPUT_BYTES),
      durationMs: Math.round(performance.now() - started),
      model: result.model,
      usage: result.usage,
    }
  } finally {
    await rm(promptDirectory, { recursive: true, force: true })
  }
}

function getPiInvocation(): Invocation {
  const currentScript = process.argv[1]
  if (currentScript && !currentScript.startsWith('/$bunfs/root/')) {
    return { command: process.execPath, args: [currentScript] }
  }
  const executable = basename(process.execPath).toLowerCase()
  if (!/^(node|bun)(\.exe)?$/.test(executable)) {
    return { command: process.execPath, args: [] }
  }
  return { command: 'pi', args: [] }
}

type ProcessResult = {
  output: string
  model?: string | undefined
  usage: ScoutResult['usage']
}

function executeScoutProcess(
  label: string,
  command: string,
  args: string[],
  options: {
    cwd: string
    environment: NodeJS.ProcessEnv
    signal?: AbortSignal | undefined
    timeoutMs: number
    onProgress?: ((message: string) => void) | undefined
  },
): Promise<ProcessResult> {
  return new Promise((resolveProcess, rejectProcess) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.environment,
      detached: process.platform !== 'win32',
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    let stdoutBuffer = ''
    let stderr = ''
    let output = ''
    let model: string | undefined
    let failure: Error | undefined
    let aborted = false
    const usage = { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, cost: 0 }

    const kill = (childSignal: NodeJS.Signals) => {
      if (child.pid && process.platform !== 'win32') {
        try {
          process.kill(-child.pid, childSignal)
        } catch {
          child.kill(childSignal)
        }
      } else {
        child.kill(childSignal)
      }
    }
    let forceKill: NodeJS.Timeout | undefined
    const scheduleForceKill = () => {
      if (forceKill) return
      forceKill = setTimeout(() => kill('SIGKILL'), 5_000)
      forceKill.unref()
    }
    const abort = () => {
      aborted = true
      kill('SIGTERM')
      scheduleForceKill()
    }
    const timeout = setTimeout(() => {
      failure = new Error(`Scout ${label} timed out after ${options.timeoutMs}ms`)
      kill('SIGTERM')
      scheduleForceKill()
    }, options.timeoutMs)
    timeout.unref()

    options.signal?.addEventListener('abort', abort, { once: true })
    if (options.signal?.aborted) abort()

    const processLine = (line: string) => {
      if (!line.trim()) return
      if (Buffer.byteLength(line, 'utf8') > MAX_RECORD_BYTES) {
        failure = new Error(`Scout ${label} emitted an oversized JSON record`)
        kill('SIGTERM')
        scheduleForceKill()
        return
      }
      let event: JsonEvent
      try {
        event = JSON.parse(line) as JsonEvent
      } catch {
        return
      }
      if (event.type === 'tool_execution_start' && event.toolName) {
        options.onProgress?.(event.toolName)
      }
      if (event.type !== 'message_end' || event.message?.role !== 'assistant') return
      const text = event.message.content
        ?.filter((part) => part.type === 'text' && typeof part.text === 'string')
        .map((part) => part.text ?? '')
        .join('\n')
      if (text) output = text
      model = event.message.model ?? model
      const eventUsage = event.message.usage
      if (eventUsage) {
        usage.input += eventUsage.input ?? 0
        usage.output += eventUsage.output ?? 0
        usage.cacheRead += eventUsage.cacheRead ?? 0
        usage.cacheWrite += eventUsage.cacheWrite ?? 0
        usage.cost += eventUsage.cost?.total ?? 0
      }
    }

    child.stdout.setEncoding('utf8')
    child.stdout.on('data', (chunk: string) => {
      stdoutBuffer += chunk
      if (Buffer.byteLength(stdoutBuffer, 'utf8') > MAX_RECORD_BYTES) {
        failure = new Error(`Scout ${label} exceeded its streaming buffer limit`)
        kill('SIGTERM')
        scheduleForceKill()
        return
      }
      const lines = stdoutBuffer.split('\n')
      stdoutBuffer = lines.pop() ?? ''
      for (const line of lines) processLine(line)
    })
    child.stderr.setEncoding('utf8')
    child.stderr.on('data', (chunk: string) => {
      stderr = truncateUtf8(`${stderr}${chunk}`, MAX_STDERR_BYTES)
    })
    child.on('error', (error) => {
      failure = error
    })
    child.on('close', (code) => {
      clearTimeout(timeout)
      if (forceKill) clearTimeout(forceKill)
      options.signal?.removeEventListener('abort', abort)
      if (stdoutBuffer.trim()) processLine(stdoutBuffer)
      if (aborted) {
        rejectProcess(new Error(`Scout ${label} was cancelled`))
      } else if (failure) {
        rejectProcess(failure)
      } else if (code !== 0) {
        rejectProcess(
          new Error(`Scout ${label} exited with code ${code}: ${stderr || 'no diagnostics'}`),
        )
      } else if (!output) {
        rejectProcess(new Error(`Scout ${label} returned no assistant output`))
      } else {
        resolveProcess({ output, model, usage })
      }
    })
  })
}

async function withScoutSlot<T>(
  signal: AbortSignal | undefined,
  operation: () => Promise<T>,
): Promise<T> {
  await acquireScoutSlot(signal)
  try {
    return await operation()
  } finally {
    activeScouts -= 1
    scoutWaiters.shift()?.()
  }
}

function acquireScoutSlot(signal?: AbortSignal): Promise<void> {
  if (signal?.aborted) return Promise.reject(new Error('Scout was cancelled before starting'))
  if (activeScouts < MAX_SCOUTS) {
    activeScouts += 1
    return Promise.resolve()
  }
  return new Promise((resolveSlot, rejectSlot) => {
    const ready = () => {
      signal?.removeEventListener('abort', abort)
      activeScouts += 1
      resolveSlot()
    }
    const abort = () => {
      const index = scoutWaiters.indexOf(ready)
      if (index >= 0) scoutWaiters.splice(index, 1)
      rejectSlot(new Error('Scout was cancelled before starting'))
    }
    scoutWaiters.push(ready)
    signal?.addEventListener('abort', abort, { once: true })
  })
}

function truncateUtf8(value: string, maximumBytes: number): string {
  const buffer = Buffer.from(value, 'utf8')
  if (buffer.byteLength <= maximumBytes) return value
  let truncated = buffer.subarray(0, maximumBytes).toString('utf8')
  if (truncated.endsWith('\uFFFD')) truncated = truncated.slice(0, -1)
  return `${truncated}\n\n[Scout output truncated]`
}
