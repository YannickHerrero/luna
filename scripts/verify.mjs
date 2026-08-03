#!/usr/bin/env node

import { spawn } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const SCHEMA_VERSION = 1
const DEFAULT_CONCURRENCY = 4

export function verificationLanes(profile = 'full') {
  if (!['code', 'full'].includes(profile)) {
    throw new Error(`Unknown verification profile: ${profile}`)
  }

  const lanes = [
    lane('generate', 'pnpm', ['generate']),
    lane('runner-tests', 'pnpm', ['test:verification'], ['generate']),
    lane('rust-tests', 'cargo', ['test', '--workspace'], ['runner-tests']),
    lane('web-tests', 'pnpm', ['test:web'], ['rust-tests']),
    lane('typecheck', 'pnpm', ['typecheck'], ['rust-tests']),
    lane('lint', 'pnpm', ['lint'], ['rust-tests']),
  ]

  if (profile === 'full') {
    const checks = ['web-tests', 'rust-tests', 'typecheck', 'lint']
    lanes.push(
      lane('browser-e2e', 'pnpm', ['--filter', '@luna/web', 'test:e2e'], checks),
      lane('release-build', 'pnpm', ['build'], ['browser-e2e']),
    )
  }

  return lanes
}

function lane(id, command, args, dependencies = []) {
  return { id, command, args, dependencies }
}

export async function runVerification({
  profile = 'full',
  lanes = verificationLanes(profile),
  concurrency = DEFAULT_CONCURRENCY,
  execute = executeLane,
  signal,
  onLaneStart = defaultLaneStart,
  onLaneFinish = defaultLaneFinish,
} = {}) {
  validateLanes(lanes)
  const startedAt = new Date()
  const started = performance.now()
  const results = new Map(
    lanes.map((item) => [
      item.id,
      {
        id: item.id,
        command: item.command,
        args: item.args,
        dependencies: item.dependencies,
        status: 'pending',
      },
    ]),
  )
  const pending = new Map(lanes.map((item) => [item.id, item]))
  const limit = Math.max(1, Math.floor(concurrency))

  while (pending.size > 0) {
    if (signal?.aborted) {
      for (const item of pending.values()) {
        results.get(item.id).status = 'cancelled'
      }
      break
    }

    let changed = false
    for (const [id, item] of pending) {
      const blocked = item.dependencies.some((dependency) =>
        ['failed', 'skipped', 'cancelled'].includes(results.get(dependency)?.status),
      )
      if (blocked) {
        results.get(id).status = 'skipped'
        pending.delete(id)
        changed = true
      }
    }

    const ready = [...pending.values()].filter((item) =>
      item.dependencies.every((dependency) => results.get(dependency)?.status === 'passed'),
    )
    if (ready.length === 0) {
      if (pending.size === 0) break
      if (changed) continue
      throw new Error(`Verification graph cannot make progress: ${[...pending.keys()].join(', ')}`)
    }

    const batch = ready.slice(0, limit)
    await Promise.all(
      batch.map(async (item) => {
        pending.delete(item.id)
        const result = results.get(item.id)
        result.status = 'running'
        result.startedAt = new Date().toISOString()
        onLaneStart?.(item)
        const laneStarted = performance.now()
        let exitCode
        try {
          exitCode = await execute(item, { signal })
        } catch (error) {
          result.error = error instanceof Error ? error.message : String(error)
          exitCode = 1
        }
        result.durationMs = Math.round(performance.now() - laneStarted)
        result.exitCode = exitCode
        result.status = exitCode === 0 ? 'passed' : signal?.aborted ? 'cancelled' : 'failed'
        onLaneFinish?.(result)
      }),
    )
  }

  const finishedAt = new Date()
  const report = {
    schemaVersion: SCHEMA_VERSION,
    profile,
    startedAt: startedAt.toISOString(),
    finishedAt: finishedAt.toISOString(),
    durationMs: Math.round(performance.now() - started),
    concurrency: limit,
    status: [...results.values()].every((result) => result.status === 'passed')
      ? 'passed'
      : signal?.aborted
        ? 'cancelled'
        : 'failed',
    lanes: [...results.values()],
  }
  return report
}

function validateLanes(lanes) {
  const ids = new Set()
  for (const item of lanes) {
    if (!item.id || ids.has(item.id)) throw new Error(`Duplicate or empty lane id: ${item.id}`)
    ids.add(item.id)
  }
  for (const item of lanes) {
    for (const dependency of item.dependencies) {
      if (!ids.has(dependency)) {
        throw new Error(`Lane ${item.id} has unknown dependency ${dependency}`)
      }
    }
  }
}

export function executeLane(item, { signal } = {}) {
  return new Promise((resolveExecution) => {
    const child = spawn(item.command, item.args, {
      cwd: process.cwd(),
      env: process.env,
      detached: process.platform !== 'win32',
      stdio: 'inherit',
    })
    let settled = false
    let forceKill
    const kill = (childSignal) => {
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
    const finish = (exitCode) => {
      if (settled) return
      settled = true
      if (forceKill) clearTimeout(forceKill)
      signal?.removeEventListener('abort', abort)
      resolveExecution(exitCode)
    }
    const abort = () => {
      kill('SIGTERM')
      forceKill = setTimeout(() => kill('SIGKILL'), 5_000)
      forceKill.unref()
    }
    signal?.addEventListener('abort', abort, { once: true })
    if (signal?.aborted) abort()
    child.on('error', () => finish(1))
    child.on('close', (code, childSignal) => finish(code ?? (childSignal ? 130 : 1)))
  })
}

export async function writeReport(report, outputPath) {
  const path = resolve(
    outputPath ?? `.data/verification/${safeTimestamp(report.startedAt)}-${report.profile}.json`,
  )
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify(report, null, 2)}\n`, { mode: 0o600 })
  return path
}

function safeTimestamp(value) {
  return value.replace(/[-:]/g, '').replace(/\.\d{3}Z$/, 'Z')
}

function defaultLaneStart(item) {
  console.log(`\n[verify:${item.id}] ${item.command} ${item.args.join(' ')}`)
}

function defaultLaneFinish(result) {
  console.log(`[verify:${result.id}] ${result.status} in ${(result.durationMs / 1000).toFixed(2)}s`)
}

function parseArguments(argv) {
  let profile = 'full'
  let concurrency = DEFAULT_CONCURRENCY
  let outputPath
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]
    if (argument === 'code' || argument === 'full') profile = argument
    else if (argument === '--concurrency') concurrency = Number(argv[++index])
    else if (argument === '--output') outputPath = argv[++index]
    else throw new Error(`Unknown argument: ${argument}`)
  }
  if (!Number.isInteger(concurrency) || concurrency < 1 || concurrency > 16) {
    throw new Error(`Concurrency must be an integer from 1 to 16, received ${concurrency}`)
  }
  return { profile, concurrency, outputPath }
}

async function main() {
  const options = parseArguments(process.argv.slice(2))
  const controller = new AbortController()
  const abort = () => controller.abort()
  process.once('SIGINT', abort)
  process.once('SIGTERM', abort)
  try {
    const report = await runVerification({
      profile: options.profile,
      concurrency: options.concurrency,
      signal: controller.signal,
    })
    const reportPath = await writeReport(report, options.outputPath)
    console.log(
      `\nVerification ${report.status} in ${(report.durationMs / 1000).toFixed(2)}s. Report: ${reportPath}`,
    )
    process.exitCode = report.status === 'passed' ? 0 : report.status === 'cancelled' ? 130 : 1
  } finally {
    process.removeListener('SIGINT', abort)
    process.removeListener('SIGTERM', abort)
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  await main()
}
