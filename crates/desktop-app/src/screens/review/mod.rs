//! Review (`Review - v2` `j1Drb`): the tasks whose run finished, and a task's review. Review is no longer a hub of
//! tabs: the list is [`list`] (ORC-794) and a task's review is [`task`] (ORC-795).

mod list;
pub(super) mod task;

pub(super) use self::list::screen as list;
