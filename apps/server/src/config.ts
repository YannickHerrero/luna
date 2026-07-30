import { existsSync, readFileSync, statSync } from 'node:fs'
import { homedir, platform } from 'node:os'
import { dirname, resolve } from 'node:path'
import { config as loadEnv } from 'dotenv'

export interface LunaConfig {
  bindHost: string
  port: number
  publicOrigin?: string
  dataDirectory: string
  credentialsDirectory: string
  databasePath: string
  piSessionDirectory: string
  attachmentDirectory: string
  openAiTranscriptionModel: string
  eventRetentionDays: number
}

interface LocalConfig {
  bindHost?: string
  port?: number
  publicOrigin?: string
  dataDirectory?: string
}

function expandHome(value: string): string {
  return value === '~' || value.startsWith('~/')
    ? resolve(homedir(), value.slice(2))
    : resolve(value)
}

function defaultDataDirectory(): string {
  if (platform() === 'darwin') return resolve(homedir(), 'Library/Application Support/Luna Server')
  return resolve(process.env.XDG_DATA_HOME ?? resolve(homedir(), '.local/share'), 'luna')
}

function readLocalConfig(): LocalConfig {
  const configuredPath = process.env.LUNA_LOCAL_CONFIG
  const path = configuredPath
    ? expandHome(configuredPath)
    : resolve(process.cwd(), '.luna.local.json')
  if (!existsSync(path)) return {}
  return JSON.parse(readFileSync(path, 'utf8')) as LocalConfig
}

function assertPrivateFile(path: string): void {
  if (platform() === 'win32') return
  const mode = statSync(path).mode & 0o777
  if ((mode & 0o077) !== 0) throw new Error(`Luna environment file must be mode 600: ${path}`)
}

function loadExternalEnvironment(): void {
  const path = expandHome(process.env.LUNA_ENV_FILE ?? '~/.config/luna/server.env')
  if (!existsSync(path)) return
  assertPrivateFile(path)
  loadEnv({ path, quiet: true })
}

function parsePort(value: string | number | undefined): number {
  const parsed = Number(value ?? 9870)
  if (!Number.isInteger(parsed) || parsed < 1 || parsed > 65_535) {
    throw new Error('LUNA_PORT must be an integer between 1 and 65535')
  }
  return parsed
}

function parsePositiveInteger(value: string | undefined, fallback: number): number {
  if (!value) return fallback
  const parsed = Number(value)
  if (!Number.isInteger(parsed) || parsed < 1) throw new Error('Expected a positive integer')
  return parsed
}

export function loadConfig(): LunaConfig {
  loadExternalEnvironment()
  const local = readLocalConfig()
  const dataDirectory = expandHome(
    process.env.LUNA_DATA_DIR ?? local.dataDirectory ?? defaultDataDirectory(),
  )
  const credentialsDirectory = expandHome(process.env.LUNA_CREDENTIALS_DIR ?? '~/.config/luna')
  const publicOrigin = process.env.LUNA_PUBLIC_ORIGIN ?? local.publicOrigin

  return {
    bindHost: process.env.LUNA_BIND_HOST ?? local.bindHost ?? '127.0.0.1',
    port: parsePort(process.env.LUNA_PORT ?? local.port),
    ...(publicOrigin ? { publicOrigin } : {}),
    dataDirectory,
    credentialsDirectory,
    databasePath: resolve(dataDirectory, 'luna.sqlite'),
    piSessionDirectory: resolve(dataDirectory, 'pi-sessions'),
    attachmentDirectory: resolve(dataDirectory, 'attachments'),
    openAiTranscriptionModel: process.env.LUNA_TRANSCRIPTION_MODEL ?? 'gpt-4o-mini-transcribe',
    eventRetentionDays: parsePositiveInteger(process.env.LUNA_EVENT_RETENTION_DAYS, 30),
  }
}

export function configDirectoryFor(path: string): string {
  return dirname(path)
}
