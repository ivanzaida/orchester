use axum::{
  Json, Router,
  extract::{Query, State},
  http::{HeaderMap, StatusCode},
  response::{IntoResponse, Response},
  routing::get,
};
use orchester_api::*;
use orchester_core::id::{ProjectId, TagId, UserId};
use serde::Deserialize;

use super::{
  base_address,
  extract::{Auth, Id, SafeJson},
};
use crate::{
  Server, db,
  error::ServerError,
  sharing::{access, members, projects, tags},
};

pub fn routes() -> Router<Server> {
  Router::new()
    .route("/projects", get(list).post(create))
    .route("/projects/{project}", get(show).patch(patch))
    .route("/projects/{project}/link", axum::routing::post(link))
    .route("/projects/{project}/members", get(list_members).post(add_member))
    .route(
      "/projects/{project}/members/{user}",
      axum::routing::patch(change_member).delete(remove_member),
    )
    .route("/projects/{project}/tags", get(list_tags).post(add_tag))
    .route(
      "/projects/{project}/tags/{tag}",
      axum::routing::patch(rename_tag).delete(remove_tag),
    )
    .route("/projects/{project}/tags/{tag}/merge", axum::routing::post(merge_tag))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lookup {
  slug: Option<String>,
}

/// `GET /projects`: the caller's projects, or with `?slug=` the one project at that address.
async fn list(
  State(server): State<Server>,
  Auth(auth): Auth,
  headers: HeaderMap,
  query: Result<Query<Lookup>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, ServerError> {
  let Query(query) = query.map_err(|_| ErrorCode::BadRequest)?;
  let base = base_address(&server, &headers);
  let response = match query.slug {
    Some(slug) => {
      let project = server
        .snapshot(move |db, now| {
          let current = db::checked(db, &auth, now, false)?;
          projects::by_slug(db, current.user.id, &slug, &base)
        })
        .await?;
      Json(ProjectResponse { project }).into_response()
    }
    None => {
      let projects = server
        .snapshot(move |db, now| {
          let current = db::checked(db, &auth, now, false)?;
          projects::list(db, current.user.id, &base)
        })
        .await?;
      Json(Projects { projects }).into_response()
    }
  };
  Ok(response)
}

/// `POST /projects`: admins create projects by name and slug.
async fn create(
  State(server): State<Server>,
  Auth(auth): Auth,
  headers: HeaderMap,
  SafeJson(input): SafeJson<NewProject>,
) -> Result<(StatusCode, Json<ProjectCreated>), ServerError> {
  let base = base_address(&server, &headers);
  let response = server
    .write(move |db, now| {
      let current = db::checked(db, &auth, now, true)?;
      projects::create(db, current.user.id, input, &base, now)
    })
    .await?;
  Ok((StatusCode::CREATED, Json(response)))
}

async fn link(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(project): Id<ProjectId>,
  SafeJson(input): SafeJson<ProjectLink>,
) -> Result<(StatusCode, Json<ProjectLinked>), ServerError> {
  let response = server
    .write(move |db, now| {
      let current = access::member(db, &auth, project, now)?;
      projects::link(db, project, current.user.id, input, now)
    })
    .await?;
  Ok((StatusCode::CREATED, Json(response)))
}

async fn show(
  State(server): State<Server>,
  Auth(auth): Auth,
  headers: HeaderMap,
  Id(project): Id<ProjectId>,
) -> Result<Json<ProjectResponse>, ServerError> {
  let base = base_address(&server, &headers);
  let project = server
    .snapshot(move |db, now| {
      let current = access::member(db, &auth, project, now)?;
      projects::load(db, project, current.user.id, &base)
    })
    .await?;
  Ok(Json(ProjectResponse { project }))
}

async fn patch(
  State(server): State<Server>,
  Auth(auth): Auth,
  headers: HeaderMap,
  Id(project): Id<ProjectId>,
  SafeJson(input): SafeJson<ProjectChanges>,
) -> Result<Json<ProjectResponse>, ServerError> {
  let base = base_address(&server, &headers);
  let project = server
    .write(move |db, now| {
      let current = access::owner(db, &auth, project, now)?;
      projects::patch(db, project, current.user.id, input, &base, now)
    })
    .await?;
  Ok(Json(ProjectResponse { project }))
}

async fn list_members(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(project): Id<ProjectId>,
) -> Result<Json<Members>, ServerError> {
  let members = server
    .snapshot(move |db, now| {
      access::member(db, &auth, project, now)?;
      members::list(db, project)
    })
    .await?;
  Ok(Json(Members { members }))
}

async fn add_member(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(project): Id<ProjectId>,
  SafeJson(input): SafeJson<MemberCreate>,
) -> Result<(StatusCode, Json<MemberResponse>), ServerError> {
  let member = server
    .write(move |db, now| {
      let current = access::owner(db, &auth, project, now)?;
      members::add(db, project, &input, current.user.id, now)
    })
    .await?;
  Ok((StatusCode::CREATED, Json(MemberResponse { member })))
}

async fn change_member(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id((project, user)): Id<(ProjectId, UserId)>,
  SafeJson(input): SafeJson<MemberChanges>,
) -> Result<Json<MemberResponse>, ServerError> {
  let member = server
    .write(move |db, now| {
      let current = access::owner(db, &auth, project, now)?;
      members::change(db, project, user, input.role, current.user.id, now)
    })
    .await?;
  Ok(Json(MemberResponse { member }))
}

async fn remove_member(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id((project, user)): Id<(ProjectId, UserId)>,
) -> Result<StatusCode, ServerError> {
  server
    .write(move |db, now| {
      let current = if auth.user.id == user {
        access::member(db, &auth, project, now)?
      } else {
        access::owner(db, &auth, project, now)?
      };
      members::remove(db, project, user, current.user.id, now)
    })
    .await?;
  Ok(StatusCode::NO_CONTENT)
}

async fn list_tags(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(project): Id<ProjectId>,
) -> Result<Json<Tags>, ServerError> {
  let tags = server
    .snapshot(move |db, now| {
      access::member(db, &auth, project, now)?;
      tags::list(db, project)
    })
    .await?;
  Ok(Json(Tags { tags }))
}

async fn add_tag(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(project): Id<ProjectId>,
  SafeJson(input): SafeJson<TagName>,
) -> Result<(StatusCode, Json<TagResponse>), ServerError> {
  let tag = server
    .write(move |db, now| {
      access::member(db, &auth, project, now)?;
      tags::create(db, project, &input.name, now)
    })
    .await?;
  Ok((StatusCode::CREATED, Json(TagResponse { tag })))
}

async fn rename_tag(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id((project, tag)): Id<(ProjectId, TagId)>,
  SafeJson(input): SafeJson<TagName>,
) -> Result<Json<TagResponse>, ServerError> {
  let tag = server
    .write(move |db, now| {
      access::member(db, &auth, project, now)?;
      tags::rename(db, project, tag, &input.name, now)
    })
    .await?;
  Ok(Json(TagResponse { tag }))
}

async fn merge_tag(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id((project, tag)): Id<(ProjectId, TagId)>,
  SafeJson(input): SafeJson<TagMerge>,
) -> Result<Json<TagsChanged>, ServerError> {
  let response = server
    .write(move |db, now| {
      let current = access::member(db, &auth, project, now)?;
      tags::remove(db, project, tag, Some(input.into), current.user.id, now)
    })
    .await?;
  Ok(Json(response))
}

async fn remove_tag(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id((project, tag)): Id<(ProjectId, TagId)>,
) -> Result<Json<TagsChanged>, ServerError> {
  let response = server
    .write(move |db, now| {
      let current = access::member(db, &auth, project, now)?;
      tags::remove(db, project, tag, None, current.user.id, now)
    })
    .await?;
  Ok(Json(response))
}
