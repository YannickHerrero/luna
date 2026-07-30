import { index, integer, primaryKey, sqliteTable, text, uniqueIndex } from 'drizzle-orm/sqlite-core'

export const devices = sqliteTable(
  'devices',
  {
    id: text('id').primaryKey(),
    name: text('name').notNull(),
    platform: text('platform', { enum: ['ios', 'ipados', 'web'] }).notNull(),
    credentialHash: text('credential_hash').notNull(),
    notificationsEnabled: integer('notifications_enabled', { mode: 'boolean' })
      .notNull()
      .default(false),
    appVersion: text('app_version'),
    createdAt: text('created_at').notNull(),
    lastSeenAt: text('last_seen_at').notNull(),
    revokedAt: text('revoked_at'),
  },
  (table) => [uniqueIndex('devices_credential_hash_unique').on(table.credentialHash)],
)

export const conversations = sqliteTable(
  'conversations',
  {
    id: text('id').primaryKey(),
    piSessionId: text('pi_session_id'),
    piSessionPath: text('pi_session_path'),
    title: text('title').notNull().default('New Conversation'),
    titleMode: text('title_mode', { enum: ['automatic', 'manual'] })
      .notNull()
      .default('automatic'),
    state: text('state').notNull().default('creating'),
    preview: text('preview').notNull().default(''),
    activeWorkingDirectory: text('active_working_directory').notNull(),
    notificationTargetDeviceId: text('notification_target_device_id').references(() => devices.id),
    avatarAttachmentId: text('avatar_attachment_id'),
    unreadCount: integer('unread_count').notNull().default(0),
    version: integer('version').notNull().default(1),
    createdAt: text('created_at').notNull(),
    updatedAt: text('updated_at').notNull(),
    archivedAt: text('archived_at'),
  },
  (table) => [uniqueIndex('conversations_pi_session_path_unique').on(table.piSessionPath)],
)

export const messages = sqliteTable(
  'messages',
  {
    id: text('id').primaryKey(),
    conversationId: text('conversation_id')
      .notNull()
      .references(() => conversations.id, { onDelete: 'cascade' }),
    clientMessageId: text('client_message_id'),
    piEntryId: text('pi_entry_id'),
    role: text('role', { enum: ['user', 'assistant'] }).notNull(),
    status: text('status').notNull(),
    delivery: text('delivery', { enum: ['initial', 'steer'] }),
    text: text('text').notNull().default(''),
    sentByDeviceId: text('sent_by_device_id').references(() => devices.id),
    ordinal: integer('ordinal').notNull(),
    createdAt: text('created_at').notNull(),
    updatedAt: text('updated_at').notNull(),
  },
  (table) => [
    uniqueIndex('messages_conversation_ordinal_unique').on(table.conversationId, table.ordinal),
    uniqueIndex('messages_device_client_id_unique').on(table.sentByDeviceId, table.clientMessageId),
    uniqueIndex('messages_pi_entry_id_unique').on(table.piEntryId),
    index('messages_conversation_created_index').on(table.conversationId, table.createdAt),
  ],
)

export const messageChunks = sqliteTable(
  'message_chunks',
  {
    messageId: text('message_id')
      .notNull()
      .references(() => messages.id, { onDelete: 'cascade' }),
    chunkIndex: integer('chunk_index').notNull(),
    contentIndex: integer('content_index').notNull().default(0),
    delta: text('delta').notNull(),
    createdAt: text('created_at').notNull(),
  },
  (table) => [primaryKey({ columns: [table.messageId, table.chunkIndex] })],
)

export const attachments = sqliteTable(
  'attachments',
  {
    id: text('id').primaryKey(),
    conversationId: text('conversation_id').references(() => conversations.id, {
      onDelete: 'cascade',
    }),
    uploadedByDeviceId: text('uploaded_by_device_id').references(() => devices.id),
    storageKey: text('storage_key').notNull(),
    thumbnailStorageKey: text('thumbnail_storage_key').notNull(),
    originalName: text('original_name').notNull(),
    mimeType: text('mime_type').notNull(),
    byteSize: integer('byte_size').notNull(),
    sha256: text('sha256').notNull(),
    width: integer('width').notNull(),
    height: integer('height').notNull(),
    status: text('status').notNull(),
    createdAt: text('created_at').notNull(),
    deletedAt: text('deleted_at'),
  },
  (table) => [index('attachments_conversation_index').on(table.conversationId)],
)

export const messageAttachments = sqliteTable(
  'message_attachments',
  {
    messageId: text('message_id')
      .notNull()
      .references(() => messages.id, { onDelete: 'cascade' }),
    attachmentId: text('attachment_id')
      .notNull()
      .references(() => attachments.id, { onDelete: 'cascade' }),
    position: integer('position').notNull(),
  },
  (table) => [primaryKey({ columns: [table.messageId, table.attachmentId] })],
)

export const repositories = sqliteTable(
  'repositories',
  {
    id: text('id').primaryKey(),
    canonicalRoot: text('canonical_root').notNull(),
    gitDirectory: text('git_directory').notNull(),
    displayName: text('display_name').notNull(),
    iconStorageKey: text('icon_storage_key'),
    iconSource: text('icon_source'),
    iconFingerprint: text('icon_fingerprint'),
    createdAt: text('created_at').notNull(),
    updatedAt: text('updated_at').notNull(),
  },
  (table) => [uniqueIndex('repositories_root_unique').on(table.canonicalRoot)],
)

export const conversationRepositories = sqliteTable(
  'conversation_repositories',
  {
    conversationId: text('conversation_id')
      .notNull()
      .references(() => conversations.id, { onDelete: 'cascade' }),
    repositoryId: text('repository_id')
      .notNull()
      .references(() => repositories.id, { onDelete: 'cascade' }),
    branch: text('branch'),
    active: integer('active', { mode: 'boolean' }).notNull().default(false),
    firstSeenAt: text('first_seen_at').notNull(),
    lastSeenAt: text('last_seen_at').notNull(),
  },
  (table) => [primaryKey({ columns: [table.conversationId, table.repositoryId] })],
)

export const dispatches = sqliteTable(
  'dispatches',
  {
    id: text('id').primaryKey(),
    messageId: text('message_id')
      .notNull()
      .references(() => messages.id, { onDelete: 'cascade' }),
    workerCommandId: text('worker_command_id').notNull(),
    markerEntryId: text('marker_entry_id'),
    state: text('state').notNull(),
    attempts: integer('attempts').notNull().default(0),
    errorCode: text('error_code'),
    createdAt: text('created_at').notNull(),
    updatedAt: text('updated_at').notNull(),
  },
  (table) => [uniqueIndex('dispatches_message_unique').on(table.messageId)],
)

export const syncEvents = sqliteTable(
  'sync_events',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    type: text('type').notNull(),
    conversationId: text('conversation_id').references(() => conversations.id, {
      onDelete: 'cascade',
    }),
    aggregateId: text('aggregate_id'),
    payload: text('payload').notNull(),
    createdAt: text('created_at').notNull(),
  },
  (table) => [index('sync_events_conversation_index').on(table.conversationId, table.id)],
)

export const pairingCodes = sqliteTable(
  'pairing_codes',
  {
    id: text('id').primaryKey(),
    codeHash: text('code_hash').notNull(),
    expiresAt: text('expires_at').notNull(),
    createdAt: text('created_at').notNull(),
    redeemedAt: text('redeemed_at'),
  },
  (table) => [uniqueIndex('pairing_codes_hash_unique').on(table.codeHash)],
)

export const apnsRegistrations = sqliteTable(
  'apns_registrations',
  {
    id: text('id').primaryKey(),
    deviceId: text('device_id')
      .notNull()
      .references(() => devices.id, { onDelete: 'cascade' }),
    token: text('token').notNull(),
    environment: text('environment', { enum: ['sandbox', 'production'] }).notNull(),
    bundleId: text('bundle_id').notNull(),
    updatedAt: text('updated_at').notNull(),
    invalidatedAt: text('invalidated_at'),
  },
  (table) => [uniqueIndex('apns_token_unique').on(table.token)],
)

export const webPushSubscriptions = sqliteTable(
  'web_push_subscriptions',
  {
    id: text('id').primaryKey(),
    deviceId: text('device_id')
      .notNull()
      .references(() => devices.id, { onDelete: 'cascade' }),
    endpointHash: text('endpoint_hash').notNull(),
    encryptedSubscription: text('encrypted_subscription').notNull(),
    updatedAt: text('updated_at').notNull(),
    invalidatedAt: text('invalidated_at'),
  },
  (table) => [uniqueIndex('web_push_endpoint_unique').on(table.endpointHash)],
)

export const notificationDeliveries = sqliteTable(
  'notification_deliveries',
  {
    id: text('id').primaryKey(),
    conversationId: text('conversation_id')
      .notNull()
      .references(() => conversations.id, { onDelete: 'cascade' }),
    messageId: text('message_id').references(() => messages.id, { onDelete: 'set null' }),
    targetDeviceId: text('target_device_id')
      .notNull()
      .references(() => devices.id),
    channel: text('channel', { enum: ['apns', 'web_push'] }).notNull(),
    status: text('status').notNull(),
    responseCode: text('response_code'),
    createdAt: text('created_at').notNull(),
    completedAt: text('completed_at'),
  },
  (table) => [
    uniqueIndex('notification_delivery_unique').on(
      table.messageId,
      table.targetDeviceId,
      table.channel,
    ),
  ],
)

export const sessionRuns = sqliteTable(
  'session_runs',
  {
    id: text('id').primaryKey(),
    conversationId: text('conversation_id')
      .notNull()
      .references(() => conversations.id, { onDelete: 'cascade' }),
    generation: integer('generation').notNull(),
    processId: integer('process_id'),
    state: text('state').notNull(),
    startedAt: text('started_at').notNull(),
    endedAt: text('ended_at'),
    exitCode: integer('exit_code'),
    exitSignal: text('exit_signal'),
    errorCode: text('error_code'),
  },
  (table) => [index('session_runs_conversation_index').on(table.conversationId, table.startedAt)],
)
