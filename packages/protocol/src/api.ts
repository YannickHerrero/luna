import { Type, type Static } from 'typebox'
import { ApiErrorSchema, CursorSchema, IdSchema, NonEmptyTextSchema } from './common.js'
import {
  AttachmentSchema,
  BootstrapSchema,
  ConversationSchema,
  DevicePlatformSchema,
  MessageSchema,
} from './entities.js'

export const PairingExchangeRequestSchema = Type.Object(
  {
    code: Type.String({ minLength: 6, maxLength: 128 }),
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
  AttachmentResponse: AttachmentResponseSchema,
  TranscriptionResponse: TranscriptionResponseSchema,
  Bootstrap: BootstrapSchema,
  SyncResponse: SyncResponseSchema,
  ApiError: ApiErrorSchema,
} as const
