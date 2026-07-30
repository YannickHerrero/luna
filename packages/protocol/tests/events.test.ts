import { describe, expect, it } from 'vitest'
import { Value } from 'typebox/value'
import { ClientCommandSchema, ServerEventSchema } from '../src/index.js'

describe('normalized protocol', () => {
  it('accepts a steering-capable message command', () => {
    expect(
      Value.Check(ClientCommandSchema, {
        version: 1,
        type: 'message.send',
        requestId: '91a841a2-481f-4a71-92b4-c1212ca58a10',
        conversationId: '3e4df6d1-5662-498c-aac2-c98c5c38a838',
        clientMessageId: '682e5a58-51aa-4fee-8f74-c47c3a02cc5f',
        text: 'Focus on the iPad layout.',
        attachmentIds: [],
      }),
    ).toBe(true)
  })

  it('rejects raw tool details as a normalized event', () => {
    expect(
      Value.Check(ServerEventSchema, {
        version: 1,
        type: 'tool_execution_start',
        emittedAt: new Date().toISOString(),
        payload: { command: 'rm -rf /' },
      }),
    ).toBe(false)
  })
})
