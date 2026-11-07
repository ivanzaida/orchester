//! The pager of a task's review page (ORC-796): where a task stands in *Waiting for you*, in the list's order, which
//! tasks its arrows lead to, and what it counts once the task left the group. `cargo test -p orchester-workspace --test
//! review_pager`.

use std::{num::NonZeroU32, str::FromStr};

use orchester_core::{
  id::{RunId, TaskId},
  project::TaskKey,
  run::RunNumber,
  time::Timestamp,
};
use orchester_workspace::tasks::review_list::{Pager, Place, ReviewList, ReviewRow, ReviewState};

fn key(number: u32) -> TaskKey {
  TaskKey::from_str(&format!("KON-{number}")).expect("a task key")
}

fn row(number: u32, state: ReviewState) -> ReviewRow {
  ReviewRow {
    task: TaskId::generate(),
    key: key(number),
    title: format!("Task {number}").into(),
    tag: None,
    run: RunId::generate(),
    number: RunNumber::new(NonZeroU32::new(number).expect("from 1")),
    state,
    open_threads: 0,
    size: None,
    since: Timestamp::from_unix_millis(i64::from(number)),
  }
}

/// A list whose *Waiting for you* holds the tasks `waiting`, in that order, with `with_agent` and `approved` beside.
fn list(waiting: &[u32], with_agent: &[u32], approved: &[u32]) -> ReviewList {
  let rows = |numbers: &[u32], state: ReviewState| numbers.iter().map(|&n| row(n, state.clone())).collect();
  ReviewList {
    waiting: rows(waiting, ReviewState::Ready),
    with_agent: rows(with_agent, ReviewState::ImplementOn { comments: 1 }),
    approved_today: rows(approved, ReviewState::Merged { base: None }),
  }
}

#[test]
fn review_pager_follows_the_waiting_groups_order_with_its_neighbours() {
  let list = list(&[4, 9, 7], &[], &[]);
  let pager = Pager::of(&list, &key(9), None).expect("a waiting task has a pager");
  assert_eq!(pager.place(), Place::At(2));
  assert_eq!(pager.total(), 3);
  assert_eq!(pager.previous(), Some(&key(4)));
  assert_eq!(pager.next(), Some(&key(7)));
}

#[test]
fn review_pager_the_first_has_no_previous_and_the_last_no_next() {
  let list = list(&[4, 9, 7], &[], &[]);
  let first = Pager::of(&list, &key(4), None).expect("a pager");
  assert_eq!(
    (first.place(), first.previous(), first.next()),
    (Place::At(1), None, Some(&key(9)))
  );
  let last = Pager::of(&list, &key(7), None).expect("a pager");
  assert_eq!(
    (last.place(), last.previous(), last.next()),
    (Place::At(3), Some(&key(9)), None)
  );
}

#[test]
fn review_pager_a_lone_waiting_task_has_neither_arrow() {
  let list = list(&[5], &[], &[]);
  let pager = Pager::of(&list, &key(5), None).expect("a pager");
  assert_eq!((pager.place(), pager.total()), (Place::At(1), 1));
  assert_eq!((pager.previous(), pager.next()), (None, None));
}

#[test]
fn review_pager_a_task_of_the_other_groups_shows_none() {
  let list = list(&[4, 9], &[6], &[8]);
  assert_eq!(Pager::of(&list, &key(6), None), None, "with the agent");
  assert_eq!(Pager::of(&list, &key(8), None), None, "approved today");
  assert_eq!(Pager::of(&list, &key(99), None), None, "not listed at all");
}

#[test]
fn review_pager_a_task_that_left_the_group_counts_the_rest_from_where_it_stood() {
  let waiting = list(&[4, 9, 7], &[], &[]);
  let seen = Pager::of(&waiting, &key(9), None).expect("a pager");
  assert_eq!(seen.slot(), 1);
  // 9 was approved: it is in *Approved today* now, and 4 and 7 remain.
  let after = list(&[4, 7], &[], &[9]);
  let pager = Pager::of(&after, &key(9), Some(seen.slot())).expect("the page stays on it with a pager");
  assert_eq!((pager.place(), pager.total()), (Place::Left, 2));
  assert_eq!((pager.previous(), pager.next()), (Some(&key(4)), Some(&key(7))));
}

#[test]
fn review_pager_a_task_that_left_from_the_end_or_the_start_keeps_one_arrow() {
  let after_last = Pager::of(&list(&[4, 9], &[], &[7]), &key(7), Some(2)).expect("a pager");
  assert_eq!((after_last.previous(), after_last.next()), (Some(&key(9)), None));
  let after_first = Pager::of(&list(&[9, 7], &[], &[4]), &key(4), Some(0)).expect("a pager");
  assert_eq!((after_first.previous(), after_first.next()), (None, Some(&key(9))));
}

#[test]
fn review_pager_the_last_task_leaving_counts_none_waiting_and_has_no_arrows() {
  let pager = Pager::of(&list(&[], &[], &[5]), &key(5), Some(0)).expect("a pager");
  assert_eq!((pager.place(), pager.total()), (Place::Left, 0));
  assert_eq!((pager.previous(), pager.next()), (None, None));
}

#[test]
fn review_pager_a_task_that_left_and_whose_group_shrank_past_its_slot_clamps_to_the_end() {
  let pager = Pager::of(&list(&[4], &[], &[]), &key(7), Some(5)).expect("a pager");
  assert_eq!((pager.slot(), pager.previous(), pager.next()), (1, Some(&key(4)), None));
}
