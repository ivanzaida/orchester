//! Who sent a person's message to the Orchestrator chat (ORC-96): the person, or automation driving the window
//! (lurq's synthetic input). Only the person's own message lets the chat apply a proposal it approves.

pub(super) const MIGRATION: &str = r"
-- 1: automation sent the message. 0 for every other entry, and for messages kept before the sender was (all a
-- person's then: no proposal is applied for them any more, since only the turn under way may apply one).
ALTER TABLE orchestrator_entries ADD COLUMN by_automation INTEGER NOT NULL DEFAULT 0 CHECK (by_automation IN (0, 1));
";
