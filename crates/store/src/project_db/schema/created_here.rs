//! A version marks whether its resource was created here as its first version (`ProjectDb::create_published`): such a
//! resource never had a draft, so one the team server refuses when it is sent later (after a restart, or the server
//! back) can be taken back whole. The first version of a draft published for the first time is not marked: taking it
//! back would delete the draft's work.

pub(super) const MIGRATION: &str = r"
ALTER TABLE shared_publishes ADD COLUMN created INTEGER NOT NULL DEFAULT 0 CHECK (created IN (0, 1));
";
