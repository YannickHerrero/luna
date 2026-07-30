import { describe, expect, it } from 'vitest'
import type { Conversation, Message } from '@luna/protocol'
import { applyServerEvent, type LunaClientState } from './events.js'

const conversation = {
  id: '00000000-0000-0000-0000-000000000001',
  title: 'New Conversation',
  state: 'working',
  repositories: [],
  activities: [],
} as unknown as Conversation
const message = {
  id: '00000000-0000-0000-0000-000000000002',
  conversationId: conversation.id,
  text: '',
  status: 'streaming',
  ordinal: 2,
} as Message
const initial: LunaClientState = {
  conversations: [conversation],
  messages: [message],
  selectedConversationId: conversation.id,
  nextBeforeOrdinal: undefined,
  cursor: 2,
}

describe('Luna event reducer', () => {
  it('applies streamed deltas and completion in cursor order', () => {
    const streamed = applyServerEvent(initial, {
      eventId: 3,
      conversationId: conversation.id,
      type: 'message.delta',
      payload: { messageId: message.id, delta: 'Hello' },
    })
    const completed = applyServerEvent(streamed, {
      eventId: 4,
      conversationId: conversation.id,
      type: 'message.completed',
      payload: { messageId: message.id },
    })
    expect(completed.messages[0]?.text).toBe('Hello')
    expect(completed.messages[0]?.status).toBe('completed')
    expect(completed.cursor).toBe(4)
  })

  it('keeps live message upserts in transcript order', () => {
    const state = applyServerEvent(initial, {
      eventId: 5,
      conversationId: conversation.id,
      type: 'message.upserted',
      payload: {
        ...message,
        id: '00000000-0000-0000-0000-000000000003',
        ordinal: 1,
      },
    })
    expect(state.messages.map((item) => item.ordinal)).toEqual([1, 2])
  })

  it('removes archived conversations delivered over the live stream', () => {
    const state = applyServerEvent(initial, {
      eventId: 5,
      conversationId: conversation.id,
      type: 'conversation.upserted',
      payload: { ...conversation, archivedAt: '2026-03-20T12:00:00Z' },
    })
    expect(state.conversations).toEqual([])
    expect(state.selectedConversationId).toBeUndefined()
  })

  it('upserts and resets ordered Pi progress summaries', () => {
    const first = applyServerEvent(initial, {
      eventId: 5,
      conversationId: conversation.id,
      type: 'agent.activity_upserted',
      payload: {
        id: '00000000-0000-0000-0000-000000000010',
        sequence: 0,
        summary: 'Planning Luna deployment',
        createdAt: '2026-03-20T12:00:00Z',
        updatedAt: '2026-03-20T12:00:00Z',
      },
    })
    const updated = applyServerEvent(first, {
      eventId: 6,
      conversationId: conversation.id,
      type: 'agent.activity_upserted',
      payload: {
        ...first.conversations[0]?.activities[0],
        id: '00000000-0000-0000-0000-000000000010',
        sequence: 0,
        summary: 'Planning Luna deployment with log verification',
        createdAt: '2026-03-20T12:00:00Z',
        updatedAt: '2026-03-20T12:00:01Z',
      },
    })
    expect(updated.conversations[0]?.activities.map((activity) => activity.summary)).toEqual([
      'Planning Luna deployment with log verification',
    ])

    const reset = applyServerEvent(updated, {
      eventId: 7,
      conversationId: conversation.id,
      type: 'agent.activities_reset',
      payload: {},
    })
    expect(reset.conversations[0]?.activities).toEqual([])
  })

  it('updates normalized conversation state without terminal output', () => {
    const state = applyServerEvent(initial, {
      eventId: 5,
      conversationId: conversation.id,
      type: 'session.state_changed',
      payload: { state: 'idle' },
    })
    expect(state.conversations[0]?.state).toBe('idle')
  })
})
