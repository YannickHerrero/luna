import { describe, expect, it } from 'vitest'
import {
  createPlan,
  formatPlan,
  LUNA_PLAN_TOOL,
  restorePlanFromEntries,
  updatePlan,
  type PlanToolDetails,
} from './plan-progress-state.js'

describe('Pi plan progress', () => {
  it('creates and advances a structured plan after verification', () => {
    const created = createPlan({
      title: 'Ship progress tracking',
      tasks: ['Add the extension', 'Persist updates', 'Verify the interface'],
    })

    expect(created.revision).toBe(1)
    expect(created.tasks.map((task) => task.status)).toEqual(['pending', 'pending', 'pending'])

    const started = updatePlan(created, { action: 'start', step: 1 })
    const completed = updatePlan(started, {
      action: 'complete',
      step: 1,
      note: 'Extension tests passed',
    })

    expect(completed.revision).toBe(3)
    expect(completed.tasks[0]).toMatchObject({
      status: 'completed',
      note: 'Extension tests passed',
    })
    expect(formatPlan(completed)).toContain('Next: #2 Persist updates')
  })

  it('requires one active task and a reason for blocked work', () => {
    const plan = createPlan({ tasks: ['First task', 'Second task'] })
    const started = updatePlan(plan, { action: 'start', step: 1 })

    expect(() => updatePlan(started, { action: 'start', step: 2 })).toThrow(
      'Task #1 is already in progress',
    )
    expect(() => updatePlan(started, { action: 'block', step: 1 })).toThrow(
      'A blocking reason is required',
    )
  })

  it('rejects empty, duplicate, and oversized plans', () => {
    expect(() => createPlan({ tasks: [] })).toThrow('At least one non-empty task is required')
    expect(() => createPlan({ tasks: ['Same task', 'same task'] })).toThrow(
      'Plan tasks must be unique',
    )
    expect(() => createPlan({ tasks: ['x'.repeat(241)] })).toThrow(
      'Task is limited to 240 characters',
    )
  })

  it('restores the latest branch-correct tool snapshot', () => {
    const earlier = createPlan({ tasks: ['Earlier task'] })
    const latest = updatePlan(earlier, { action: 'complete', step: 1 })
    const entries = entriesWithDetails([
      { action: 'replace', plan: earlier },
      { action: 'complete', plan: latest },
    ])

    expect(restorePlanFromEntries(entries)).toEqual(latest)
  })

  it('restores a cleared plan as empty', () => {
    const plan = createPlan({ tasks: ['Temporary task'] })
    const entries = entriesWithDetails([
      { action: 'replace', plan },
      { action: 'clear', plan: null },
    ])

    expect(restorePlanFromEntries(entries)).toBeUndefined()
  })
})

function entriesWithDetails(details: PlanToolDetails[]): unknown[] {
  return details.map((detail, index) => ({
    type: 'message',
    id: `entry-${String(index)}`,
    parentId: index === 0 ? null : `entry-${String(index - 1)}`,
    timestamp: new Date(index).toISOString(),
    message: {
      role: 'toolResult',
      toolCallId: `call-${String(index)}`,
      toolName: LUNA_PLAN_TOOL,
      content: [{ type: 'text', text: 'updated' }],
      details: detail,
      isError: false,
      timestamp: index,
    },
  }))
}
