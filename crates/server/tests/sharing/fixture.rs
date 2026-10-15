use orchester_api::{
  ApiError, Credential, ErrorCode, ProjectCreated, SharedTag, SharedTask, TagResponse, TaskResponse, TasksCreated,
};
use orchester_core::id::{IdempotencyKey, ProjectId};
use serde_json::{Value, json};

use crate::support::{Response, TestServer};

pub fn key() -> IdempotencyKey {
  IdempotencyKey::generate()
}
pub fn content(title: &str) -> Value {
  json!({"title":title,"brief":"Implement the change","criteria":["Works"],"pipeline":"feature","limits":{"budget_tokens":1000,"max_duration_ms":60000}})
}
pub fn upload(number: u32) -> Value {
  let mut value = content("Uploaded task");
  value["id"] = json!(orchester_core::id::TaskId::generate());
  value["number"] = json!(number);
  value["status"] = json!("draft");
  value["created_at"] = json!(123);
  value["claim"] = json!(false);
  value["has_run"] = json!(false);
  value
}

pub async fn project(server: &TestServer, token: &Credential, tasks: Vec<Value>) -> ProjectCreated {
  let slug = format!("p-{}", key());
  let response = server
    .request(
      "POST",
      "/projects",
      Some(token),
      Some(json!({"idempotency_key":key(),"name":"Shared project","slug":slug,"key":"ORC","remote_hint":"host.example/owner/repo","tasks":tasks})),
    )
    .await;
  assert_eq!(response.status, 201);
  response.json()
}

pub fn path(project: ProjectId) -> String {
  format!("/projects/{project}")
}
pub fn task_path(project: ProjectId, task: &SharedTask) -> String {
  format!("{}/tasks/{}", path(project), task.id)
}
pub async fn add_member(server: &TestServer, token: &Credential, project: ProjectId, name: &str, role: &str) {
  assert_eq!(
    server
      .request(
        "POST",
        &format!("{}/members", path(project)),
        Some(token),
        Some(json!({"username":name,"role":role}))
      )
      .await
      .status,
    201
  );
}
pub async fn create_tasks(server: &TestServer, token: &Credential, project: ProjectId, count: usize) -> TasksCreated {
  let response = server
    .request(
      "POST",
      &format!("{}/tasks", path(project)),
      Some(token),
      Some(
        json!({"idempotency_key":key(),"tasks":(0..count).map(|i|content(&format!("Task {i}"))).collect::<Vec<_>>()}),
      ),
    )
    .await;
  assert_eq!(response.status, 201);
  response.json()
}
pub async fn patch(
  server: &TestServer,
  token: &Credential,
  project: ProjectId,
  task: &SharedTask,
  changes: Value,
) -> Response {
  server
    .request(
      "PATCH",
      &task_path(project, task),
      Some(token),
      Some(json!({"revision":task.revision,"changes":changes})),
    )
    .await
}
pub fn changed(response: &Response) -> SharedTask {
  assert_eq!(response.status, 200);
  response.json::<TaskResponse>().task
}
pub fn error(response: &Response, status: u16, code: ErrorCode) {
  assert_eq!(response.status, status);
  assert_eq!(response.json::<ApiError>().error, code);
}
pub fn assert_audit(server: &TestServer, action: &str, count: i64) {
  let actual: i64 = server
    .database()
    .query_row("SELECT COUNT(*) FROM audit_events WHERE action=?1", [action], |row| {
      row.get(0)
    })
    .expect("audit count");
  assert_eq!(actual, count);
}
pub async fn tag(server: &TestServer, token: &Credential, project: ProjectId, name: &str) -> SharedTag {
  let response = server
    .request(
      "POST",
      &format!("{}/tags", path(project)),
      Some(token),
      Some(json!({"name":name})),
    )
    .await;
  assert_eq!(response.status, 201);
  response.json::<TagResponse>().tag
}
