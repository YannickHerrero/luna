'use client'

import { Settings, X } from 'lucide-react'
import type {
  AgentModel,
  CompactConversationResponse,
  ConversationAgentState,
  ThinkingLevel,
  UpdateConversationAgentRequest,
} from '@luna/protocol'
import { useCallback, useMemo, useRef, useState } from 'react'
import { api, messageFromError } from '../lib/api.js'

const thinkingLabels: Record<ThinkingLevel, string> = {
  off: 'Off',
  minimal: 'Minimal',
  low: 'Low',
  medium: 'Medium',
  high: 'High',
  xhigh: 'Extra high',
  max: 'Maximum',
}

export function AgentControls({
  conversationId,
  busy,
  onError,
}: {
  conversationId: string
  busy: boolean
  onError: (message: string | undefined) => void
}) {
  const dialog = useRef<HTMLDialogElement>(null)
  const trigger = useRef<HTMLButtonElement>(null)
  const [agent, setAgent] = useState<ConversationAgentState>()
  const [selectedModelKey, setSelectedModelKey] = useState('')
  const [selectedThinking, setSelectedThinking] = useState<ThinkingLevel>('off')
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [compacting, setCompacting] = useState(false)
  const [confirmingCompact, setConfirmingCompact] = useState(false)
  const [estimatedTokens, setEstimatedTokens] = useState<number>()

  const installState = useCallback((next: ConversationAgentState) => {
    setAgent(next)
    setSelectedModelKey(next.model ? modelKey(next.model) : '')
    setSelectedThinking(next.thinkingLevel)
  }, [])

  const load = useCallback(async () => {
    setLoading(true)
    onError(undefined)
    try {
      installState(await api<ConversationAgentState>(`/v1/conversations/${conversationId}/agent`))
    } catch (requestError) {
      onError(messageFromError(requestError))
    } finally {
      setLoading(false)
    }
  }, [conversationId, installState, onError])

  const open = () => {
    setConfirmingCompact(false)
    setEstimatedTokens(undefined)
    dialog.current?.showModal()
    void load()
  }

  const close = () => dialog.current?.close()
  const selectedModel = agent?.availableModels.find((model) => modelKey(model) === selectedModelKey)
  const groupedModels = useMemo(() => {
    const groups = new Map<string, AgentModel[]>()
    for (const model of agent?.availableModels ?? []) {
      const group = groups.get(model.provider) ?? []
      group.push(model)
      groups.set(model.provider, group)
    }
    return [...groups.entries()]
  }, [agent?.availableModels])
  const supportedThinking = selectedModel?.supportedThinkingLevels ?? ['off']
  const controlsDisabled = busy || loading || saving || compacting

  const selectModel = (key: string) => {
    setSelectedModelKey(key)
    const model = agent?.availableModels.find((candidate) => modelKey(candidate) === key)
    if (!model || model.supportedThinkingLevels.includes(selectedThinking)) return
    setSelectedThinking(
      model.supportedThinkingLevels.includes('high')
        ? 'high'
        : (model.supportedThinkingLevels.at(-1) ?? 'off'),
    )
  }

  const save = async () => {
    if (!selectedModel) return
    setSaving(true)
    onError(undefined)
    try {
      const request: UpdateConversationAgentRequest = {
        model: { provider: selectedModel.provider, modelId: selectedModel.id },
        thinkingLevel: selectedThinking,
      }
      installState(
        await api<ConversationAgentState>(`/v1/conversations/${conversationId}/agent`, {
          method: 'PATCH',
          body: JSON.stringify(request),
        }),
      )
    } catch (requestError) {
      onError(messageFromError(requestError))
    } finally {
      setSaving(false)
    }
  }

  const compact = async () => {
    setCompacting(true)
    setConfirmingCompact(false)
    onError(undefined)
    try {
      const result = await api<CompactConversationResponse>(
        `/v1/conversations/${conversationId}/compact`,
        { method: 'POST' },
      )
      setEstimatedTokens(result.estimatedTokensAfter)
    } catch (requestError) {
      onError(messageFromError(requestError))
    } finally {
      setCompacting(false)
    }
  }

  const contextWindow = agent?.contextUsage?.contextWindow ?? selectedModel?.contextWindow
  const contextTokens = estimatedTokens ?? agent?.contextUsage?.tokens
  const contextPercent =
    contextTokens !== undefined && contextWindow
      ? Math.min(100, Math.max(0, (contextTokens / contextWindow) * 100))
      : agent?.contextUsage?.percent

  return (
    <>
      <button
        ref={trigger}
        className="icon-button"
        aria-label="Agent settings"
        title="Agent settings"
        onClick={open}
      >
        <Settings size={18} />
      </button>
      <dialog
        ref={dialog}
        className="agent-dialog"
        aria-labelledby="agent-dialog-title"
        onClose={() => trigger.current?.focus()}
        onClick={(event) => {
          if (event.target === event.currentTarget) close()
        }}
      >
        <div className="agent-dialog-panel">
          <header className="agent-dialog-header">
            <div>
              <span className="eyebrow">Conversation controls</span>
              <h2 id="agent-dialog-title">Agent settings</h2>
            </div>
            <button className="icon-button" aria-label="Close agent settings" onClick={close}>
              <X size={18} />
            </button>
          </header>

          {loading && !agent ? (
            <div className="agent-dialog-loading" aria-label="Loading agent settings">
              <span className="button-spinner" />
            </div>
          ) : agent ? (
            <div className="agent-dialog-content" aria-busy={saving || compacting}>
              <section className="agent-setting-section">
                <label htmlFor="agent-model">Model</label>
                <select
                  id="agent-model"
                  value={selectedModelKey}
                  disabled={controlsDisabled}
                  onChange={(event) => selectModel(event.target.value)}
                >
                  {groupedModels.map(([provider, models]) => (
                    <optgroup key={provider} label={provider}>
                      {models.map((model) => (
                        <option key={modelKey(model)} value={modelKey(model)}>
                          {model.name}
                        </option>
                      ))}
                    </optgroup>
                  ))}
                </select>
              </section>

              <section className="agent-setting-section">
                <label htmlFor="thinking-level">Thinking effort</label>
                <select
                  id="thinking-level"
                  value={selectedThinking}
                  disabled={controlsDisabled || supportedThinking.length === 1}
                  onChange={(event) => setSelectedThinking(event.target.value as ThinkingLevel)}
                >
                  {supportedThinking.map((level) => (
                    <option key={level} value={level}>
                      {thinkingLabels[level]}
                    </option>
                  ))}
                </select>
                {supportedThinking.length === 1 && (
                  <p>This model does not support configurable reasoning.</p>
                )}
              </section>

              <button
                className="primary-button agent-save-button"
                disabled={controlsDisabled || !selectedModel}
                onClick={() => void save()}
              >
                {saving ? <span className="button-spinner" /> : 'Apply model settings'}
              </button>
              <p className="agent-default-note">
                Like Pi’s model selector, this also becomes the default for new Pi sessions.
              </p>

              <section className="context-card">
                <div className="context-heading">
                  <div>
                    <span>Conversation context</span>
                    <strong>
                      {contextTokens === undefined
                        ? 'Not measured yet'
                        : `${formatTokens(contextTokens)} / ${formatTokens(contextWindow ?? 0)} tokens`}
                    </strong>
                  </div>
                  {contextPercent !== undefined && <b>{Math.round(contextPercent)}%</b>}
                </div>
                <div
                  className="context-progress"
                  role="progressbar"
                  aria-label="Conversation context used"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={
                    contextPercent === undefined ? undefined : Math.round(contextPercent)
                  }
                >
                  <span style={{ width: `${String(contextPercent ?? 0)}%` }} />
                </div>
                <p>
                  Automatic compaction is {agent.autoCompactionEnabled ? 'enabled' : 'disabled'}.
                  {estimatedTokens !== undefined && ' Size shown is the post-compaction estimate.'}
                </p>
                {confirmingCompact ? (
                  <div className="compact-confirmation" role="alert">
                    <p>Pi will summarize older context while preserving recent work.</p>
                    <div>
                      <button
                        className="secondary-button"
                        disabled={compacting}
                        onClick={() => setConfirmingCompact(false)}
                      >
                        Cancel
                      </button>
                      <button
                        className="primary-button"
                        disabled={compacting}
                        onClick={() => void compact()}
                      >
                        Compact now
                      </button>
                    </div>
                  </div>
                ) : (
                  <button
                    className="secondary-button compact-button"
                    disabled={controlsDisabled || contextTokens === undefined}
                    onClick={() => setConfirmingCompact(true)}
                  >
                    {compacting ? <span className="button-spinner" /> : 'Compact context'}
                  </button>
                )}
              </section>
            </div>
          ) : (
            <p className="agent-dialog-error">Agent settings are unavailable.</p>
          )}
        </div>
      </dialog>
    </>
  )
}

function modelKey(model: Pick<AgentModel, 'provider' | 'id'>): string {
  return JSON.stringify([model.provider, model.id])
}

function formatTokens(value: number): string {
  return new Intl.NumberFormat(undefined, {
    notation: 'compact',
    maximumFractionDigits: 1,
  }).format(value)
}
