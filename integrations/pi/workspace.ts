import { isAbsolute, resolve } from 'node:path'

const PATH_TOOLS = new Set(['read', 'edit', 'write', 'grep', 'find', 'ls'])

export function resolveToolPath(workingDirectory: string, value: string): string {
  return isAbsolute(value) ? value : resolve(workingDirectory, value)
}

export function rewriteToolPath(
  toolName: string,
  input: Record<string, unknown>,
  workingDirectory: string,
): string | undefined {
  if (!PATH_TOOLS.has(toolName)) return undefined
  const path = typeof input.path === 'string' ? input.path : '.'
  const resolved = resolveToolPath(workingDirectory, path)
  input.path = resolved
  return resolved
}

export function quoteForShell(value: string): string {
  return `'${value.replaceAll("'", `'"'"'`)}'`
}

export function instrumentBash(
  command: string,
  reportPath: string,
  workingDirectory: string,
): string {
  return `( cd ${quoteForShell(workingDirectory)}
trap 'pwd > ${quoteForShell(reportPath)}' EXIT
${command}
)`
}
