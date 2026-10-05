//! The shared projects' endpoints (`docs/architecture/server.md`, "Endpoints"): finding and
//! creating a project, its members and their roles, its tasks and their changes under a revision,
//! claims, the heartbeat and the project's tags. Every call sends the token as the bearer.

use orchester_core::{
  id::{ProjectId, TagId, TaskId, UserId},
  repository::RepositoryIdentity,
  secret::SecretValue,
};
use reqwest::Method;

use super::{Client, ClientError, bearer};
use crate::{
  ClaimTake, Heartbeat, HeartbeatResponse, Member, MemberChanges, MemberCreate, MemberResponse, Members, ProjectCreate,
  ProjectCreated, ProjectResponse, Projects, SharedProject, SharedTag, SharedTask, TagMerge, TagName, TagResponse,
  Tags, TagsChanged, TaskPatch, TaskResponse, Tasks, TasksCreate, TasksCreated, TasksImport,
};

impl Client {
  /// `GET /projects?remote=`: the project of the repository `remote` on the server, if there is one
  /// and the person is a member; `403 not_member` names its owners otherwise. Without `remote`, the
  /// projects the person is a member of.
  pub async fn projects(
    &self,
    token: &SecretValue,
    remote: Option<&RepositoryIdentity>,
  ) -> Result<Vec<SharedProject>, ClientError> {
    let mut url = self.address.endpoint("/projects")?;
    if let Some(remote) = remote {
      url.query_pairs_mut().append_pair("remote", remote.as_str());
    }
    let response: Projects = self.call(bearer(self.http.get(url), token)).await?;
    Ok(response.projects)
  }

  /// `POST /projects`: creates the project from the local one and uploads its tasks, numbers kept.
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

  pub async fn members(&self, token: &SecretValue, project: ProjectId) -> Result<Vec<Member>, ClientError> {
    let path = format!("/projects/{project}/members");
    let response: Members = self.call(bearer(self.request(Method::GET, &path)?, token)).await?;
    Ok(response.members)
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

  /// `PATCH …/members/{user}`: the last owner cannot become a member (`409 last_owner`).
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

  /// `DELETE …/members/{user}`: by an owner, or the member themself (Leave); their claims are
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
