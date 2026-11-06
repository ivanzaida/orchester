//! Review (`Review - v2` `j1Drb`): the tasks whose run finished, and a task's review. Review is no longer a hub of
//! tabs: the list is [`list`] (ORC-794) and a task's review is [`task`] (ORC-795). The Merge queue, Pull requests and
//! Merges keep their own routes until their tasks remove them.

mod list;
pub(super) mod task;

pub(super) use self::list::screen as list;
