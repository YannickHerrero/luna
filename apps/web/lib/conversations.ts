import type { Conversation, Message } from '@luna/protocol'

export function sortConversations(conversations: Conversation[]): Conversation[] {
  return [...conversations].sort((left, right) => {
    const recency = conversationTimestamp(right) - conversationTimestamp(left)
    return recency || right.id.localeCompare(left.id)
  })
}

export function upsertConversation(
  conversations: Conversation[],
  conversation: Conversation,
): Conversation[] {
  const found = conversations.some((item) => item.id === conversation.id)
  const next = found
    ? conversations.map((item) => (item.id === conversation.id ? conversation : item))
    : [...conversations, conversation]
  return sortConversations(next)
}

export function applyLatestMessage(
  conversations: Conversation[],
  message: Message,
): Conversation[] {
  const next = conversations.map((conversation) => {
    if (conversation.id !== message.conversationId) return conversation
    const current = conversation.lastMessageAt
    if (current && Date.parse(current) >= Date.parse(message.createdAt)) return conversation
    return { ...conversation, lastMessageAt: message.createdAt }
  })
  return sortConversations(next)
}

function conversationTimestamp(conversation: Conversation): number {
  return Date.parse(conversation.lastMessageAt ?? conversation.createdAt)
}
