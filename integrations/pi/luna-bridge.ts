import { readFile, rm } from 'node:fs/promises'
import { connect, type Socket } from 'node:net'
import { homedir, tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { isToolCallEventType, type ExtensionAPI } from '@earendil-works/pi-coding-agent'
import { registerPlanProgress, type PlanProgress } from './plan-progress.js'
import { registerReadOnlyScout } from './scout.js'
import { instrumentBash, rewriteToolPath } from './workspace.js'

type BridgeCommand =
  | {
      type: 'dispatch'
      dispatchId: string
    }
  | {
      type: 'cancel_dispatch'
      dispatchId: string
    }

type WorkspaceEntry = {
  cwd: string
}

export default function lunaBridge(pi: ExtensionAPI) {
  registerReadOnlyScout(pi)
  const socketPath = process.env.LUNA_BRIDGE_SOCKET
  let socket: Socket | undefined
  let reconnectTimer: NodeJS.Timeout | undefined
  let connected = false
  let inputBuffer = ''
  let outgoing: string[] = []
  let stopped = false
  let workingDirectory = resolve(process.env.LUNA_WORKING_DIRECTORY ?? homedir())
  const pendingDispatches: string[] = []
  const bashReports = new Map<string, string>()

  const emit = (message: Record<string, unknown>) => {
    const line = `${JSON.stringify(message)}\n`
    if (socket?.writable && connected) socket.write(line)
    else outgoing.push(line)
  }

  let latestTaskList: PlanProgress | undefined
  const emitTaskList = () =>
    emit(
      latestTaskList
        ? { type: 'task_list_updated', taskList: latestTaskList }
        : { type: 'task_list_cleared' },
    )
  registerPlanProgress(pi, {
    onChanged: (taskList) => {
      latestTaskList = taskList
      emitTaskList()
    },
  })

  const handleCommand = (command: BridgeCommand) => {
    if (!command.dispatchId) return
    if (command.type === 'cancel_dispatch') {
      const index = pendingDispatches.indexOf(command.dispatchId)
      if (index >= 0) pendingDispatches.splice(index, 1)
      return
    }
    emitTaskList()
    pendingDispatches.push(command.dispatchId)
    emit({ type: 'dispatch_ready', dispatchId: command.dispatchId })
  }

  const scheduleReconnect = () => {
    if (stopped || reconnectTimer || !socketPath) return
    reconnectTimer = setTimeout(() => {
      reconnectTimer = undefined
      openSocket()
    }, 250)
    reconnectTimer.unref()
  }

  const openSocket = () => {
    if (!socketPath || stopped) return
    const next = connect(socketPath)
    socket = next
    next.setEncoding('utf8')
    next.on('connect', () => {
      connected = true
      next.write(`${JSON.stringify({ type: 'ready', pid: process.pid, cwd: workingDirectory })}\n`)
      for (const line of outgoing) next.write(line)
      outgoing = []
    })
    next.on('data', (chunk: string) => {
      inputBuffer += chunk
      while (inputBuffer.includes('\n')) {
        const index = inputBuffer.indexOf('\n')
        const line = inputBuffer.slice(0, index).trimEnd()
        inputBuffer = inputBuffer.slice(index + 1)
        if (!line) continue
        try {
          handleCommand(JSON.parse(line) as BridgeCommand)
        } catch (error) {
          emit({ type: 'bridge_error', message: String(error) })
        }
      }
    })
    next.on('error', () => next.destroy())
    next.on('close', () => {
      connected = false
      scheduleReconnect()
    })
    next.unref()
  }

  pi.on('session_start', (_event, ctx) => {
    const restored = [...ctx.sessionManager.getBranch()]
      .reverse()
      .find(
        (entry) =>
          entry.type === 'custom' &&
          entry.customType === 'luna-workspace' &&
          typeof (entry.data as WorkspaceEntry | undefined)?.cwd === 'string',
      )
    if (restored?.type === 'custom') {
      workingDirectory = resolve((restored.data as WorkspaceEntry).cwd)
    }
    openSocket()
    emit({ type: 'workspace', cwd: workingDirectory, restored: Boolean(restored) })
  })

  pi.on('input', (event) => {
    if (event.source !== 'rpc') return
    const dispatchId = pendingDispatches.shift()
    if (!dispatchId) {
      emit({ type: 'bridge_error', message: 'RPC input arrived without a dispatch marker' })
      return
    }
    pi.appendEntry('luna-dispatch', { dispatchId })
    emit({ type: 'dispatch_recorded', dispatchId })
  })

  pi.on('tool_call', (event) => {
    if (isToolCallEventType('bash', event)) {
      const reportPath = join(tmpdir(), `luna-cwd-${process.pid}-${event.toolCallId}`)
      event.input.command = instrumentBash(event.input.command, reportPath, workingDirectory)
      bashReports.set(event.toolCallId, reportPath)
      return
    }
    const path = rewriteToolPath(event.toolName, event.input, workingDirectory)
    if (path) emit({ type: 'path_observed', path, toolName: event.toolName })
  })

  pi.on('tool_result', async (event) => {
    const reportPath = bashReports.get(event.toolCallId)
    if (!reportPath) return
    bashReports.delete(event.toolCallId)
    try {
      const next = (await readFile(reportPath, 'utf8')).trim()
      if (next && resolve(next) !== workingDirectory) {
        workingDirectory = resolve(next)
        pi.appendEntry('luna-workspace', { cwd: workingDirectory })
        emit({ type: 'workspace', cwd: workingDirectory, restored: false })
      }
    } catch {
      emit({ type: 'bridge_error', message: 'Unable to observe the bash working directory' })
    } finally {
      await rm(reportPath, { force: true })
    }
  })

  pi.on('session_shutdown', () => {
    stopped = true
    if (reconnectTimer) clearTimeout(reconnectTimer)
    socket?.destroy()
  })
}
