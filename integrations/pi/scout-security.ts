import { existsSync, realpathSync } from 'node:fs'
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from 'node:path'

const ALLOWED_TOOLS = new Set(['read', 'grep', 'find', 'ls'])

export function isAllowedScoutTool(toolName: string): boolean {
  return ALLOWED_TOOLS.has(toolName)
}

export function findScoutGitRoot(cwd: string): string | undefined {
  let current = realpathSync(cwd)
  while (true) {
    if (existsSync(join(current, '.git'))) return current
    const parent = dirname(current)
    if (parent === current) return undefined
    current = parent
  }
}

export function isScoutPathAllowed(root: string, cwd: string, inputPath: string): boolean {
  const canonicalRoot = realpathSync(root)
  const normalizedInput = inputPath.startsWith('@') ? inputPath.slice(1) : inputPath
  const candidate = isAbsolute(normalizedInput)
    ? resolve(normalizedInput)
    : resolve(cwd, normalizedInput)
  const canonicalCandidate = canonicalizeCandidate(candidate)
  const pathFromRoot = relative(canonicalRoot, canonicalCandidate)
  return pathFromRoot === '' || (!pathFromRoot.startsWith(`..${sep}`) && pathFromRoot !== '..')
}

function canonicalizeCandidate(candidate: string): string {
  let existing = candidate
  const missing: string[] = []
  while (!existsSync(existing)) {
    const parent = dirname(existing)
    if (parent === existing) break
    missing.unshift(basename(existing))
    existing = parent
  }
  return resolve(realpathSync(existing), ...missing)
}

const ENVIRONMENT_NAMES = new Set([
  'HOME',
  'LANG',
  'LC_ALL',
  'LOGNAME',
  'NODE_EXTRA_CA_CERTS',
  'NO_PROXY',
  'PATH',
  'PI_CODING_AGENT_DIR',
  'SHELL',
  'SSL_CERT_DIR',
  'SSL_CERT_FILE',
  'TMPDIR',
  'USER',
  'http_proxy',
  'https_proxy',
  'no_proxy',
  'HTTP_PROXY',
  'HTTPS_PROXY',
])

const PROVIDER_CREDENTIALS = new Set([
  'ANTHROPIC_API_KEY',
  'AZURE_OPENAI_API_KEY',
  'CEREBRAS_API_KEY',
  'CLOUDFLARE_API_KEY',
  'DEEPSEEK_API_KEY',
  'FIREWORKS_API_KEY',
  'GEMINI_API_KEY',
  'GOOGLE_API_KEY',
  'GROQ_API_KEY',
  'HUGGINGFACE_API_KEY',
  'MISTRAL_API_KEY',
  'OPENAI_API_KEY',
  'OPENROUTER_API_KEY',
  'TOGETHER_API_KEY',
  'XAI_API_KEY',
  'ZAI_API_KEY',
])

export function scoutEnvironment(source: NodeJS.ProcessEnv, root: string): NodeJS.ProcessEnv {
  const environment: NodeJS.ProcessEnv = {
    LUNA_SCOUT_ROOT: realpathSync(root),
    PI_SKIP_VERSION_CHECK: '1',
    PI_TELEMETRY: '0',
  }
  for (const name of [...ENVIRONMENT_NAMES, ...PROVIDER_CREDENTIALS]) {
    if (source[name] !== undefined) environment[name] = source[name]
  }
  return environment
}
