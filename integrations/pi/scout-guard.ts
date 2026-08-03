import type { ExtensionAPI } from '@earendil-works/pi-coding-agent'
import { isAllowedScoutTool, isScoutPathAllowed } from './scout-security.js'

export default function scoutGuard(pi: ExtensionAPI) {
  const root = process.env.LUNA_SCOUT_ROOT
  if (!root) throw new Error('LUNA_SCOUT_ROOT is required')

  pi.on('tool_call', (event, ctx) => {
    if (!isAllowedScoutTool(event.toolName)) {
      return { block: true, reason: `Scout tool is not allowed: ${event.toolName}` }
    }
    const input = event.input as { path?: unknown; file_path?: unknown }
    const requestedPath = input.path ?? input.file_path ?? '.'
    if (typeof requestedPath !== 'string' || !isScoutPathAllowed(root, ctx.cwd, requestedPath)) {
      return { block: true, reason: 'Scout paths must remain inside the delegated repository' }
    }
  })
}
