import { describe, expect, it } from 'vitest'
import type { Conversation, Message } from '@luna/protocol'
import { applyServerEvent, type LunaClientState } from './events.js'

const conversation = {
  id: '00000000-0000-0000-0000-000000000001',
  title: 'New Conversation',
  state: 'working',
  repositories: [],
} as unknown as Conversation
const message = {
  id: '00000000-0000-0000-0000-000000000002',
  conversationId: conversation.id,
  text: '',
  status: 'streaming',
} as Message
const initial: LunaClientState = {
  conversations: [conversation],
  messages: [message],
  selectedConversationId: conversation.id,
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
