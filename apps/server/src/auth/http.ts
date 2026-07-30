import type { FastifyReply, FastifyRequest } from 'fastify'
import type { AuthenticatedDevice, AuthService } from './service.js'

export interface AuthContext {
  authService: AuthService
  allowedTailnetLogins: string[]
  publicOrigin?: string
}

function bearerToken(request: FastifyRequest): string | undefined {
  const authorization = request.headers.authorization
  if (authorization?.startsWith('Bearer ')) return authorization.slice('Bearer '.length)
  return request.cookies.luna_device
}

function isMutation(method: string): boolean {
  return !['GET', 'HEAD', 'OPTIONS'].includes(method)
}

export function validateTailnetIdentity(
  request: FastifyRequest,
  reply: FastifyReply,
  context: AuthContext,
): boolean {
  if (context.allowedTailnetLogins.length === 0) return true
  const login = request.headers['tailscale-user-login']
  const normalized = typeof login === 'string' ? login.toLowerCase() : undefined
  if (!normalized || !context.allowedTailnetLogins.includes(normalized)) {
    void reply.code(403).send({
      code: 'forbidden',
      message: 'This Tailnet identity is not allowed to access Luna.',
      retryable: false,
    })
    return false
  }
  return true
}

export function validateOrigin(
  request: FastifyRequest,
  reply: FastifyReply,
  context: AuthContext,
): boolean {
  if (!context.publicOrigin || !isMutation(request.method)) return true
  const origin = request.headers.origin
  if (origin && origin !== context.publicOrigin) {
    void reply.code(403).send({
      code: 'forbidden',
      message: 'The request origin is not allowed.',
      retryable: false,
    })
    return false
  }
  return true
}

export function authenticateRequest(
  request: FastifyRequest,
  reply: FastifyReply,
  context: AuthContext,
): AuthenticatedDevice | undefined {
  if (
    !validateTailnetIdentity(request, reply, context) ||
    !validateOrigin(request, reply, context)
  ) {
    return undefined
  }
  const device = context.authService.authenticate(bearerToken(request))
  if (!device) {
    void reply.code(401).send({
      code: 'authentication_required',
      message: 'Pair this device with Luna before continuing.',
      retryable: false,
    })
  }
  return device
}
