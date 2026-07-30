import { resolve } from 'node:path'
import Fastify, { type FastifyInstance } from 'fastify'
import cookie from '@fastify/cookie'
import { Value } from 'typebox/value'
import {
  PairingExchangeRequestSchema,
  type Bootstrap,
  type PairingExchangeRequest,
} from '@luna/protocol'
import {
  AuthService,
  authenticateRequest,
  validateOrigin,
  validateTailnetIdentity,
} from './auth/index.js'
import type { LunaConfig } from './config.js'
import { createDatabase } from './db/database.js'

export interface LunaAppContext {
  config: LunaConfig
  authService: AuthService
  pairingCode: string
}

export async function buildApp(
  config: LunaConfig,
): Promise<FastifyInstance & { luna: LunaAppContext }> {
  const app = Fastify({ logger: true }) as unknown as FastifyInstance & { luna: LunaAppContext }
  await app.register(cookie)

  const database = createDatabase(config.databasePath, resolve(import.meta.dirname, '../drizzle'))
  const authService = new AuthService(database.db)
  const pairingCode = authService.createPairingCode()
  app.decorate('luna', { config, authService, pairingCode })

  app.addHook('onClose', () => database.close())

  app.get('/v1/health/live', () => ({ status: 'ok' }))

  app.post('/v1/pairing/exchange', async (request, reply) => {
    const authContext = {
      authService,
      allowedTailnetLogins: config.allowedTailnetLogins,
      ...(config.publicOrigin ? { publicOrigin: config.publicOrigin } : {}),
    }
    if (
      !validateTailnetIdentity(request, reply, authContext) ||
      !validateOrigin(request, reply, authContext)
    )
      return
    if (!Value.Check(PairingExchangeRequestSchema, request.body)) {
      return reply.code(400).send({
        code: 'invalid_request',
        message: 'The pairing request is invalid.',
        retryable: false,
      })
    }
    const body: PairingExchangeRequest = request.body
    const result = authService.exchangePairingCode(body.code, body.deviceName, body.platform)
    if (!result) {
      return reply.code(401).send({
        code: 'authentication_required',
        message: 'The pairing code is invalid or expired.',
        retryable: false,
      })
    }
    const bootstrap: Bootstrap = {
      protocolVersion: 1,
      cursor: 0,
      device: result.device,
      conversations: [],
    }
    if (result.device.platform === 'web') {
      reply.setCookie('luna_device', result.token, {
        httpOnly: true,
        secure: config.publicOrigin?.startsWith('https://') ?? false,
        sameSite: 'strict',
        path: '/',
      })
    }
    return reply.code(201).send({ deviceId: result.device.id, token: result.token, bootstrap })
  })

  app.get('/v1/bootstrap', async (request, reply) => {
    const device = authenticateRequest(request, reply, {
      authService,
      allowedTailnetLogins: config.allowedTailnetLogins,
      ...(config.publicOrigin ? { publicOrigin: config.publicOrigin } : {}),
    })
    if (!device) return
    const bootstrap: Bootstrap = {
      protocolVersion: 1,
      cursor: 0,
      device,
      conversations: [],
    }
    return bootstrap
  })

  return app
}
