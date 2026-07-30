import { Type, type Static } from 'typebox'
import { CursorSchema, DateTimeSchema, IdSchema } from './common.js'

export const ThemeSchema = Type.Union([Type.Literal('latte'), Type.Literal('mocha')])
export type Theme = Static<typeof ThemeSchema>

export const DevicePlatformSchema = Type.Union([
  Type.Literal('ios'),
  Type.Literal('ipados'),
  Type.Literal('web'),
])
export type DevicePlatform = Static<typeof DevicePlatformSchema>

export const DeviceSchema = Type.Object(
  {
    id: IdSchema,
    name: Type.String({ minLength: 1, maxLength: 80 }),
    platform: DevicePlatformSchema,
    notificationsEnabled: Type.Boolean(),
    createdAt: DateTimeSchema,
    lastSeenAt: DateTimeSchema,
  },
  { additionalProperties: false },
)
export type Device = Static<typeof DeviceSchema>

export const SessionStateSchema = Type.Union([
  Type.Literal('creating'),
  Type.Literal('starting'),
  Type.Literal('idle'),
  Type.Literal('working'),
  Type.Literal('compacting'),
  Type.Literal('retrying'),
  Type.Literal('crashed'),
  Type.Literal('restoring'),
  Type.Literal('interrupted'),
  Type.Literal('stopped'),
  Type.Literal('error'),
])
export type SessionState = Static<typeof SessionStateSchema>

export const MessageRoleSchema = Type.Union([Type.Literal('user'), Type.Literal('assistant')])
export const MessageStatusSchema = Type.Union([
  Type.Literal('pending'),
  Type.Literal('accepted'),
  Type.Literal('queued'),
  Type.Literal('streaming'),
  Type.Literal('completed'),
  Type.Literal('interrupted'),
  Type.Literal('failed'),
])
export const MessageDeliverySchema = Type.Union([
  Type.Literal('initial'),
  Type.Literal('steer'),
  Type.Literal('bash'),
])

export const AttachmentStatusSchema = Type.Union([
  Type.Literal('uploading'),
  Type.Literal('ready'),
  Type.Literal('attached'),
  Type.Literal('failed'),
  Type.Literal('deleted'),
])

export const AttachmentSchema = Type.Object(
  {
    id: IdSchema,
    fileName: Type.String({ minLength: 1, maxLength: 255 }),
    mimeType: Type.String({ minLength: 1, maxLength: 100 }),
    byteSize: Type.Integer({ minimum: 0 }),
    width: Type.Integer({ minimum: 1 }),
    height: Type.Integer({ minimum: 1 }),
    status: AttachmentStatusSchema,
    contentUrl: Type.String(),
    thumbnailUrl: Type.String(),
    createdAt: DateTimeSchema,
  },
  { additionalProperties: false },
)
export type Attachment = Static<typeof AttachmentSchema>

export const MessageSchema = Type.Object(
  {
    id: IdSchema,
    conversationId: IdSchema,
    clientMessageId: Type.Optional(IdSchema),
    role: MessageRoleSchema,
    status: MessageStatusSchema,
    delivery: Type.Optional(MessageDeliverySchema),
    text: Type.String({ maxLength: 1_000_000 }),
    attachments: Type.Array(AttachmentSchema),
    sentByDeviceId: Type.Optional(IdSchema),
    ordinal: Type.Integer({ minimum: 1 }),
    createdAt: DateTimeSchema,
    updatedAt: DateTimeSchema,
  },
  { additionalProperties: false },
)
export type Message = Static<typeof MessageSchema>

export const RepositoryIconSchema = Type.Object(
  {
    repositoryId: IdSchema,
    contentUrl: Type.Optional(Type.String()),
    fallbackText: Type.String({ minLength: 1, maxLength: 3 }),
    fallbackColor: Type.String({ pattern: '^#[0-9a-fA-F]{6}$' }),
  },
  { additionalProperties: false },
)

export const RepositorySchema = Type.Object(
  {
    id: IdSchema,
    displayName: Type.String({ minLength: 1, maxLength: 160 }),
    rootPath: Type.String({ minLength: 1 }),
    branch: Type.Optional(Type.String({ maxLength: 255 })),
    active: Type.Boolean(),
    icon: RepositoryIconSchema,
    firstSeenAt: DateTimeSchema,
    lastSeenAt: DateTimeSchema,
  },
  { additionalProperties: false },
)
export type Repository = Static<typeof RepositorySchema>

export const AgentActivitySchema = Type.Object(
  {
    id: IdSchema,
    sequence: Type.Integer({ minimum: 0 }),
    summary: Type.String({ minLength: 1, maxLength: 240 }),
    createdAt: DateTimeSchema,
    updatedAt: DateTimeSchema,
  },
  { additionalProperties: false },
)
export type AgentActivity = Static<typeof AgentActivitySchema>

export const ConversationSchema = Type.Object(
  {
    id: IdSchema,
    title: Type.String({ minLength: 1, maxLength: 120 }),
    titleMode: Type.Union([Type.Literal('automatic'), Type.Literal('manual')]),
    state: SessionStateSchema,
    preview: Type.String({ maxLength: 240 }),
    activeWorkingDirectory: Type.String({ minLength: 1 }),
    repositories: Type.Array(RepositorySchema),
    activities: Type.Array(AgentActivitySchema),
    lastMessageAt: Type.Optional(DateTimeSchema),
    notificationTargetDeviceId: Type.Optional(IdSchema),
    unreadCount: Type.Integer({ minimum: 0 }),
    archivedAt: Type.Optional(DateTimeSchema),
    createdAt: DateTimeSchema,
    updatedAt: DateTimeSchema,
    version: Type.Integer({ minimum: 1 }),
  },
  { additionalProperties: false },
)
export type Conversation = Static<typeof ConversationSchema>

export const BootstrapSchema = Type.Object(
  {
    protocolVersion: Type.Literal(1),
    cursor: CursorSchema,
    device: DeviceSchema,
    conversations: Type.Array(ConversationSchema),
  },
  { additionalProperties: false },
)
export type Bootstrap = Static<typeof BootstrapSchema>
