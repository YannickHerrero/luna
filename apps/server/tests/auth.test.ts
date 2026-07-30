import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { buildApp } from '../src/app.js'
import type { LunaConfig } from '../src/config.js'

const directories: string[] = []
afterEach(() => {
  for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true })
})

function testConfig(): LunaConfig {
  const directory = mkdtempSync(resolve(tmpdir(), 'luna-auth-'))
  directories.push(directory)
  return {
    bindHost: '127.0.0.1',
    port: 9870,
    dataDirectory: directory,
    credentialsDirectory: directory,
    databasePath: resolve(directory, 'luna.sqlite'),
    piSessionDirectory: resolve(directory, 'pi-sessions'),
    attachmentDirectory: resolve(directory, 'attachments'),
    openAiTranscriptionModel: 'gpt-4o-mini-transcribe',
    eventRetentionDays: 30,
    allowedTailnetLogins: [],
  }
}

describe('device authentication', () => {
  it('exchanges one pairing code and authenticates the new device', async () => {
    const app = await buildApp(testConfig())
    const response = await app.inject({
      method: 'POST',
      url: '/v1/pairing/exchange',
      payload: {
        code: app.luna.pairingCode,
        deviceName: 'Test iPhone',
        platform: 'ios',
      },
    })
    expect(response.statusCode).toBe(201)
    const token = response.json<{ token: string }>().token

    const bootstrap = await app.inject({
      method: 'GET',
      url: '/v1/bootstrap',
      headers: { authorization: `Bearer ${token}` },
    })
    expect(bootstrap.statusCode).toBe(200)
    expect(bootstrap.json<{ device: { name: string } }>().device.name).toBe('Test iPhone')
    await app.close()
  })

  it('rejects reuse of a pairing code', async () => {
    const app = await buildApp(testConfig())
    const payload = { code: app.luna.pairingCode, deviceName: 'Browser', platform: 'web' }
    expect(
      (await app.inject({ method: 'POST', url: '/v1/pairing/exchange', payload })).statusCode,
    ).toBe(201)
    expect(
      (await app.inject({ method: 'POST', url: '/v1/pairing/exchange', payload })).statusCode,
    ).toBe(401)
    await app.close()
  })
})
