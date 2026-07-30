import { mkdirSync } from 'node:fs'
import { dirname } from 'node:path'
import Database from 'better-sqlite3'
import { drizzle } from 'drizzle-orm/better-sqlite3'
import { migrate } from 'drizzle-orm/better-sqlite3/migrator'
import * as schema from './schema.js'

export type LunaDatabase = ReturnType<typeof createDatabase>['db']

export function createDatabase(databasePath: string, migrationsFolder?: string) {
  mkdirSync(dirname(databasePath), { recursive: true, mode: 0o700 })
  const sqlite = new Database(databasePath)
  sqlite.pragma('journal_mode = WAL')
  sqlite.pragma('foreign_keys = ON')
  sqlite.pragma('busy_timeout = 5000')
  const db = drizzle(sqlite, { schema })
  if (migrationsFolder) migrate(db, { migrationsFolder })
  return {
    db,
    sqlite,
    close: () => sqlite.close(),
  }
}
