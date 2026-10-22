//! `get_delivery` (ORC-50): a project's throughput, cycle time, where time goes, cumulative flow and the forecasts of
//! its queued work and open modules over its last days, with a link the router opens as the Delivery view; a module
//! narrows the forecasts to its own beside the queue's; and `get_module` gives the module's forecast too.

use orchester_app::routes::{ProjectPage, Route};
use orchester_workspace::delivery::DeliveryPeriod;
use serde_json::{Value, json};

use super::Served;

fn routed(link: &Value) -> Route {
  let path = link.as_str().unwrap_or_else(|| panic!("a link: {link}"));
  Route::parse(path).unwrap_or_else(|error| panic!("{path} reads as a route: {error}"))
}

#[test]
fn domain_delivery_answers_the_series_and_forecasts_with_the_views_link() {
  let (mut served, seeded) = Served::seeded("empty");
  served.ok("create_module", &json!({ "project": "Kontur", "name": "Checkout" }));
  served.ok(
    "create_task",
    &json!({ "project": "Kontur", "title": "Card form", "module": "Checkout" }),
  );

  let delivery = served.ok("get_delivery", &json!({ "project": "Kontur", "days": 90 }));
  assert_eq!(delivery["unit"], "tasks", "{delivery}");
  assert_eq!(delivery["throughput"]["by"], "week", "{delivery}");
  assert_eq!(delivery["cumulative_flow"]["days"].as_array().map(Vec::len), Some(90));
  assert!(delivery["cycle_time"]["tasks"].is_array(), "{delivery}");
  assert!(delivery["where_time_goes"]["steps"].is_array(), "{delivery}");
  let forecasts = delivery["forecasts"].as_array().expect("forecasts");
  assert_eq!(forecasts[0]["work"]["kind"], "queued_tasks", "{delivery}");
  let module = forecasts
    .iter()
    .find(|forecast| forecast["work"]["name"] == "Checkout")
    .unwrap_or_else(|| panic!("the open module's forecast: {delivery}"));
  assert_eq!(module["remaining"], 1);
  assert_eq!(
    module["forecast"]["state"], "not_enough_history",
    "nothing was merged yet: {delivery}"
  );
  assert_eq!(
    routed(&delivery["link"]),
    Route::Project(seeded.project, ProjectPage::Delivery(DeliveryPeriod::Quarter))
  );

  let narrowed = served.ok("get_delivery", &json!({ "project": "Kontur", "module": "checkout" }));
  let kinds: Vec<&Value> = narrowed["forecasts"]
    .as_array()
    .expect("forecasts")
    .iter()
    .map(|forecast| &forecast["work"]["kind"])
    .collect();
  assert_eq!(kinds, [&json!("queued_tasks"), &json!("module")], "{narrowed}");
  assert_eq!(narrowed["throughput"]["by"], "day", "30 days by default, by day");

  let page = served.ok("get_module", &json!({ "project": "Kontur", "module": "Checkout" }));
  assert_eq!(page["forecast"]["state"], "not_enough_history", "{page}");
  served.refused("get_delivery", &json!({ "project": "Kontur", "days": 0 }));
}
