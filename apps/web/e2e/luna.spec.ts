import { AxeBuilder } from '@axe-core/playwright'
import { expect, test } from '@playwright/test'
import { chmod, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'

const root = resolve(import.meta.dirname, '../../..')
let server: ChildProcessWithoutNullStreams
let directory: string
let pairingCode: string
let serverOutput = ''

const fakePi = `#!/usr/bin/env node
if (process.argv.includes('--print')) {
  require('node:fs').readFileSync(0, 'utf8')
  console.log('Luna Browser Acceptance')
  process.exit(0)
}
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
    } else if (request.type === 'prompt') {
      const steering = request.streamingBehavior === 'steer'
      reply({id:request.id,type:'response',command:request.type,success:true})
      bridge.write(JSON.stringify({type:'dispatch_recorded',dispatchId}) + '\\n')
      if (!steering) reply({type:'agent_start'})
      const activities = steering
        ? ['Refining browser acceptance after steering']
        : ['Planning Luna smoke-test coverage', 'Validating the browser workflow']
      for (const activity of activities) {
        reply({type:'message_update',assistantMessageEvent:{type:'thinking_start',contentIndex:0}})
        reply({type:'message_update',assistantMessageEvent:{type:'thinking_delta',contentIndex:0,delta:'**' + activity + '**'}})
        reply({type:'message_update',assistantMessageEvent:{type:'thinking_end',contentIndex:0}})
      }
      reply({type:'message_update',assistantMessageEvent:{type:'text_delta',contentIndex:0,delta:steering ? ' after steering' : 'Working response from Pi'}})
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
      LUNA_PUBLIC_ORIGIN: 'http://127.0.0.1:19873',
      LUNA_ALLOWED_TAILNET_LOGINS: '',
      LUNA_TRANSCRIPTION_API_KEY: '',
      LUNA_DATA_DIR: resolve(directory, 'data'),
      LUNA_ENV_FILE: resolve(directory, 'test.env'),
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
    server.kill('SIGTERM')
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
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'Pair with Luna' })).toBeVisible()

  const viewport = await page.locator('meta[name="viewport"]').getAttribute('content')
  expect(viewport).toContain('maximum-scale=1')
  expect(viewport).toContain('user-scalable=no')
  const rootPresentation = await page.evaluate(() => {
    const style = window.getComputedStyle(document.documentElement)
    return {
      textSizeAdjust:
        style.getPropertyValue('text-size-adjust') ||
        style.getPropertyValue('-webkit-text-size-adjust'),
      touchAction: style.touchAction,
    }
  })
  expect(rootPresentation.textSizeAdjust).toBe('100%')
  expect(rootPresentation.touchAction).toContain('pan-x')
  expect(rootPresentation.touchAction).toContain('pan-y')
  for (const label of ['Pairing code', 'Device name']) {
    expect(
      await page
        .getByLabel(label)
        .evaluate((element) => Number.parseFloat(window.getComputedStyle(element).fontSize)),
    ).toBeGreaterThanOrEqual(16)
  }

  const startupCode = pairingCode
  await page.getByRole('button', { name: 'Ask for a pairing code' }).click()
  await expect(page.getByRole('status')).toContainText('written to Luna’s Citadel logs')
  pairingCode = latestPairingCode()
  expect(pairingCode).not.toBe(startupCode)

  await page.getByLabel('Pairing code').fill(startupCode)
  await page.getByLabel('Device name').fill('Chrome acceptance test')
  await page.getByRole('button', { name: 'Pair device' }).click()
  await expect(page.getByText(/invalid, expired, or already used/)).toBeVisible()
  await page.getByLabel('Pairing code').fill(pairingCode)
  await page.getByRole('button', { name: 'Pair device' }).click()

  const search = page.getByLabel('Search conversations')
  await expect(search).toBeVisible()
  expect(
    await search.evaluate((element) =>
      Number.parseFloat(window.getComputedStyle(element).fontSize),
    ),
  ).toBeGreaterThanOrEqual(16)
  await page.locator('button[aria-label="New conversation"]').click()
  await expect(page.getByRole('heading', { name: 'What should we work on?' })).toBeVisible()

  const prompt = page.getByLabel('Message Luna')
  await prompt.fill('Draft for the first conversation')
  await page.getByRole('button', { name: 'Back' }).click()
  await page.locator('button[aria-label="New conversation"]').click()
  await expect(prompt).toHaveValue('')
  await prompt.fill('Draft for the second conversation')
  await page.getByRole('button', { name: 'Back' }).click()
  await page.locator('.conversation-cell').nth(1).click()
  await expect(prompt).toHaveValue('Draft for the first conversation')
  await page.reload()
  await expect(prompt).toHaveValue('Draft for the second conversation')
  await page.getByRole('button', { name: 'Back' }).click()
  await expect(page.locator('.conversation-cell')).toHaveCount(2)
  await page.locator('.conversation-cell').nth(1).click()
  await expect(prompt).toHaveValue('Draft for the first conversation')
  await prompt.fill('')

  const composer = page.locator('.composer')
  const composerBounds = await composer.boundingBox()
  expect(composerBounds).not.toBeNull()
  expect(composerBounds?.x).toBeGreaterThanOrEqual(16)
  expect(375 - (composerBounds?.x ?? 0) - (composerBounds?.width ?? 0)).toBeGreaterThanOrEqual(16)

  const singleLineHeight = await prompt.evaluate((element) => element.clientHeight)
  await prompt.fill(
    Array.from({ length: 10 }, (_, index) => `Prompt line ${String(index + 1)}`).join('\n'),
  )
  await expect
    .poll(() => prompt.evaluate((element) => element.clientHeight))
    .toBeGreaterThan(singleLineHeight)
  const expandedPrompt = await prompt.evaluate((element) => {
    const style = window.getComputedStyle(element)
    return {
      overflowY: style.overflowY,
      scrollHeight: element.scrollHeight,
      clientHeight: element.clientHeight,
      visibleLines:
        (element.clientHeight -
          Number.parseFloat(style.paddingTop) -
          Number.parseFloat(style.paddingBottom)) /
        Number.parseFloat(style.lineHeight),
    }
  })
  expect(expandedPrompt.visibleLines).toBeGreaterThanOrEqual(8)
  expect(expandedPrompt.visibleLines).toBeLessThan(8.1)
  expect(expandedPrompt.scrollHeight).toBeGreaterThan(expandedPrompt.clientHeight)
  expect(expandedPrompt.overflowY).toBe('auto')
  await prompt.fill('')
  await expect.poll(() => prompt.evaluate((element) => element.clientHeight)).toBe(singleLineHeight)

  await page.locator('input[type="file"][multiple]').setInputFiles({
    name: 'pixel.png',
    mimeType: 'image/png',
    buffer: Buffer.from(
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
      'base64',
    ),
  })
  await expect(page.getByAltText('Pending attachment')).toBeVisible()
  await page.getByPlaceholder('Message Luna…').fill('Build a Luna smoke test')
  await page.getByRole('button', { name: 'Send' }).click()
  await expect(page.getByAltText('pixel.png')).toBeVisible()
  await expect(page.getByText('Working response from Pi')).toBeVisible()
  const firstUserMessage = page.locator('.message-row.user').first()
  await firstUserMessage.click()
  const messageTimestamp = firstUserMessage.locator('.message-timestamp')
  await expect(messageTimestamp).toBeVisible()
  await expect(messageTimestamp).toHaveAttribute('datetime', /T/)
  await firstUserMessage.click()
  await expect(messageTimestamp).toHaveCount(0)
  await firstUserMessage.focus()
  await page.keyboard.press('Enter')
  await expect(messageTimestamp).toBeVisible()
  await page.keyboard.press('Space')
  await expect(messageTimestamp).toHaveCount(0)
  await expect(page.locator('.activity-details summary')).toContainText(
    'Validating the browser workflow',
  )
  await page.locator('.activity-details summary').click()
  await expect(page.locator('.activity-details')).toContainText('Planning Luna smoke-test coverage')
  const mobileLayout = await page.evaluate(() => ({
    clientWidth: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
    messageBounds: Array.from(document.querySelectorAll('.message-bubble')).map((element) => {
      const bounds = element.getBoundingClientRect()
      return { left: bounds.left, right: bounds.right }
    }),
  }))
  expect(mobileLayout.scrollWidth).toBeLessThanOrEqual(mobileLayout.clientWidth)
  for (const bounds of mobileLayout.messageBounds) {
    expect(bounds.left).toBeGreaterThanOrEqual(0)
    expect(bounds.right).toBeLessThanOrEqual(mobileLayout.clientWidth)
  }
  await page.getByRole('button', { name: 'Back' }).click()
  const sortedConversations = page.locator('.conversation-cell')
  await expect(sortedConversations).toHaveCount(2)
  await expect(sortedConversations.first().locator('.state-dot.working')).toBeVisible()
  await expect(sortedConversations.first().locator('.cell-time')).toHaveAttribute('datetime', /T/)
  await sortedConversations.first().click()
  await expect(page.getByText('Working response from Pi')).toBeVisible()
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.getByPlaceholder('Steer Pi…').fill('Focus on browser acceptance')
  await page.getByRole('button', { name: 'Send' }).click()
  await expect(page.getByText('Working response from Pi after steering')).toBeVisible()
  await expect(page.locator('.activity-details summary')).toContainText(
    'Refining browser acceptance after steering',
  )
  const transcript = page.locator('.message-row')
  await expect(transcript.nth(0)).toContainText('Build a Luna smoke test')
  await expect(transcript.nth(1)).toContainText('Working response from Pi after steering')
  await expect(transcript.nth(2)).toContainText('Focus on browser acceptance')
  await page.getByRole('button', { name: 'Interrupt Pi' }).click()
  await expect(page.locator('.status-pill')).toContainText('Interrupted')

  page.once('dialog', (dialog) => void dialog.accept('Luna acceptance'))
  await page.getByTitle('Rename conversation').click()
  await expect(page.locator('.title-button strong')).toHaveText('Luna acceptance')

  await page.reload()
  await expect(page.getByText('Working response from Pi after steering')).toBeVisible()
  await expect(page.locator('.title-button strong')).toHaveText('Luna acceptance')
  await page.keyboard.press('Escape')
  await expect(
    page.getByRole('heading', { name: 'Powerful agents. Familiar conversations.' }),
  ).toBeVisible()
  await page.getByRole('button', { name: /Luna acceptance/ }).click()
  await page.getByRole('button', { name: 'Toggle theme' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'mocha')
  await page.locator('.conversation-cell.selected').evaluate(async (element) => {
    await Promise.all(element.getAnimations().map(async (animation) => animation.finished))
  })
  const registrationState = await page.evaluate(async () => {
    const registration = await navigator.serviceWorker.ready
    return registration.active?.state
  })
  expect(registrationState).toBe('activated')
  const accessibility = await new AxeBuilder({ page }).analyze()
  expect(
    accessibility.violations.filter((violation) =>
      ['serious', 'critical'].includes(violation.impact ?? ''),
    ),
  ).toEqual([])

  page.once('dialog', (dialog) => void dialog.accept())
  await page.getByRole('button', { name: 'Archive conversation' }).click()
  await expect(page.locator('.conversation-cell')).toHaveCount(1)
  await page.locator('.conversation-cell').click()
  page.once('dialog', (dialog) => void dialog.accept())
  await page.getByRole('button', { name: 'Archive conversation' }).click()
  await expect(page.getByText('No conversations yet.')).toBeVisible()
})

async function waitForPairingCode(process: ChildProcessWithoutNullStreams): Promise<string> {
  return new Promise((resolveCode, reject) => {
    const timeout = setTimeout(
      () => reject(new Error(`Pairing code not found: ${serverOutput}`)),
      15_000,
    )
    const ansiEscape = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*m`, 'g')
    const inspect = (chunk: Buffer) => {
      serverOutput += chunk.toString('utf8').replaceAll(ansiEscape, '')
      const match = /pairing_code=([0-9]{6})\b/.exec(serverOutput)
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

function latestPairingCode(): string {
  const matches = [...serverOutput.matchAll(/pairing_code=([0-9]{6})\b/g)]
  const code = matches.at(-1)?.[1]
  if (!code) throw new Error('No pairing code in Luna output')
  return code
}

async function waitForReady(): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    const response = await fetch('http://127.0.0.1:19873/v1/health/ready').catch(() => undefined)
    if (response?.ok) return
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 100))
  }
  throw new Error('Luna did not become ready')
}
