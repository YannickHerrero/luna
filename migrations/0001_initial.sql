CREATE TABLE `apns_registrations` (
	`id` text PRIMARY KEY NOT NULL,
	`device_id` text NOT NULL,
	`token` text NOT NULL,
	`environment` text NOT NULL,
	`bundle_id` text NOT NULL,
	`updated_at` text NOT NULL,
	`invalidated_at` text,
	FOREIGN KEY (`device_id`) REFERENCES `devices`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE UNIQUE INDEX `apns_token_unique` ON `apns_registrations` (`token`);
CREATE TABLE `attachments` (
	`id` text PRIMARY KEY NOT NULL,
	`conversation_id` text,
	`uploaded_by_device_id` text,
	`storage_key` text NOT NULL,
	`thumbnail_storage_key` text NOT NULL,
	`original_name` text NOT NULL,
	`mime_type` text NOT NULL,
	`byte_size` integer NOT NULL,
	`sha256` text NOT NULL,
	`width` integer NOT NULL,
	`height` integer NOT NULL,
	`status` text NOT NULL,
	`created_at` text NOT NULL,
	`deleted_at` text,
	FOREIGN KEY (`conversation_id`) REFERENCES `conversations`(`id`) ON UPDATE no action ON DELETE cascade,
	FOREIGN KEY (`uploaded_by_device_id`) REFERENCES `devices`(`id`) ON UPDATE no action ON DELETE no action
);

CREATE INDEX `attachments_conversation_index` ON `attachments` (`conversation_id`);
CREATE TABLE `conversation_repositories` (
	`conversation_id` text NOT NULL,
	`repository_id` text NOT NULL,
	`branch` text,
	`active` integer DEFAULT false NOT NULL,
	`first_seen_at` text NOT NULL,
	`last_seen_at` text NOT NULL,
	PRIMARY KEY(`conversation_id`, `repository_id`),
	FOREIGN KEY (`conversation_id`) REFERENCES `conversations`(`id`) ON UPDATE no action ON DELETE cascade,
	FOREIGN KEY (`repository_id`) REFERENCES `repositories`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE TABLE `conversations` (
	`id` text PRIMARY KEY NOT NULL,
	`pi_session_id` text,
	`pi_session_path` text,
	`title` text DEFAULT 'New Conversation' NOT NULL,
	`title_mode` text DEFAULT 'automatic' NOT NULL,
	`state` text DEFAULT 'creating' NOT NULL,
	`preview` text DEFAULT '' NOT NULL,
	`active_working_directory` text NOT NULL,
	`notification_target_device_id` text,
	`avatar_attachment_id` text,
	`unread_count` integer DEFAULT 0 NOT NULL,
	`version` integer DEFAULT 1 NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	`archived_at` text,
	FOREIGN KEY (`notification_target_device_id`) REFERENCES `devices`(`id`) ON UPDATE no action ON DELETE no action
);

CREATE UNIQUE INDEX `conversations_pi_session_path_unique` ON `conversations` (`pi_session_path`);
CREATE TABLE `devices` (
	`id` text PRIMARY KEY NOT NULL,
	`name` text NOT NULL,
	`platform` text NOT NULL,
	`credential_hash` text NOT NULL,
	`notifications_enabled` integer DEFAULT false NOT NULL,
	`app_version` text,
	`created_at` text NOT NULL,
	`last_seen_at` text NOT NULL,
	`revoked_at` text
);

CREATE UNIQUE INDEX `devices_credential_hash_unique` ON `devices` (`credential_hash`);
CREATE TABLE `dispatches` (
	`id` text PRIMARY KEY NOT NULL,
	`message_id` text NOT NULL,
	`worker_command_id` text NOT NULL,
	`marker_entry_id` text,
	`state` text NOT NULL,
	`attempts` integer DEFAULT 0 NOT NULL,
	`error_code` text,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	FOREIGN KEY (`message_id`) REFERENCES `messages`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE UNIQUE INDEX `dispatches_message_unique` ON `dispatches` (`message_id`);
CREATE TABLE `message_attachments` (
	`message_id` text NOT NULL,
	`attachment_id` text NOT NULL,
	`position` integer NOT NULL,
	PRIMARY KEY(`message_id`, `attachment_id`),
	FOREIGN KEY (`message_id`) REFERENCES `messages`(`id`) ON UPDATE no action ON DELETE cascade,
	FOREIGN KEY (`attachment_id`) REFERENCES `attachments`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE TABLE `message_chunks` (
	`message_id` text NOT NULL,
	`chunk_index` integer NOT NULL,
	`content_index` integer DEFAULT 0 NOT NULL,
	`delta` text NOT NULL,
	`created_at` text NOT NULL,
	PRIMARY KEY(`message_id`, `chunk_index`),
	FOREIGN KEY (`message_id`) REFERENCES `messages`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE TABLE `messages` (
	`id` text PRIMARY KEY NOT NULL,
	`conversation_id` text NOT NULL,
	`client_message_id` text,
	`pi_entry_id` text,
	`role` text NOT NULL,
	`status` text NOT NULL,
	`delivery` text,
	`text` text DEFAULT '' NOT NULL,
	`sent_by_device_id` text,
	`ordinal` integer NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	FOREIGN KEY (`conversation_id`) REFERENCES `conversations`(`id`) ON UPDATE no action ON DELETE cascade,
	FOREIGN KEY (`sent_by_device_id`) REFERENCES `devices`(`id`) ON UPDATE no action ON DELETE no action
);

CREATE UNIQUE INDEX `messages_conversation_ordinal_unique` ON `messages` (`conversation_id`,`ordinal`);
CREATE UNIQUE INDEX `messages_device_client_id_unique` ON `messages` (`sent_by_device_id`,`client_message_id`);
CREATE UNIQUE INDEX `messages_pi_entry_id_unique` ON `messages` (`pi_entry_id`);
CREATE INDEX `messages_conversation_created_index` ON `messages` (`conversation_id`,`created_at`);
CREATE TABLE `notification_deliveries` (
	`id` text PRIMARY KEY NOT NULL,
	`conversation_id` text NOT NULL,
	`message_id` text,
	`target_device_id` text NOT NULL,
	`channel` text NOT NULL,
	`status` text NOT NULL,
	`response_code` text,
	`created_at` text NOT NULL,
	`completed_at` text,
	FOREIGN KEY (`conversation_id`) REFERENCES `conversations`(`id`) ON UPDATE no action ON DELETE cascade,
	FOREIGN KEY (`message_id`) REFERENCES `messages`(`id`) ON UPDATE no action ON DELETE set null,
	FOREIGN KEY (`target_device_id`) REFERENCES `devices`(`id`) ON UPDATE no action ON DELETE no action
);

CREATE UNIQUE INDEX `notification_delivery_unique` ON `notification_deliveries` (`message_id`,`target_device_id`,`channel`);
CREATE TABLE `pairing_codes` (
	`id` text PRIMARY KEY NOT NULL,
	`code_hash` text NOT NULL,
	`expires_at` text NOT NULL,
	`created_at` text NOT NULL,
	`redeemed_at` text
);

CREATE UNIQUE INDEX `pairing_codes_hash_unique` ON `pairing_codes` (`code_hash`);
CREATE TABLE `repositories` (
	`id` text PRIMARY KEY NOT NULL,
	`canonical_root` text NOT NULL,
	`git_directory` text NOT NULL,
	`display_name` text NOT NULL,
	`icon_storage_key` text,
	`icon_source` text,
	`icon_fingerprint` text,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL
);

CREATE UNIQUE INDEX `repositories_root_unique` ON `repositories` (`canonical_root`);
CREATE TABLE `session_runs` (
	`id` text PRIMARY KEY NOT NULL,
	`conversation_id` text NOT NULL,
	`generation` integer NOT NULL,
	`process_id` integer,
	`state` text NOT NULL,
	`started_at` text NOT NULL,
	`ended_at` text,
	`exit_code` integer,
	`exit_signal` text,
	`error_code` text,
	FOREIGN KEY (`conversation_id`) REFERENCES `conversations`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE INDEX `session_runs_conversation_index` ON `session_runs` (`conversation_id`,`started_at`);
CREATE TABLE `sync_events` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`type` text NOT NULL,
	`conversation_id` text,
	`aggregate_id` text,
	`payload` text NOT NULL,
	`created_at` text NOT NULL,
	FOREIGN KEY (`conversation_id`) REFERENCES `conversations`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE INDEX `sync_events_conversation_index` ON `sync_events` (`conversation_id`,`id`);
CREATE TABLE `web_push_subscriptions` (
	`id` text PRIMARY KEY NOT NULL,
	`device_id` text NOT NULL,
	`endpoint_hash` text NOT NULL,
	`encrypted_subscription` text NOT NULL,
	`updated_at` text NOT NULL,
	`invalidated_at` text,
	FOREIGN KEY (`device_id`) REFERENCES `devices`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE UNIQUE INDEX `web_push_endpoint_unique` ON `web_push_subscriptions` (`endpoint_hash`);