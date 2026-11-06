//! The context window Claude Code last reported for the session an Orchestrator conversation's turns continue
//! (ORC-704): each turn is a CLI of its own, which reports a window only in its result, so the next turn carries it
//! into its readings before that.

pub(super) const MIGRATION: &str = r"
-- The window the CLI last reported for the conversation's session, with the model the turn ran on; NULL until a
-- result reported one, and again once the conversation starts another session.
ALTER TABLE orchestrator_chats ADD COLUMN reported_window INTEGER CHECK (reported_window IS NULL OR reported_window > 0);
ALTER TABLE orchestrator_chats ADD COLUMN reported_window_model TEXT
  CHECK ((reported_window_model IS NULL) = (reported_window IS NULL));
";
