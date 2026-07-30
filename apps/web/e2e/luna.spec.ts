import { expect, test } from '@playwright/test'
import { chmod, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'

const root = resolve(import.meta.dirname, '../../..')
let server: ChildProcessWithoutNullStreams
let directory: string
let pairingCode: string

const fakePi = `#!/usr/bin/env node
const net = require('node:net')
const bridge = net.createConnection(process.env.LUNA_BRIDGE_SOCKET)
let dispatchId
let bridgeBuffer = ''
bridge.on('connect', () => bridge.write(JSON.stringify({type:'ready', pid:process.pid, cwd:process.cwd()}) + '\\n'))
bridge.on('data', chunk => {
  bridgeBuffer += chunk.toString('utf8')
  while (bridgeBuffer.includes('\\n')) {
    const index = bridgeBuffer.indexOf('\\n')
    const command = JSON.parse(bridgeBuffer.slice(0, index))
    bridgeBuffer = bridgeBuffer.slice(index + 1)
    if (command.type === 'dispatch') {
      dispatchId = command.dispatchId
      bridge.write(JSON.stringify({type:'dispatch_ready', dispatchId}) + '\\n')
    }
  }
})
let input = ''
const reply = value => process.stdout.write(JSON.stringify(value) + '\\n')
process.stdin.on('data', chunk => {
  input += chunk.toString('utf8')
  while (input.includes('\\n')) {
    const index = input.indexOf('\\n')
    const request = JSON.parse(input.slice(0, index))
    input = input.slice(index + 1)
    if (request.type === 'get_state') {
      reply({id:request.id,type:'response',command:'get_state',success:true,data:{sessionId:'e2e-session',sessionFile:process.env.LUNA_FAKE_SESSION_FILE,isStreaming:false}})
    } else if (request.type === 'get_entries') {
      reply({id:request.id,type:'response',command:'get_entries',success:true,data:{entries:[],leafId:null}})
    } else if (request.type === 'prompt' || request.type === 'steer') {
      reply({id:request.id,type:'response',command:request.type,success:true})
      bridge.write(JSON.stringify({type:'dispatch_recorded',dispatchId}) + '\\n')
      reply({type:'agent_start'})
      reply({type:'message_update',assistantMessageEvent:{type:'text_delta',contentIndex:0,delta:'Fake response from Pi'}})
      reply({type:'agent_settled'})
    } else if (request.type === 'abort') {
      reply({id:request.id,type:'response',command:'abort',success:true})
    }
  }
})
process.stdin.on('end', () => process.exit(0))
`

test.beforeAll(async () => {
  directory = await mkdtemp(resolve(tmpdir(), 'luna-e2e-'))
  const executable = resolve(directory, 'fake-pi')
  const bridge = resolve(directory, 'bridge.ts')
  await writeFile(executable, fakePi)
  await chmod(executable, 0o700)
  await writeFile(bridge, 'export default () => {}')
  await mkdir(resolve(directory, 'data'), { recursive: true })
  server = spawn(resolve(root, 'target/debug/luna-server'), [], {
    cwd: root,
    env: {
      ...process.env,
      NO_COLOR: '1',
      LUNA_BIND_HOST: '127.0.0.1',
      LUNA_PORT: '19873',
      LUNA_DATA_DIR: resolve(directory, 'data'),
      LUNA_BRIDGE_DIR: resolve('/tmp', `luna-e2e-bridge-${String(process.pid)}`),
      LUNA_WEB_DIR: resolve(root, 'apps/web/out'),
      LUNA_PI_EXECUTABLE: executable,
      LUNA_PI_BRIDGE: bridge,
      LUNA_FAKE_SESSION_FILE: resolve(directory, 'session.jsonl'),
    },
    stdio: ['pipe', 'pipe', 'pipe'],
  })
  pairingCode = await waitForPairingCode(server)
  await waitForReady()
})

test.afterAll(async () => {
  if (server && server.exitCode === null) {
    server.kill('SIGINT')
    await Promise.race([
      new Promise<void>((resolveExit) => server.once('exit', () => resolveExit())),
      new Promise<void>((resolveTimeout) => setTimeout(resolveTimeout, 5_000)),
    ])
    if (server.exitCode === null) server.kill('SIGKILL')
  }
  if (directory) await rm(directory, { recursive: true, force: true })
  await rm(resolve('/tmp', `luna-e2e-bridge-${String(process.pid)}`), {
    recursive: true,
    force: true,
  })
})

test('pairs, streams, restores, themes, and archives a conversation', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'Pair with Luna' })).toBeVisible()
  await page.getByLabel('Pairing code').fill(pairingCode)
  await page.getByLabel('Device name').fill('Chrome acceptance test')
  await page.getByRole('button', { name: 'Pair device' }).click()

  await page.locator('button[aria-label="New conversation"]').click()
  await expect(page.getByRole('heading', { name: 'What should we work on?' })).toBeVisible()
  await page.getByPlaceholder('Message Luna…').fill('Build a Luna smoke test')
  await page.getByRole('button', { name: 'Send' }).click()
  await expect(page.getByText('Fake response from Pi')).toBeVisible()
  await expect(page.locator('.title-button strong')).toHaveText('Build a Luna smoke test')

  await page.reload()
  await expect(page.getByText('Fake response from Pi')).toBeVisible()
  await page.getByRole('button', { name: 'Toggle theme' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'mocha')
  const registrationState = await page.evaluate(async () => {
    const registration = await navigator.serviceWorker.ready
    return registration.active?.state
  })
  expect(registrationState).toBe('activated')

  page.once('dialog', (dialog) => void dialog.accept())
  await page.getByRole('button', { name: 'Archive conversation' }).click()
  await expect(page.getByText('No conversations yet.')).toBeVisible()
})

async function waitForPairingCode(process: ChildProcessWithoutNullStreams): Promise<string> {
  return new Promise((resolveCode, reject) => {
    let output = ''
    const timeout = setTimeout(() => reject(new Error(`Pairing code not found: ${output}`)), 15_000)
    const ansiEscape = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*m`, 'g')
    const inspect = (chunk: Buffer) => {
      output += chunk.toString('utf8').replaceAll(ansiEscape, '')
      const match = /pairing_code=([A-F0-9]+)/.exec(output)
      if (match?.[1]) {
        clearTimeout(timeout)
        resolveCode(match[1])
      }
    }
    process.stdout.on('data', inspect)
    process.stderr.on('data', inspect)
    process.once('exit', (code) => reject(new Error(`Luna exited before pairing: ${String(code)}`)))
  })
}

async function waitForReady(): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    const response = await fetch('http://127.0.0.1:19873/v1/health/ready').catch(() => undefined)
    if (response?.ok) return
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 100))
  }
  throw new Error('Luna did not become ready')
}
