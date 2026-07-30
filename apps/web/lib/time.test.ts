import { describe, expect, it } from 'vitest'
import { formatConversationTimestamp, formatMessageTimestamp } from './time.js'

describe('timestamp formatting', () => {
  const now = new Date(2026, 2, 20, 15, 0)

  it('uses time today, weekday this week, and a date for older conversations', () => {
    const today = new Date(2026, 2, 20, 9, 5)
    const recent = new Date(2026, 2, 18, 9, 5)
    const older = new Date(2026, 1, 1, 9, 5)
    expect(formatConversationTimestamp(today.toISOString(), now, 'en-US')).toBe(
      new Intl.DateTimeFormat('en-US', { hour: 'numeric', minute: '2-digit' }).format(today),
    )
    expect(formatConversationTimestamp(recent.toISOString(), now, 'en-US')).toBe(
      new Intl.DateTimeFormat('en-US', { weekday: 'short' }).format(recent),
    )
    expect(formatConversationTimestamp(older.toISOString(), now, 'en-US')).toBe(
      new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric' }).format(older),
    )
  })

  it('shows both the date and time for an expanded message timestamp', () => {
    const sentAt = new Date(2026, 2, 20, 9, 5)
    expect(formatMessageTimestamp(sentAt.toISOString(), 'en-US')).toBe(
      new Intl.DateTimeFormat('en-US', {
        dateStyle: 'medium',
        timeStyle: 'short',
      }).format(sentAt),
    )
  })
})
