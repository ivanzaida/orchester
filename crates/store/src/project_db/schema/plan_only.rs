//! The Orchestrator's plan-only mode (ORC-646): a conversation's *Plan only* switch, and how each person's message's
//! turn goes. A plan the Orchestrator answers with is kept as an `event` entry whose body says `"kind": "plan"`: the
//! entries' kinds are a check that changes only by making the table again, and a todo list refers to its rows.

pub(super) const MIGRATION: &str = r"
-- 1 while the conversation's Plan only switch is on: each message it sends plans, until the person switches it off.
ALTER TABLE orchestrator_chats ADD COLUMN plan_only INTEGER NOT NULL DEFAULT 0 CHECK (plan_only IN (0, 1));

-- How a person's message's turn goes: NULL as any turn, 'plan' when it only plans, 'carry_out' when it carries out a
-- plan the person approved (its body is then that plan, as the agent is told it).
ALTER TABLE orchestrator_entries ADD COLUMN mode TEXT CHECK (mode IS NULL OR mode IN ('plan', 'carry_out'));
";
