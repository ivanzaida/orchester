//! `navigate` to Review's routes: the list and a task's review, which take `key` back from the route they answer.
//! `cargo test -p orchester-app --test navigate_review navigate_`.

mod support;

use orchester_app::mcp::tools::navigate;
use serde_json::json;
use support::Window;

const PROJECT: &str = "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b";

#[test]
fn navigate_opens_review_and_a_tasks_review() {
  let mut window = Window::open();
  let navigation = window.navigation();
  let root = format!("/project/{PROJECT}");
  for (args, path) in [
    (
      json!({ "route": "review", "project": PROJECT }),
      format!("{root}/review"),
    ),
    (
      json!({ "route": "review", "project": PROJECT, "key": "KON-12" }),
      format!("{root}/review/KON-12"),
    ),
  ] {
    let answer = navigate(&navigation, &args).expect("a valid route");
    assert_eq!(answer["route"]["path"], json!(path), "{args}");
    assert_eq!(answer["route"]["name"], json!("review"), "{args}");
    let mut again = answer["route"]["params"].as_object().expect("params").clone();
    again.insert("route".to_owned(), answer["route"]["name"].clone());
    let repeated = navigate(&navigation, &json!(again)).expect("its params name the same route");
    assert_eq!(repeated["route"]["path"], json!(path), "{again:?}");
    window.until("Review", |window| window.texts("screen-review").is_some());
  }
  let refused = navigate(
    &navigation,
    &json!({ "route": "review", "project": PROJECT, "tab": "all" }),
  )
  .expect_err("review takes no tab");
  assert_eq!(refused.code(), "unexpected_param");
}
