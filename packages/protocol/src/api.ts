import { Type, type Static } from 'typebox'
import {
  ApiErrorSchema,
  CursorSchema,
  DateTimeSchema,
  IdSchema,
  NonEmptyTextSchema,
} from './common.js'
import {
  AttachmentSchema,
  BootstrapSchema,
  ConversationSchema,
  DevicePlatformSchema,
  MessageSchema,
} from './entities.js'

export const ThinkingLevelSchema = Type.Union([
  Type.Literal('off'),
  Type.Literal('minimal'),
  Type.Literal('low'),
  Type.Literal('medium'),
  Type.Literal('high'),
  Type.Literal('xhigh'),
  Type.Literal('max'),
])
export type ThinkingLevel = Static<typeof ThinkingLevelSchema>

export const AgentModelSchema = Type.Object(
  {
    provider: Type.String(),
    id: Type.String(),
    name: Type.String(),
    reasoning: Type.Boolean(),
    contextWindow: Type.Integer({ minimum: 1 }),
    supportedThinkingLevels: Type.Array(ThinkingLevelSchema),
  },
  { additionalProperties: false },
)
export type AgentModel = Static<typeof AgentModelSchema>

export const ContextUsageSchema = Type.Object(
  {
    tokens: Type.Optional(Type.Integer({ minimum: 0 })),
    contextWindow: Type.Integer({ minimum: 1 }),
    percent: Type.Optional(Type.Number({ minimum: 0 })),
  },
  { additionalProperties: false },
)
export type ContextUsage = Static<typeof ContextUsageSchema>

export const ConversationAgentStateSchema = Type.Object(
  {
    model: Type.Optional(AgentModelSchema),
    thinkingLevel: ThinkingLevelSchema,
    availableModels: Type.Array(AgentModelSchema),
    contextUsage: Type.Optional(ContextUsageSchema),
    autoCompactionEnabled: Type.Boolean(),
  },
  { additionalProperties: false },
)
export type ConversationAgentState = Static<typeof ConversationAgentStateSchema>

export const AgentModelSelectionSchema = Type.Object(
  { provider: Type.String(), modelId: Type.String() },
  { additionalProperties: false },
)

export const UpdateConversationAgentRequestSchema = Type.Object(
  {
    model: Type.Optional(AgentModelSelectionSchema),
    thinkingLevel: Type.Optional(ThinkingLevelSchema),
  },
  { additionalProperties: false, minProperties: 1 },
)
export type UpdateConversationAgentRequest = Static<typeof UpdateConversationAgentRequestSchema>

export const CompactConversationResponseSchema = Type.Object(
  {
    tokensBefore: Type.Integer({ minimum: 0 }),
    estimatedTokensAfter: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
)
export type CompactConversationResponse = Static<typeof CompactConversationResponseSchema>

export const PairingExchangeRequestSchema = Type.Object(
  {
    code: Type.String({ minLength: 6, maxLength: 6, pattern: '^[0-9]{6}$' }),
    deviceName: Type.String({ minLength: 1, maxLength: 80 }),
    platform: DevicePlatformSchema,
  },
  { additionalProperties: false },
)
export type PairingExchangeRequest = Static<typeof PairingExchangeRequestSchema>

export const PairingExchangeResponseSchema = Type.Object(
  {
    deviceId: IdSchema,
    token: Type.String({ minLength: 32 }),
    bootstrap: BootstrapSchema,
  },
  { additionalProperties: false },
)
export type PairingExchangeResponse = Static<typeof PairingExchangeResponseSchema>

export const PairingCodeRequestResponseSchema = Type.Object(
  { expiresAt: Type.String() },
  { additionalProperties: false },
)
export type PairingCodeRequestResponse = Static<typeof PairingCodeRequestResponseSchema>

export const CreateConversationRequestSchema = Type.Object({}, { additionalProperties: false })
export const CreateConversationResponseSchema = ConversationSchema

export const UpdateConversationRequestSchema = Type.Object(
  {
    title: Type.Optional(Type.String({ minLength: 1, maxLength: 120 })),
    avatarAttachmentId: Type.Optional(IdSchema),
  },
  { additionalProperties: false, minProperties: 1 },
)

export const SendMessageRequestSchema = Type.Object(
  {
    clientMessageId: IdSchema,
    text: NonEmptyTextSchema,
    attachmentIds: Type.Array(IdSchema, { maxItems: 8 }),
  },
  { additionalProperties: false },
)
export type SendMessageRequest = Static<typeof SendMessageRequestSchema>

export const SendMessageResponseSchema = Type.Object(
  {
    accepted: Type.Boolean(),
    message: MessageSchema,
  },
  { additionalProperties: false },
)
export type SendMessageResponse = Static<typeof SendMessageResponseSchema>

export const OpenAiUsageAvailabilitySchema = Type.Union([
  Type.Literal('available'),
  Type.Literal('stale'),
  Type.Literal('unavailable'),
])
export type OpenAiUsageAvailability = Static<typeof OpenAiUsageAvailabilitySchema>

export const OpenAiWeeklyUsageSchema = Type.Object(
  {
    availability: OpenAiUsageAvailabilitySchema,
    usedPercent: Type.Optional(Type.Integer({ minimum: 0, maximum: 100 })),
    resetsAt: Type.Optional(DateTimeSchema),
    collectedAt: Type.Optional(DateTimeSchema),
  },
  { additionalProperties: false },
)
export type OpenAiWeeklyUsage = Static<typeof OpenAiWeeklyUsageSchema>

export const ConversationMessagesSchema = Type.Object(
  {
    messages: Type.Array(MessageSchema),
    nextBeforeOrdinal: Type.Optional(Type.Integer({ minimum: 1 })),
  },
  { additionalProperties: false },
)
export type ConversationMessages = Static<typeof ConversationMessagesSchema>

export const AttachmentResponseSchema = Type.Object(
  { attachment: AttachmentSchema },
  { additionalProperties: false },
)
export type AttachmentResponse = Static<typeof AttachmentResponseSchema>
export const TranscriptionResponseSchema = Type.Object(
  { text: Type.String({ maxLength: 100_000 }) },
  { additionalProperties: false },
)
export type TranscriptionResponse = Static<typeof TranscriptionResponseSchema>

export const SyncResponseSchema = Type.Object(
  {
    cursor: CursorSchema,
    events: Type.Array(Type.Unknown()),
    resetRequired: Type.Boolean(),
  },
  { additionalProperties: false },
)

export const ApiResponseSchemas = {
  PairingExchangeResponse: PairingExchangeResponseSchema,
  PairingCodeRequestResponse: PairingCodeRequestResponseSchema,
  Conversation: ConversationSchema,
  SendMessageResponse: SendMessageResponseSchema,
  ConversationMessages: ConversationMessagesSchema,
  ConversationAgentState: ConversationAgentStateSchema,
  CompactConversationResponse: CompactConversationResponseSchema,
  OpenAiWeeklyUsage: OpenAiWeeklyUsageSchema,
  AttachmentResponse: AttachmentResponseSchema,
  TranscriptionResponse: TranscriptionResponseSchema,
  Bootstrap: BootstrapSchema,
  SyncResponse: SyncResponseSchema,
  ApiError: ApiErrorSchema,
} as const
