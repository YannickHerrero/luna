import { Type, type Static, type TSchema } from 'typebox'
import { ApiErrorSchema, CursorSchema, DateTimeSchema, IdSchema } from './common.js'
import {
  AgentActivitySchema,
  AgentTaskListSchema,
  AttachmentSchema,
  ConversationSchema,
  MessageDeliverySchema,
  MessageSchema,
  RepositorySchema,
  SessionStateSchema,
} from './entities.js'

const envelope = <TType extends string, TPayload extends TSchema>(type: TType, payload: TPayload) =>
  Type.Object(
    {
      version: Type.Literal(1),
      type: Type.Literal(type),
      eventId: Type.Optional(CursorSchema),
      conversationId: Type.Optional(IdSchema),
      emittedAt: DateTimeSchema,
      payload,
    },
    { additionalProperties: false },
  )

export const ClientHelloSchema = Type.Object(
  {
    version: Type.Literal(1),
    type: Type.Literal('client.hello'),
    requestId: IdSchema,
    deviceId: IdSchema,
    lastCursor: CursorSchema,
  },
  { additionalProperties: false },
)

export const MessageSendCommandSchema = Type.Object(
  {
    version: Type.Literal(1),
    type: Type.Literal('message.send'),
    requestId: IdSchema,
    conversationId: IdSchema,
    clientMessageId: IdSchema,
    text: Type.String({ minLength: 1, maxLength: 100_000 }),
    attachmentIds: Type.Array(IdSchema, { maxItems: 8 }),
  },
  { additionalProperties: false },
)

export const SessionInterruptCommandSchema = Type.Object(
  {
    version: Type.Literal(1),
    type: Type.Literal('session.interrupt'),
    requestId: IdSchema,
    conversationId: IdSchema,
  },
  { additionalProperties: false },
)

export const ClientPingSchema = Type.Object(
  {
    version: Type.Literal(1),
    type: Type.Literal('client.ping'),
    requestId: IdSchema,
  },
  { additionalProperties: false },
)

export const ClientCommandSchema = Type.Union([
  ClientHelloSchema,
  MessageSendCommandSchema,
  SessionInterruptCommandSchema,
  ClientPingSchema,
])
export type ClientCommand = Static<typeof ClientCommandSchema>

export const ServerWelcomeEventSchema = envelope(
  'server.welcome',
  Type.Object({ cursor: CursorSchema, resumed: Type.Boolean() }, { additionalProperties: false }),
)
export const CommandAcceptedEventSchema = envelope(
  'command.accepted',
  Type.Object(
    { requestId: IdSchema, message: Type.Optional(MessageSchema) },
    { additionalProperties: false },
  ),
)
export const CommandRejectedEventSchema = envelope(
  'command.rejected',
  Type.Object({ requestId: IdSchema, error: ApiErrorSchema }, { additionalProperties: false }),
)
export const ConversationUpsertedEventSchema = envelope('conversation.upserted', ConversationSchema)
export const ConversationTitleUpdatedEventSchema = envelope(
  'conversation.title_updated',
  Type.Object(
    { title: Type.String({ minLength: 1, maxLength: 120 }), automatic: Type.Boolean() },
    { additionalProperties: false },
  ),
)
export const MessageUpsertedEventSchema = envelope('message.upserted', MessageSchema)
export const MessageDeltaEventSchema = envelope(
  'message.delta',
  Type.Object(
    {
      messageId: IdSchema,
      chunkIndex: Type.Integer({ minimum: 0 }),
      delta: Type.String({ minLength: 1 }),
    },
    { additionalProperties: false },
  ),
)
export const MessageCompletedEventSchema = envelope(
  'message.completed',
  Type.Object({ messageId: IdSchema }, { additionalProperties: false }),
)
export const SessionStateChangedEventSchema = envelope(
  'session.state_changed',
  Type.Object({ state: SessionStateSchema }, { additionalProperties: false }),
)
export const AgentActivityChangedEventSchema = envelope(
  'agent.activity_changed',
  Type.Object(
    {
      active: Type.Boolean(),
      phase: Type.Union([
        Type.Literal('thinking'),
        Type.Literal('working'),
        Type.Literal('compacting'),
        Type.Literal('retrying'),
      ]),
    },
    { additionalProperties: false },
  ),
)
export const AgentActivitiesResetEventSchema = envelope(
  'agent.activities_reset',
  Type.Object({}, { additionalProperties: false }),
)
export const AgentActivityUpsertedEventSchema = envelope(
  'agent.activity_upserted',
  AgentActivitySchema,
)
export const AgentTaskListChangedEventSchema = envelope(
  'agent.task_list_changed',
  Type.Object({ taskList: Type.Optional(AgentTaskListSchema) }, { additionalProperties: false }),
)
export const SteeringQueueChangedEventSchema = envelope(
  'steering.queue_changed',
  Type.Object(
    { pending: Type.Integer({ minimum: 0 }), delivery: MessageDeliverySchema },
    { additionalProperties: false },
  ),
)
export const WorkspaceUpdatedEventSchema = envelope(
  'workspace.updated',
  Type.Object({ workingDirectory: Type.String({ minLength: 1 }) }, { additionalProperties: false }),
)
export const RepositoriesUpdatedEventSchema = envelope(
  'repositories.updated',
  Type.Object({ repositories: Type.Array(RepositorySchema) }, { additionalProperties: false }),
)
export const AttachmentUpdatedEventSchema = envelope('attachment.updated', AttachmentSchema)
export const NotificationTargetChangedEventSchema = envelope(
  'notification_target.changed',
  Type.Object(
    { deviceId: Type.Union([IdSchema, Type.Null()]) },
    { additionalProperties: false },
  ),
)
export const SyncResetRequiredEventSchema = envelope(
  'sync.reset_required',
  Type.Object({ cursor: CursorSchema }, { additionalProperties: false }),
)
export const ErrorEventSchema = envelope('error', ApiErrorSchema)
export const ServerPongEventSchema = envelope(
  'server.pong',
  Type.Object({ requestId: IdSchema }, { additionalProperties: false }),
)

export const ServerEventSchema = Type.Union([
  ServerWelcomeEventSchema,
  CommandAcceptedEventSchema,
  CommandRejectedEventSchema,
  ConversationUpsertedEventSchema,
  ConversationTitleUpdatedEventSchema,
  MessageUpsertedEventSchema,
  MessageDeltaEventSchema,
  MessageCompletedEventSchema,
  SessionStateChangedEventSchema,
  AgentActivityChangedEventSchema,
  AgentActivitiesResetEventSchema,
  AgentActivityUpsertedEventSchema,
  AgentTaskListChangedEventSchema,
  SteeringQueueChangedEventSchema,
  WorkspaceUpdatedEventSchema,
  RepositoriesUpdatedEventSchema,
  AttachmentUpdatedEventSchema,
  NotificationTargetChangedEventSchema,
  SyncResetRequiredEventSchema,
  ErrorEventSchema,
  ServerPongEventSchema,
])
export type ServerEvent = Static<typeof ServerEventSchema>
