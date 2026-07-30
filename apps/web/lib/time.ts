export function formatMessageTimestamp(value: string, locales?: Intl.LocalesArgument): string {
  return new Intl.DateTimeFormat(locales, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value))
}

export function formatConversationTimestamp(
  value: string,
  now = new Date(),
  locales?: Intl.LocalesArgument,
): string {
  const date = new Date(value)
  const dayDifference = calendarDay(now) - calendarDay(date)
  if (dayDifference === 0) {
    return new Intl.DateTimeFormat(locales, {
      hour: 'numeric',
      minute: '2-digit',
    }).format(date)
  }
  if (dayDifference > 0 && dayDifference < 7) {
    return new Intl.DateTimeFormat(locales, { weekday: 'short' }).format(date)
  }
  return new Intl.DateTimeFormat(locales, {
    month: 'short',
    day: 'numeric',
    year: date.getFullYear() === now.getFullYear() ? undefined : 'numeric',
  }).format(date)
}

function calendarDay(date: Date): number {
  return Math.floor(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / 86_400_000)
}
