import { Type } from 'typebox'

export const IdSchema = Type.String({ format: 'uuid' })
export const DateTimeSchema = Type.String({ format: 'date-time' })
export const CursorSchema = Type.Integer({ minimum: 0 })
export const NonEmptyTextSchema = Type.String({ minLength: 1, maxLength: 100_000 })

export const ErrorCodeSchema = Type.Union([
  Type.Literal('authentication_required'),
  Type.Literal('forbidden'),
  Type.Literal('invalid_request'),
  Type.Literal('not_found'),
  Type.Literal('conflict'),
  Type.Literal('agent_unavailable'),
  Type.Literal('agent_rejected'),
  Type.Literal('attachment_invalid'),
  Type.Literal('transcription_failed'),
  Type.Literal('rate_limited'),
  Type.Literal('internal_error'),
])

export const ApiErrorSchema = Type.Object(
  {
    code: ErrorCodeSchema,
    message: Type.String({ minLength: 1, maxLength: 500 }),
    retryable: Type.Boolean(),
    requestId: Type.Optional(Type.String()),
  },
  { additionalProperties: false },
)
