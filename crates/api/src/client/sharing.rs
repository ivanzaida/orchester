//! The shared projects' endpoints (`docs/architecture/server.md`, "Endpoints"): finding a project
//! by its slug and linking a checkout to it, its members and their roles, its tasks and their changes under a revision,
//! claims, the run progress report, the heartbeat and the project's tags. Every call sends the token as the bearer.

use orchester_core::{
  id::{ProjectId, TagId, TaskId, UserId},
  secret::SecretValue,
};
use reqwest::Method;

use super::{Client, ClientError, bearer};
use crate::{
  ClaimTake, Heartbeat, HeartbeatResponse, Member, MemberChanges, MemberCreate, MemberResponse, Members, ProjectCreate,
  ProjectCreated, ProjectLink, ProjectLinked, ProjectResponse, Projects, RunReport, SharedProject, SharedTag,
  SharedTask, TagMerge, TagName, TagResponse, Tags, TagsChanged, TaskPatch, TaskResponse, Tasks, TasksCreate,
  TasksCreated, TasksImport, TasksPage,
};

impl Client {
  /// `GET /projects`: the projects the person is a member of.
  pub async fn projects(&self, token: &SecretValue) -> Result<Vec<SharedProject>, ClientError> {
    let response: Projects = self
      .call(bearer(self.request(Method::GET, "/projects")?, token))
      .await?;
    Ok(response.projects)
  }

  /// `GET /projects?slug=`: the project at the address ending in `slug`. `403 not_member` names the project and its
  /// workspace admins when the person is not a member; `404` when there is none.
  pub async fn project_by_slug(&self, token: &SecretValue, slug: &str) -> Result<SharedProject, ClientError> {
    let mut url = self.address.endpoint("/projects")?;
    url.query_pairs_mut().append_pair("slug", slug);
    let response: ProjectResponse = self.call(bearer(self.http.get(url), token)).await?;
    Ok(response.project)
  }

  /// `POST /projects/{id}/link`: a checkout linked to the project uploads its tasks; into a project with no tasks they
  /// keep their numbers. A repeated key returns the first answer.
  pub async fn link_project(
    &self,
    token: &SecretValue,
    project: ProjectId,
    link: &ProjectLink,
  ) -> Result<ProjectLinked, ClientError> {
    let path = format!("/projects/{project}/link");
    self.call(bearer(self.json(Method::POST, &path, link)?, token)).await
  }

  /// API 2's `POST /projects` by repository, which an API 3 server refuses with `400` (see [`ProjectCreate`]); kept
  /// while the frozen `crates/desktop`'s tests call it.
  pub async fn create_project(
    &self,
    token: &SecretValue,
    create: &ProjectCreate,
  ) -> Result<ProjectCreated, ClientError> {
    self
      .call(bearer(self.json(Method::POST, "/projects", create)?, token))
      .await
  }

  pub async fn project(&self, token: &SecretValue, project: ProjectId) -> Result<SharedProject, ClientError> {
    let path = format!("/projects/{project}");
    let response: ProjectResponse = self.call(bearer(self.request(Method::GET, &path)?, token)).await?;
    Ok(response.project)
  }

  /// The project's members and the server's admins.
  pub async fn members(&self, token: &SecretValue, project: ProjectId) -> Result<Members, ClientError> {
    let path = format!("/projects/{project}/members");
    self.call(bearer(self.request(Method::GET, &path)?, token)).await
  }

  pub async fn add_member(
    &self,
    token: &SecretValue,
    project: ProjectId,
    member: &MemberCreate,
  ) -> Result<Member, ClientError> {
    let path = format!("/projects/{project}/members");
    let response: MemberResponse = self
      .call(bearer(self.json(Method::POST, &path, member)?, token))
      .await?;
    Ok(response.member)
  }

  /// `PATCH …/members/{user}`: the last workspace admin cannot become a member (`409 last_workspace_admin`).
  pub async fn change_member(
    &self,
    token: &SecretValue,
    project: ProjectId,
    user: UserId,
    changes: &MemberChanges,
  ) -> Result<Member, ClientError> {
    let path = format!("/projects/{project}/members/{user}");
    let response: MemberResponse = self
      .call(bearer(self.json(Method::PATCH, &path, changes)?, token))
      .await?;
    Ok(response.member)
  }

  /// `DELETE …/members/{user}`: by a workspace admin, server admin or the member themself (Leave); their claims are
  /// released.
  pub async fn remove_member(&self, token: &SecretValue, project: ProjectId, user: UserId) -> Result<(), ClientError> {
    let path = format!("/projects/{project}/members/{user}");
    self.send(bearer(self.request(Method::DELETE, &path)?, token)).await
  }

  /// `GET …/tasks`: every task of the project and the event cursor the list is current at.
  pub async fn tasks(&self, token: &SecretValue, project: ProjectId) -> Result<Tasks, ClientError> {
    let path = format!("/projects/{project}/tasks");
    self.call(bearer(self.request(Method::GET, &path)?, token)).await
  }

  /// `GET …/tasks?after=`: the tasks numbered above `after`, lowest first, one page at a time; `next` asks for the
  /// following page. A server that does not know the parameter answers every task as one page.
  pub async fn tasks_page(
    &self,
    token: &SecretValue,
    project: ProjectId,
    after: u32,
  ) -> Result<TasksPage, ClientError> {
    let mut url = self.address.endpoint(&format!("/projects/{project}/tasks"))?;
    url.query_pairs_mut().append_pair("after", &after.to_string());
    self.call(bearer(self.http.get(url), token)).await
  }

  /// `GET …/tasks/{task}`: one task as the server has it now, e.g. after a delta the cache could not take.
  pub async fn task(&self, token: &SecretValue, project: ProjectId, task: TaskId) -> Result<SharedTask, ClientError> {
    let path = format!("/projects/{project}/tasks/{task}");
    let response: TaskResponse = self.call(bearer(self.request(Method::GET, &path)?, token)).await?;
    Ok(response.task)
  }

  /// `POST …/tasks`: new tasks with consecutive numbers; a repeated batch key returns the first ones.
  pub async fn create_tasks(
    &self,
    token: &SecretValue,
    project: ProjectId,
    create: &TasksCreate,
  ) -> Result<TasksCreated, ClientError> {
    let path = format!("/projects/{project}/tasks");
    self.call(bearer(self.json(Method::POST, &path, create)?, token)).await
  }

  /// `POST …/tasks/import`: uploads local tasks with new numbers, their ids and former keys kept
  /// (Join, *Share task*); a repeated batch key returns the first ones.
  pub async fn import_tasks(
    &self,
    token: &SecretValue,
    project: ProjectId,
    import: &TasksImport,
  ) -> Result<TasksCreated, ClientError> {
    let path = format!("/projects/{project}/tasks/import");
    self.call(bearer(self.json(Method::POST, &path, import)?, token)).await
  }

  /// `PATCH …/tasks/{task}`: the changes, made on the revision read; `409 revision_conflict` carries
  /// the current task.
  pub async fn update_task(
    &self,
    token: &SecretValue,
    project: ProjectId,
    task: TaskId,
    patch: &TaskPatch,
  ) -> Result<SharedTask, ClientError> {
    let path = format!("/projects/{project}/tasks/{task}");
    let response: TaskResponse = self
      .call(bearer(self.json(Method::PATCH, &path, patch)?, token))
      .await?;
    Ok(response.task)
  }

  /// `POST …/tasks/{task}/claim`: takes the claim if the task has none, or refreshes the caller's;
  /// `409 claim_held` names the holder.
  pub async fn claim(
    &self,
    token: &SecretValue,
    project: ProjectId,
    task: TaskId,
    take: &ClaimTake,
  ) -> Result<SharedTask, ClientError> {
    let path = format!("/projects/{project}/tasks/{task}/claim");
    let response: TaskResponse = self.call(bearer(self.json(Method::POST, &path, take)?, token)).await?;
    Ok(response.task)
  }

  /// `DELETE …/tasks/{task}/claim`: the holder lets the task go.
  pub async fn release_claim(
    &self,
    token: &SecretValue,
    project: ProjectId,
    task: TaskId,
  ) -> Result<SharedTask, ClientError> {
    let path = format!("/projects/{project}/tasks/{task}/claim");
    let response: TaskResponse = self.call(bearer(self.request(Method::DELETE, &path)?, token)).await?;
    Ok(response.task)
  }

  /// `PUT …/tasks/{task}/run`: the holder reports their run; `merged` completes the claim and `cancelled` releases
  /// it. `409 not_holder` names the claim of another member. A server older than the report has no such route and
  /// answers `404 not_found` or `405 method_not_allowed`.
  pub async fn report_run(
    &self,
    token: &SecretValue,
    project: ProjectId,
    task: TaskId,
    report: &RunReport,
  ) -> Result<SharedTask, ClientError> {
    let path = format!("/projects/{project}/tasks/{task}/run");
    let response: TaskResponse = self.call(bearer(self.json(Method::PUT, &path, report)?, token)).await?;
    Ok(response.task)
  }

  /// `POST /heartbeat`: keeps the caller's claims of `tasks` fresh (at most 256 ids) and returns
  /// those they lost.
  pub async fn heartbeat(&self, token: &SecretValue, heartbeat: &Heartbeat) -> Result<HeartbeatResponse, ClientError> {
    self
      .call(bearer(self.json(Method::POST, "/heartbeat", heartbeat)?, token))
      .await
  }

  pub async fn tags(&self, token: &SecretValue, project: ProjectId) -> Result<Vec<SharedTag>, ClientError> {
    let path = format!("/projects/{project}/tags");
    let response: Tags = self.call(bearer(self.request(Method::GET, &path)?, token)).await?;
    Ok(response.tags)
  }

  pub async fn create_tag(
    &self,
    token: &SecretValue,
    project: ProjectId,
    name: &TagName,
  ) -> Result<SharedTag, ClientError> {
    let path = format!("/projects/{project}/tags");
    let response: TagResponse = self.call(bearer(self.json(Method::POST, &path, name)?, token)).await?;
    Ok(response.tag)
  }

  pub async fn rename_tag(
    &self,
    token: &SecretValue,
    project: ProjectId,
    tag: TagId,
    name: &TagName,
  ) -> Result<SharedTag, ClientError> {
    let path = format!("/projects/{project}/tags/{tag}");
    let response: TagResponse = self.call(bearer(self.json(Method::PATCH, &path, name)?, token)).await?;
    Ok(response.tag)
  }

  /// `POST …/tags/{tag}/merge`: every task of `tag` gets `into`; returns the tags and the rewritten
  /// tasks.
  pub async fn merge_tag(
    &self,
    token: &SecretValue,
    project: ProjectId,
    tag: TagId,
    into: &TagMerge,
  ) -> Result<TagsChanged, ClientError> {
    let path = format!("/projects/{project}/tags/{tag}/merge");
    self.call(bearer(self.json(Method::POST, &path, into)?, token)).await
  }

  /// `DELETE …/tags/{tag}`: removes the tag from every task; returns the tags and the rewritten tasks.
  pub async fn delete_tag(
    &self,
    token: &SecretValue,
    project: ProjectId,
    tag: TagId,
  ) -> Result<TagsChanged, ClientError> {
    let path = format!("/projects/{project}/tags/{tag}");
    self.call(bearer(self.request(Method::DELETE, &path)?, token)).await
  }
}
