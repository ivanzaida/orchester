//! Story points through the task tools (ORC-49): `create_task` and `update_task` record the points given as
//! automation's, `get_task` and `list_tasks` show them with who set them, a person's points are never changed by a
//! tool, points off the scale are refused, and `get_story_point_calibration` reads the merged tasks.

use orchester_core::{store::TaskStore, task::estimate::StoryPoints, time::Timestamp};
use orchester_store::ProjectDb;
use serde_json::json;

use super::Served;

#[test]
fn domain_story_points_are_automations_and_never_over_a_persons() {
  let (mut served, seeded) = Served::seeded("empty");
  let created = served.ok(
    "create_task",
    &json!({ "project": "Kontur", "title": "Retry with jitter", "story_points": 5 }),
  );
  assert_eq!(created["task"]["story_points"]["points"], 5, "{created}");
  assert_eq!(created["task"]["story_points"]["set_by"], "automation", "{created}");
  let key = created["task"]["key"].as_str().expect("a key").to_owned();

  let off = served.refused(
    "update_task",
    &json!({ "project": "Kontur", "key": key, "story_points": 4 }),
  );
  assert_eq!(off["code"], "invalid_story_points", "{off}");
  let changed = served.ok(
    "update_task",
    &json!({ "project": "Kontur", "key": key, "story_points": 8 }),
  );
  assert_eq!(changed["task"]["story_points"]["points"], 8, "{changed}");

  let mut db = ProjectDb::open(&seeded.repo).expect("open");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the created task");
  db.set_person_points(task.id(), Some(StoryPoints::new(13).expect("points")), Timestamp::now())
    .expect("a person's points");
  let refused = served.refused(
    "update_task",
    &json!({ "project": "Kontur", "key": key, "story_points": 2 }),
  );
  assert_eq!(refused["code"], "persons_points", "{refused}");

  let read = served.ok("get_task", &json!({ "project": "Kontur", "key": key }));
  assert_eq!(read["story_points"]["points"], 13, "{read}");
  assert_eq!(read["story_points"]["set_by"], "person", "{read}");
  let listed = served.ok("list_tasks", &json!({ "project": "Kontur" }));
  let row = listed["tasks"]
    .as_array()
    .expect("tasks")
    .iter()
    .find(|row| row["key"] == key.as_str())
    .expect("the task's row");
  assert_eq!(row["story_points"]["points"], 13, "{row}");

  let calibration = served.ok("get_story_point_calibration", &json!({ "project": "Kontur" }));
  assert_eq!(
    calibration["merged_tasks"],
    json!([]),
    "nothing merged yet: {calibration}"
  );
  assert_eq!(calibration["per_points"], json!([]), "{calibration}");
}
