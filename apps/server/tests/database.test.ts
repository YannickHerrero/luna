import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { createDatabase } from '../src/db/database.js'
import { conversations } from '../src/db/schema.js'

const directories: string[] = []
afterEach(() => {
  for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true })
})

describe('database', () => {
  it('migrates a new SQLite database and enforces conversation ids', () => {
    const directory = mkdtempSync(resolve(tmpdir(), 'luna-db-'))
    directories.push(directory)
    const database = createDatabase(
      resolve(directory, 'luna.sqlite'),
      resolve(import.meta.dirname, '../drizzle'),
    )
    const now = new Date().toISOString()

    database.db
      .insert(conversations)
      .values({
        id: crypto.randomUUID(),
        activeWorkingDirectory: '/Users/test',
        createdAt: now,
        updatedAt: now,
      })
      .run()

    expect(database.db.select().from(conversations).all()).toHaveLength(1)
    database.close()
  })
})
