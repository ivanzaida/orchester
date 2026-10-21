//! Modules through the domain tools (ORC-48): `create_module` and `update_module` make and change one, `create_task`
//! and `update_task` put tasks in it and take them out, and `list_modules` and `get_module` read its progress, its
//! tasks and its burndown by day, with links the router opens.

use serde_json::json;

use super::Served;

#[test]
fn domain_modules_group_tasks_and_read_their_burndown() {
  let (mut served, seeded) = Served::seeded("empty");
  let made = served.ok(
    "create_module",
    &json!({ "project": "Kontur", "name": "Checkout", "description": "Pay by card", "target_date": "2099-12-31" }),
  );
  let module = made["module"]["id"].as_str().expect("an id").to_owned();
  assert_eq!(made["module"]["state"], "open", "{made}");
  assert_eq!(
    made["module"]["link"],
    format!("/project/{}/modules/{module}", seeded.project)
  );
  let duplicate = served.refused("create_module", &json!({ "project": "Kontur", "name": "checkout" }));
  assert_eq!(duplicate["code"], "invalid_module", "{duplicate}");
  let reversed = served.refused(
    "create_module",
    &json!({ "project": "Kontur", "name": "Late", "start_date": "2026-10-10", "target_date": "2026-10-01" }),
  );
  assert_eq!(reversed["code"], "invalid_module", "{reversed}");

  let task = served.ok(
    "create_task",
    &json!({ "project": "Kontur", "title": "Card form", "module": "Checkout" }),
  );
  assert_eq!(task["task"]["module"], module.as_str(), "{task}");
  let other = served.ok("create_task", &json!({ "project": "Kontur", "title": "Receipts" }));
  let key = other["task"]["key"].clone();
  assert!(other["task"]["module"].is_null(), "{other}");
  served.ok(
    "update_task",
    &json!({ "project": "Kontur", "key": key, "module": module }),
  );
  let missing = served.refused(
    "update_task",
    &json!({ "project": "Kontur", "key": key, "module": "Nope" }),
  );
  assert_eq!(missing["code"], "module_not_found", "{missing}");

  let listed = served.ok("list_modules", &json!({ "project": "Kontur" }));
  let entry = &listed["modules"][0];
  assert_eq!(entry["tasks"], 2, "{listed}");
  assert_eq!(entry["progress"]["unit"], "tasks");
  assert_eq!(
    (entry["progress"]["scope"].clone(), entry["progress"]["done"].clone()),
    (json!(2), json!(0))
  );

  let page = served.ok("get_module", &json!({ "project": "Kontur", "module": "Checkout" }));
  assert_eq!(page["tasks"].as_array().map(Vec::len), Some(2), "{page}");
  let days = page["burndown"]["days"].as_array().expect("days");
  assert_eq!(days[0]["remaining"], 2, "{page}");
  assert_eq!(days[0]["added"], 2, "{page}");
  assert!(
    days.last().is_some_and(|day| day["remaining"].is_null()),
    "days to come have no remaining work"
  );
  assert!(
    days.iter().all(|day| day["ideal"].is_number()),
    "a target date draws the ideal line"
  );

  served.ok(
    "update_task",
    &json!({ "project": "Kontur", "key": key, "module": null }),
  );
  let closed = served.ok(
    "update_module",
    &json!({ "project": "Kontur", "module": module, "state": "closed", "target_date": null }),
  );
  assert_eq!(closed["module"]["state"], "closed", "{closed}");
  assert!(closed["module"]["target_date"].is_null(), "{closed}");
  let page = served.ok("get_module", &json!({ "project": "Kontur", "module": module }));
  assert_eq!(page["progress"]["scope"], 1, "{page}");
  assert!(
    page["burndown"]["days"][0]["ideal"].is_null(),
    "no target, no ideal line: {page}"
  );
}
