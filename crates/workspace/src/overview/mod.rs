//! The project's Overview as data (`Overview · redesign` `iRvT6`): what waits for the person is
//! [`crate::tasks::attention`], the rest is here, each section from the project's tasks as the feed read them or from
//! state the session already follows. Nothing is read per frame: the page reads these when its inputs change.
//!
//! - [`at_work`]: the runs at work, with their step, account and model;
//! - [`queue`]: the queued tasks in start order, each with why it waits;
//! - [`limits`]: the accounts near or at a plan limit, and those whose window reset since a reading said so;
//! - [`landed`]: the runs merged today and earlier, and the merge queue's state;
//! - [`delivery`]: the open modules' progress and forecasts, and the queued work's;
//! - [`start`]: a new project's getting-started steps.

pub mod at_work;
pub mod delivery;
pub mod landed;
pub mod limits;
pub mod queue;
pub mod start;

/// How many lines a section shows before its *Show all* (05 `JzNRO`, Rules).
pub const WAITS_SHOWN: usize = 6;
pub const AT_WORK_SHOWN: usize = 6;
pub const QUEUE_SHOWN: usize = 5;
pub const TODAY_SHOWN: usize = 5;
pub const EARLIER_SHOWN: usize = 3;
pub const MODULES_SHOWN: usize = 3;
