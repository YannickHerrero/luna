import type { AgentActivity, AgentTaskList, Conversation, Message } from '@luna/protocol'
import { applyLatestMessage, upsertConversation } from './conversations.js'

export type LunaEvent = {
  eventId?: number
  conversationId?: string
  type: string
  payload?: unknown
}

export type LunaClientState = {
  conversations: Conversation[]
  messages: Message[]
  selectedConversationId: string | undefined
  nextBeforeOrdinal: number | undefined
  cursor: number
}

type Identified = { id: string }
type Delta = { messageId: string; delta: string }
type Completed = { messageId: string }
type StateChanged = { state: Conversation['state'] }
type WorkspaceUpdated = { workingDirectory: string }
type RepositoriesUpdated = { repositories: Conversation['repositories'] }
type TitleUpdated = { title: string }
type TaskListChanged = { taskList?: AgentTaskList }

export function applyServerEvent(state: LunaClientState, event: LunaEvent): LunaClientState {
  const cursor = Math.max(state.cursor, event.eventId ?? state.cursor)
  switch (event.type) {
    case 'conversation.upserted': {
      const conversation = event.payload as Conversation
      if (conversation.archivedAt) {
        const selected = state.selectedConversationId === conversation.id
        return {
          ...state,
          cursor,
          conversations: state.conversations.filter((item) => item.id !== conversation.id),
          messages: selected ? [] : state.messages,
          selectedConversationId: selected ? undefined : state.selectedConversationId,
          nextBeforeOrdinal: selected ? undefined : state.nextBeforeOrdinal,
        }
      }
      return {
        ...state,
        cursor,
        conversations: upsertConversation(state.conversations, conversation),
      }
    }
    case 'message.upserted': {
      const message = event.payload as Message
      const conversations = applyLatestMessage(state.conversations, message)
      if (message.conversationId !== state.selectedConversationId) {
        return { ...state, cursor, conversations }
      }
      return {
        ...state,
        cursor,
        conversations,
        messages: upsertMessage(state.messages, message),
      }
    }
    case 'message.delta': {
      const delta = event.payload as Delta
      return {
        ...state,
        cursor,
        messages: state.messages.map((message) =>
          message.id === delta.messageId
            ? { ...message, text: message.text + delta.delta, status: 'streaming' }
            : message,
        ),
      }
    }
    case 'message.completed': {
      const completed = event.payload as Completed
      return {
        ...state,
        cursor,
        messages: state.messages.map((message) =>
          message.id === completed.messageId ? { ...message, status: 'completed' } : message,
        ),
      }
    }
    case 'agent.activities_reset':
      return updateConversationActivities(state, event, [], cursor)
    case 'agent.activity_upserted': {
      const activity = event.payload as AgentActivity
      const conversation = state.conversations.find((item) => item.id === event.conversationId)
      if (!conversation) return { ...state, cursor }
      const activities = upsert(conversation.activities ?? [], activity).sort(
        (left, right) => left.sequence - right.sequence,
      )
      return updateConversationActivities(state, event, activities, cursor)
    }
    case 'agent.task_list_changed':
      return updateConversationTaskList(state, event, event.payload as TaskListChanged, cursor)
    case 'session.state_changed':
      return updateConversation(state, event, event.payload as StateChanged, cursor)
    case 'workspace.updated':
      return updateConversation(state, event, event.payload as WorkspaceUpdated, cursor)
    case 'repositories.updated':
      return updateConversation(state, event, event.payload as RepositoriesUpdated, cursor)
    case 'conversation.title_updated':
      return updateConversation(state, event, event.payload as TitleUpdated, cursor)
    default:
      return { ...state, cursor }
  }
}

function updateConversation(
  state: LunaClientState,
  event: LunaEvent,
  payload: StateChanged | WorkspaceUpdated | RepositoriesUpdated | TitleUpdated,
  cursor: number,
): LunaClientState {
  const conversations = state.conversations.map((conversation) => {
    if (conversation.id !== event.conversationId) return conversation
    if ('state' in payload) return { ...conversation, state: payload.state }
    if ('workingDirectory' in payload) {
      return { ...conversation, activeWorkingDirectory: payload.workingDirectory }
    }
    if ('repositories' in payload) return { ...conversation, repositories: payload.repositories }
    return { ...conversation, title: payload.title }
  })
  return { ...state, cursor, conversations }
}

function updateConversationTaskList(
  state: LunaClientState,
  event: LunaEvent,
  payload: TaskListChanged,
  cursor: number,
): LunaClientState {
  return {
    ...state,
    cursor,
    conversations: state.conversations.map((conversation) => {
      if (conversation.id !== event.conversationId) return conversation
      if (payload.taskList) return { ...conversation, taskList: payload.taskList }
      const next = { ...conversation }
      delete next.taskList
      return next
    }),
  }
}

function updateConversationActivities(
  state: LunaClientState,
  event: LunaEvent,
  activities: AgentActivity[],
  cursor: number,
): LunaClientState {
  return {
    ...state,
    cursor,
    conversations: state.conversations.map((conversation) =>
      conversation.id === event.conversationId ? { ...conversation, activities } : conversation,
    ),
  }
}

function upsertMessage(messages: Message[], message: Message): Message[] {
  return upsert(messages, message).sort((left, right) => left.ordinal - right.ordinal)
}

function upsert<T extends Identified>(values: T[], value: T): T[] {
  const index = values.findIndex((existing) => existing.id === value.id)
  if (index < 0) return [value, ...values]
  return values.map((existing, position) => (position === index ? value : existing))
}
