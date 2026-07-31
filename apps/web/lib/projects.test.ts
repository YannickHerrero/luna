import { readFileSync } from 'node:fs'
import type { Conversation, Repository } from '@luna/protocol'
import { describe, expect, it } from 'vitest'
import { groupConversationsByProject, primaryRepository } from './projects.js'

type RepositoryFixture = Pick<
  Repository,
  'id' | 'displayName' | 'rootPath' | 'active' | 'lastSeenAt'
>
type ProjectFixture = {
  name: string
  workingDirectory: string
  repositories: RepositoryFixture[]
  expectedRepositoryId: string | null
}

const fixtures = JSON.parse(
  readFileSync(
    new URL('../../../packages/protocol/tests/fixtures/project-grouping.json', import.meta.url),
    'utf8',
  ),
) as { cases: ProjectFixture[] }

describe('primaryRepository', () => {
  for (const fixture of fixtures.cases) {
    it(fixture.name, () => {
      const conversation = makeConversation({
        repositories: fixture.repositories.map(makeRepository),
        workingDirectory: fixture.workingDirectory,
      })
      expect(primaryRepository(conversation)?.id).toBe(fixture.expectedRepositoryId ?? undefined)
    })
  }
})

describe('groupConversationsByProject', () => {
  it('orders projects by their newest conversation and conversations by recency', () => {
    const firstProject = makeRepository({
      id: '00000000-0000-0000-0000-000000000101',
      displayName: 'First',
      rootPath: '/projects/first',
      active: true,
      lastSeenAt: '2026-03-20T10:00:00Z',
    })
    const secondProject = makeRepository({
      id: '00000000-0000-0000-0000-000000000102',
      displayName: 'Second',
      rootPath: '/projects/second',
      active: true,
      lastSeenAt: '2026-03-20T10:00:00Z',
    })
    const conversations = [
      makeConversation({
        id: '00000000-0000-0000-0000-000000000201',
        repositories: [firstProject],
        workingDirectory: '/projects/first',
        lastMessageAt: '2026-03-20T10:00:00Z',
      }),
      makeConversation({
        id: '00000000-0000-0000-0000-000000000202',
        repositories: [secondProject],
        workingDirectory: '/projects/second',
        lastMessageAt: '2026-03-20T12:00:00Z',
      }),
      makeConversation({
        id: '00000000-0000-0000-0000-000000000203',
        repositories: [firstProject],
        workingDirectory: '/projects/first/subdirectory',
        lastMessageAt: '2026-03-20T11:00:00Z',
      }),
      makeConversation({
        id: '00000000-0000-0000-0000-000000000204',
        repositories: [],
        workingDirectory: '/tmp',
        lastMessageAt: '2026-03-20T09:00:00Z',
      }),
    ]

    const sections = groupConversationsByProject(conversations)
    expect(sections.map((section) => section.id)).toEqual([
      secondProject.id,
      firstProject.id,
      undefined,
    ])
    expect(sections[1]?.conversations.map((conversation) => conversation.id)).toEqual([
      '00000000-0000-0000-0000-000000000203',
      '00000000-0000-0000-0000-000000000201',
    ])
  })
})

function makeRepository(fixture: RepositoryFixture): Repository {
  return {
    ...fixture,
    icon: {
      repositoryId: fixture.id,
      fallbackText: fixture.displayName.slice(0, 1),
      fallbackColor: '#7287fd',
    },
    firstSeenAt: '2026-03-20T08:00:00Z',
  }
}

function makeConversation(overrides: {
  id?: string
  repositories: Repository[]
  workingDirectory: string
  lastMessageAt?: string
}): Conversation {
  return {
    id: overrides.id ?? '00000000-0000-0000-0000-000000000100',
    title: 'Conversation',
    titleMode: 'automatic',
    state: 'idle',
    preview: '',
    activeWorkingDirectory: overrides.workingDirectory,
    repositories: overrides.repositories,
    activities: [],
    unreadCount: 0,
    ...(overrides.lastMessageAt ? { lastMessageAt: overrides.lastMessageAt } : {}),
    createdAt: '2026-03-20T08:00:00Z',
    updatedAt: '2026-03-20T08:00:00Z',
    version: 1,
  }
}
