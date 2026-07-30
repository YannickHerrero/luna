import { createHash, randomBytes, randomUUID } from 'node:crypto'
import { and, eq, isNull } from 'drizzle-orm'
import type { DevicePlatform } from '@luna/protocol'
import type { LunaDatabase } from '../db/database.js'
import { devices, pairingCodes } from '../db/schema.js'

const hash = (value: string) => createHash('sha256').update(value).digest('hex')
const now = () => new Date().toISOString()

export interface AuthenticatedDevice {
  id: string
  name: string
  platform: DevicePlatform
  notificationsEnabled: boolean
  createdAt: string
  lastSeenAt: string
}

export class AuthService {
  constructor(private readonly db: LunaDatabase) {}

  createPairingCode(lifetimeMilliseconds = 15 * 60 * 1000): string {
    const code = randomBytes(5).toString('hex').toUpperCase()
    const createdAt = now()
    this.db
      .insert(pairingCodes)
      .values({
        id: randomUUID(),
        codeHash: hash(code),
        createdAt,
        expiresAt: new Date(Date.now() + lifetimeMilliseconds).toISOString(),
      })
      .run()
    return code
  }

  exchangePairingCode(code: string, name: string, platform: DevicePlatform) {
    const codeHash = hash(code.trim().toUpperCase())
    const createdAt = now()
    const pairing = this.db
      .select()
      .from(pairingCodes)
      .where(and(eq(pairingCodes.codeHash, codeHash), isNull(pairingCodes.redeemedAt)))
      .get()
    if (!pairing || pairing.expiresAt <= createdAt) return undefined

    const token = randomBytes(32).toString('base64url')
    const device = {
      id: randomUUID(),
      name,
      platform,
      credentialHash: hash(token),
      notificationsEnabled: false,
      createdAt,
      lastSeenAt: createdAt,
    } as const

    this.db.transaction((transaction) => {
      const claimed = transaction
        .update(pairingCodes)
        .set({ redeemedAt: createdAt })
        .where(and(eq(pairingCodes.id, pairing.id), isNull(pairingCodes.redeemedAt)))
        .run()
      if (claimed.changes !== 1) throw new Error('Pairing code was already redeemed')
      transaction.insert(devices).values(device).run()
    })

    return { token, device: this.toAuthenticatedDevice(device) }
  }

  authenticate(token: string | undefined): AuthenticatedDevice | undefined {
    if (!token) return undefined
    const row = this.db
      .select()
      .from(devices)
      .where(and(eq(devices.credentialHash, hash(token)), isNull(devices.revokedAt)))
      .get()
    if (!row) return undefined
    const lastSeenAt = now()
    this.db.update(devices).set({ lastSeenAt }).where(eq(devices.id, row.id)).run()
    return this.toAuthenticatedDevice({ ...row, lastSeenAt })
  }

  private toAuthenticatedDevice(row: {
    id: string
    name: string
    platform: DevicePlatform
    notificationsEnabled: boolean
    createdAt: string
    lastSeenAt: string
  }): AuthenticatedDevice {
    return {
      id: row.id,
      name: row.name,
      platform: row.platform,
      notificationsEnabled: row.notificationsEnabled,
      createdAt: row.createdAt,
      lastSeenAt: row.lastSeenAt,
    }
  }
}
