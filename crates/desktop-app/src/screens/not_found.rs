//! A path that names no screen, or names one with an identifier or a query value no route can hold: it
//! says which part of the address names nothing.

use lurq::{app::ctx::Ctx, node::Element};

use super::placeholder::centered;
use crate::{routes::RouteError, ui::icon::Icon};

pub fn screen(ctx: &mut Ctx, path: &str, error: &RouteError) -> Element {
  let title = ctx.t("shell.notFound.title");
  let body = match error {
    RouteError::NoScreen => ctx.t_args("shell.notFound.noScreen", [("path", path)]),
    RouteError::ProjectId { value } => {
      ctx.t_args("shell.notFound.projectId", [("path", path), ("value", value.as_str())])
    }
    RouteError::TaskKey { value, .. } => {
      ctx.t_args("shell.notFound.taskKey", [("path", path), ("value", value.as_str())])
    }
    RouteError::UnexpectedParam { param } => ctx.t_args(
      "shell.notFound.unexpectedParam",
      [("path", path), ("param", param.as_str())],
    ),
    RouteError::InvalidParam { param, value } => ctx.t_args(
      "shell.notFound.invalidParam",
      [("path", path), ("param", param.as_str()), ("value", value.as_str())],
    ),
  };
  centered(Icon::CircleHelp, &title, &body).id("screen-not-found").into()
}
