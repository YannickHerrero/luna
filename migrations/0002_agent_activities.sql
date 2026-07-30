CREATE TABLE `agent_activities` (
	`id` text PRIMARY KEY NOT NULL,
	`conversation_id` text NOT NULL,
	`sequence` integer NOT NULL,
	`summary` text NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	FOREIGN KEY (`conversation_id`) REFERENCES `conversations`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE UNIQUE INDEX `agent_activities_conversation_sequence_unique`
	ON `agent_activities` (`conversation_id`, `sequence`);
