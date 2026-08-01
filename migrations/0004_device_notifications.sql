CREATE TABLE `agent_cycles` (
    `id` text PRIMARY KEY NOT NULL,
    `conversation_id` text NOT NULL,
    `started_by_message_id` text NOT NULL,
    `last_interaction_message_id` text NOT NULL,
    `target_device_id` text,
    `state` text NOT NULL,
    `started_at` text NOT NULL,
    `updated_at` text NOT NULL,
    `completed_at` text,
    FOREIGN KEY (`conversation_id`) REFERENCES `conversations`(`id`) ON UPDATE no action ON DELETE cascade,
    FOREIGN KEY (`started_by_message_id`) REFERENCES `messages`(`id`) ON UPDATE no action ON DELETE cascade,
    FOREIGN KEY (`last_interaction_message_id`) REFERENCES `messages`(`id`) ON UPDATE no action ON DELETE cascade,
    FOREIGN KEY (`target_device_id`) REFERENCES `devices`(`id`) ON UPDATE no action ON DELETE no action
);

CREATE UNIQUE INDEX `agent_cycles_active_conversation_unique`
    ON `agent_cycles` (`conversation_id`)
    WHERE `state` = 'active';
CREATE INDEX `agent_cycles_conversation_started_index`
    ON `agent_cycles` (`conversation_id`, `started_at`);

ALTER TABLE `dispatches` ADD COLUMN `cycle_id` text REFERENCES `agent_cycles`(`id`) ON UPDATE no action ON DELETE set null;
ALTER TABLE `notification_deliveries` ADD COLUMN `cycle_id` text REFERENCES `agent_cycles`(`id`) ON UPDATE no action ON DELETE cascade;
ALTER TABLE `notification_deliveries` ADD COLUMN `attempts` integer DEFAULT 0 NOT NULL;

CREATE UNIQUE INDEX `notification_delivery_cycle_unique`
    ON `notification_deliveries` (`cycle_id`, `channel`);
CREATE UNIQUE INDEX `apns_device_environment_topic_unique`
    ON `apns_registrations` (`device_id`, `environment`, `bundle_id`);
