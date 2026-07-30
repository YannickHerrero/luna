import { StringEnum } from '@earendil-works/pi-ai'
import type { ExtensionAPI, ExtensionContext } from '@earendil-works/pi-coding-agent'
import { Type } from 'typebox'
import {
  clonePlan,
  createPlan,
  formatPlan,
  LUNA_PLAN_TOOL,
  MAX_PLAN_TASKS,
  restorePlanFromEntries,
  updatePlan,
  type PlanAction,
  type PlanProgress,
  type PlanToolDetails,
} from './plan-progress-state.js'

export type { PlanProgress } from './plan-progress-state.js'

type PlanProgressOptions = {
  onChanged?: (plan: PlanProgress | undefined) => void
}

const PlanParams = Type.Object({
  action: StringEnum(['replace', 'start', 'complete', 'block', 'skip', 'show', 'clear'] as const),
  title: Type.Optional(Type.String({ description: 'Short plan title for replace' })),
  tasks: Type.Optional(
    Type.Array(Type.String({ description: 'Outcome-oriented task text' }), {
      minItems: 1,
      maxItems: MAX_PLAN_TASKS,
    }),
  ),
  step: Type.Optional(Type.Integer({ minimum: 1, description: 'One-based task number' })),
  note: Type.Optional(Type.String({ description: 'Verification result or blocking reason' })),
})

export function registerPlanProgress(pi: ExtensionAPI, options: PlanProgressOptions = {}): void {
  let plan: PlanProgress | undefined

  const publish = (ctx?: ExtensionContext) => {
    options.onChanged?.(plan ? clonePlan(plan) : undefined)
    updateWidget(ctx, plan)
  }

  const restore = (ctx: ExtensionContext) => {
    plan = restorePlanFromEntries(ctx.sessionManager.getBranch())
    publish(ctx)
  }

  pi.on('session_start', (_event, ctx) => restore(ctx))
  pi.on('session_tree', (_event, ctx) => restore(ctx))

  pi.on('before_agent_start', () => {
    if (!plan) return
    return {
      message: {
        customType: 'luna-plan-context',
        content: `Current structured plan progress:\n${formatPlan(plan)}\n\nKeep this list current with ${LUNA_PLAN_TOOL}. Mark a task complete only after verification.`,
        display: false,
      },
    }
  })

  pi.registerTool({
    name: LUNA_PLAN_TOOL,
    label: 'Plan Progress',
    description:
      'Create and update the temporary structured plan shown in Luna. Actions: replace (tasks, optional title), start (step), complete (step, optional verification note), block (step, reason in note), skip (step, optional note), show, and clear.',
    promptSnippet: 'Create and update the temporary execution plan displayed in Luna',
    promptGuidelines: [
      `Use ${LUNA_PLAN_TOOL} replace when presenting a meaningful multi-step plan, and replace it again if the plan changes.`,
      `Use ${LUNA_PLAN_TOOL} start before working on each planned task, then complete it only after verification; use block or skip rather than claiming unfinished work is complete.`,
      `After every ${LUNA_PLAN_TOOL} progress update, review the returned remaining tasks before continuing. Clear the plan when it is cancelled, but keep a completed plan available for the user until it is replaced.`,
    ],
    parameters: PlanParams,

    async execute(_toolCallId, input, signal, _onUpdate, ctx) {
      if (signal?.aborted) throw new Error('Plan update cancelled')
      await Promise.resolve()

      if (input.action === 'show') return toolResult('show', plan)
      if (input.action === 'clear') {
        plan = undefined
        publish(ctx)
        return toolResult('clear', plan)
      }
      if (input.action === 'replace') {
        plan = createPlan(input)
        publish(ctx)
        return toolResult('replace', plan)
      }

      if (!plan)
        throw new Error(`No active plan. Call ${LUNA_PLAN_TOOL} with action "replace" first.`)
      plan = updatePlan(plan, input)
      publish(ctx)
      return toolResult(input.action, plan)
    },
  })
}

function toolResult(action: PlanAction, plan: PlanProgress | undefined) {
  return {
    content: [
      {
        type: 'text' as const,
        text: plan ? formatPlan(plan) : action === 'clear' ? 'Plan cleared' : 'No active plan',
      },
    ],
    details: { action, plan: plan ? clonePlan(plan) : null } satisfies PlanToolDetails,
  }
}

function updateWidget(ctx: ExtensionContext | undefined, plan: PlanProgress | undefined): void {
  if (!ctx || ctx.mode !== 'tui') return
  if (!plan) {
    ctx.ui.setWidget('luna-plan-progress', undefined)
    return
  }
  ctx.ui.setWidget('luna-plan-progress', formatPlan(plan).split('\n'))
}
