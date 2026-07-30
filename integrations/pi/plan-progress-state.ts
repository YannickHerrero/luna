export const LUNA_PLAN_TOOL = 'luna_plan'

export const MAX_PLAN_TASKS = 30
const MAX_TASK_TEXT_LENGTH = 240
const MAX_NOTE_LENGTH = 500

export type PlanAction = 'replace' | 'start' | 'complete' | 'block' | 'skip' | 'show' | 'clear'
export type PlanTaskStatus = 'pending' | 'in_progress' | 'completed' | 'blocked' | 'skipped'

export type PlanTask = {
  id: string
  sequence: number
  text: string
  status: PlanTaskStatus
  note?: string
  createdAt: string
  updatedAt: string
}

export type PlanProgress = {
  id: string
  title?: string
  revision: number
  tasks: PlanTask[]
  createdAt: string
  updatedAt: string
}

export type PlanToolDetails = {
  action: PlanAction
  plan: PlanProgress | null
}

export type PlanToolInput = {
  action: PlanAction
  title?: string
  tasks?: string[]
  step?: number
  note?: string
}

export function createPlan(input: Pick<PlanToolInput, 'title' | 'tasks'>): PlanProgress {
  const tasks = input.tasks?.map((task) => task.trim()).filter(Boolean) ?? []
  if (tasks.length === 0) throw new Error('At least one non-empty task is required')
  if (tasks.length > MAX_PLAN_TASKS) {
    throw new Error(`Plans are limited to ${String(MAX_PLAN_TASKS)} tasks`)
  }
  if (new Set(tasks.map((task) => task.toLocaleLowerCase())).size !== tasks.length) {
    throw new Error('Plan tasks must be unique')
  }
  for (const task of tasks) validateLength('Task', task, MAX_TASK_TEXT_LENGTH)
  const title = input.title?.trim()
  if (title) validateLength('Plan title', title, 120)
  const timestamp = new Date().toISOString()
  return {
    id: crypto.randomUUID(),
    ...(title ? { title } : {}),
    revision: 1,
    tasks: tasks.map((text, index) => ({
      id: crypto.randomUUID(),
      sequence: index + 1,
      text,
      status: 'pending',
      createdAt: timestamp,
      updatedAt: timestamp,
    })),
    createdAt: timestamp,
    updatedAt: timestamp,
  }
}

export function updatePlan(plan: PlanProgress, input: PlanToolInput): PlanProgress {
  const statusByAction: Partial<Record<PlanAction, PlanTaskStatus>> = {
    start: 'in_progress',
    complete: 'completed',
    block: 'blocked',
    skip: 'skipped',
  }
  const status = statusByAction[input.action]
  if (!status) throw new Error(`Action ${input.action} cannot update an existing plan`)
  if (input.step === undefined) throw new Error(`A step number is required for ${input.action}`)
  const taskIndex = plan.tasks.findIndex((task) => task.sequence === input.step)
  if (taskIndex < 0) throw new Error(`Task #${String(input.step)} was not found`)
  const task = plan.tasks[taskIndex]
  if (!task) throw new Error(`Task #${String(input.step)} was not found`)
  const note = input.note?.trim()
  if (note) validateLength('Task note', note, MAX_NOTE_LENGTH)
  if (input.action === 'block' && !note) throw new Error('A blocking reason is required')

  if (input.action === 'start') {
    const active = plan.tasks.find(
      (candidate) => candidate.status === 'in_progress' && candidate.id !== task.id,
    )
    if (active) throw new Error(`Task #${String(active.sequence)} is already in progress`)
  }
  if (task.status === 'completed' && input.action !== 'complete') {
    throw new Error(`Task #${String(task.sequence)} is already completed`)
  }

  const timestamp = new Date().toISOString()
  const tasks = plan.tasks.map((candidate, index) => {
    if (index !== taskIndex) return candidate
    const updated = { ...candidate, status, updatedAt: timestamp }
    if (note) updated.note = note
    else if (input.action === 'start') delete updated.note
    return updated
  })
  return {
    ...plan,
    revision: plan.revision + 1,
    tasks,
    updatedAt: timestamp,
  }
}

export function restorePlanFromEntries(entries: readonly unknown[]): PlanProgress | undefined {
  for (const entry of [...entries].reverse()) {
    if (!entry || typeof entry !== 'object') continue
    const candidate = entry as {
      type?: unknown
      message?: { role?: unknown; toolName?: unknown; details?: unknown }
    }
    if (
      candidate.type !== 'message' ||
      candidate.message?.role !== 'toolResult' ||
      candidate.message.toolName !== LUNA_PLAN_TOOL
    ) {
      continue
    }
    const details = candidate.message.details
    if (!details || typeof details !== 'object' || !('plan' in details)) continue
    const plan = (details as { plan?: unknown }).plan
    return isPlanProgress(plan) ? clonePlan(plan) : undefined
  }
  return undefined
}

export function formatPlan(plan: PlanProgress): string {
  const completed = plan.tasks.filter((task) => task.status === 'completed').length
  const lines = [
    `${completed}/${plan.tasks.length} completed${plan.title ? ` — ${plan.title}` : ''}`,
  ]
  for (const task of plan.tasks) {
    const marker = {
      pending: '[ ]',
      in_progress: '[>]',
      completed: '[x]',
      blocked: '[!]',
      skipped: '[-]',
    }[task.status]
    lines.push(
      `${marker} #${String(task.sequence)} ${task.text}${task.note ? ` — ${task.note}` : ''}`,
    )
  }
  const next = plan.tasks.find((task) => task.status === 'pending')
  if (next) lines.push(`Next: #${String(next.sequence)} ${next.text}`)
  return lines.join('\n')
}

export function clonePlan(plan: PlanProgress): PlanProgress {
  return { ...plan, tasks: plan.tasks.map((task) => ({ ...task })) }
}

function isPlanProgress(value: unknown): value is PlanProgress {
  if (!value || typeof value !== 'object') return false
  const plan = value as Partial<PlanProgress>
  return (
    typeof plan.id === 'string' &&
    typeof plan.revision === 'number' &&
    typeof plan.createdAt === 'string' &&
    typeof plan.updatedAt === 'string' &&
    Array.isArray(plan.tasks) &&
    plan.tasks.every(
      (task) =>
        task &&
        typeof task === 'object' &&
        typeof task.id === 'string' &&
        typeof task.sequence === 'number' &&
        typeof task.text === 'string' &&
        ['pending', 'in_progress', 'completed', 'blocked', 'skipped'].includes(task.status) &&
        typeof task.createdAt === 'string' &&
        typeof task.updatedAt === 'string',
    )
  )
}

function validateLength(label: string, value: string, maximum: number): void {
  if (value.length > maximum) {
    throw new Error(`${label} is limited to ${String(maximum)} characters`)
  }
}
