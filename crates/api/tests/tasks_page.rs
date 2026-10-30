//! One page of a project's tasks (`GET …/tasks?after=`, ORC-291): a server that does not know the parameter answers
//! the whole list, which reads as one page, and a page's `next` and `total` keep their names on the wire.
//!
//! `cargo test -p orchester-api --test tasks_page tasks_page`

use orchester_api::TasksPage;
use serde_json::json;

#[test]
fn tasks_page_reads_a_server_without_pages() {
  let page: TasksPage = serde_json::from_value(json!({ "tasks": [], "cursor": 7 })).expect("the whole list");
  assert_eq!(page.cursor, 7);
  assert_eq!(page.next, None);
  assert_eq!(page.total, None);
  assert_eq!(
    serde_json::to_value(&page).expect("serialize"),
    json!({ "tasks": [], "cursor": 7 }),
    "the last page names neither"
  );
}

#[test]
fn tasks_page_round_trips_next_and_total() {
  let body = json!({ "tasks": [], "cursor": 7, "next": 64, "total": 212 });
  let page: TasksPage = serde_json::from_value(body.clone()).expect("a page");
  assert_eq!(page.next, Some(64));
  assert_eq!(page.total, Some(212));
  assert_eq!(serde_json::to_value(&page).expect("serialize"), body);
}
