CREATE TABLE `agent_task_lists` (
	`id` text PRIMARY KEY NOT NULL,
	`conversation_id` text NOT NULL,
	`title` text,
	`revision` integer NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	FOREIGN KEY (`conversation_id`) REFERENCES `conversations`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE UNIQUE INDEX `agent_task_lists_conversation_unique`
	ON `agent_task_lists` (`conversation_id`);

CREATE TABLE `agent_tasks` (
	`id` text PRIMARY KEY NOT NULL,
	`task_list_id` text NOT NULL,
	`sequence` integer NOT NULL,
	`text` text NOT NULL,
	`status` text NOT NULL CHECK (`status` IN ('pending', 'in_progress', 'completed', 'blocked', 'skipped')),
	`note` text,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	FOREIGN KEY (`task_list_id`) REFERENCES `agent_task_lists`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE UNIQUE INDEX `agent_tasks_list_sequence_unique`
	ON `agent_tasks` (`task_list_id`, `sequence`);
