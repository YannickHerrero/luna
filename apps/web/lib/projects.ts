import type { Conversation, Repository } from '@luna/protocol'
import { sortConversations } from './conversations.js'

export type ConversationProjectSection = {
  id: string | undefined
  repository: Repository | undefined
  conversations: Conversation[]
}

export function primaryRepository(conversation: Conversation): Repository | undefined {
  const repositories = conversation.repositories
  if (repositories.length === 0) return undefined

  const matching = repositories
    .filter((repository) => containsPath(repository.rootPath, conversation.activeWorkingDirectory))
    .sort(
      (left, right) =>
        normalizedPath(right.rootPath).length - normalizedPath(left.rootPath).length ||
        left.id.localeCompare(right.id),
    )
  if (matching[0]) return matching[0]

  const active = repositories.filter((repository) => repository.active).sort(compareRepositories)
  return active[0] ?? [...repositories].sort(compareRepositories)[0]
}

export function groupConversationsByProject(
  conversations: Conversation[],
): ConversationProjectSection[] {
  const groups = new Map<string, ConversationProjectSection>()
  for (const conversation of sortConversations(conversations)) {
    const repository = primaryRepository(conversation)
    const key = repository?.id ?? 'no-project'
    const current = groups.get(key)
    if (current) current.conversations.push(conversation)
    else {
      groups.set(key, {
        id: repository?.id,
        repository,
        conversations: [conversation],
      })
    }
  }
  return [...groups.values()]
}

function compareRepositories(left: Repository, right: Repository): number {
  return (
    Date.parse(right.lastSeenAt) - Date.parse(left.lastSeenAt) || left.id.localeCompare(right.id)
  )
}

function containsPath(rootPath: string, workingDirectory: string): boolean {
  const root = normalizedPath(rootPath)
  const working = normalizedPath(workingDirectory)
  return root === '/' ? working.startsWith('/') : working === root || working.startsWith(`${root}/`)
}

function normalizedPath(path: string): string {
  if (path === '/') return path
  return path.replace(/\/+$/, '')
}
