//! What a new pipeline's panel says of it besides why *Create v1* is off ([`super::reason`]): what it saves, its
//! row's line in the list, and the line over its name.

use std::sync::Arc;

use lurq::app::ctx::Ctx;

use super::Choice;
use crate::screens::pipeline_hub::model::Entry;

/// What *Create v1* saves, beside it once it can: *Saves Feature delivery copy as v1*.
pub(super) fn creates(ctx: &Ctx, name: &str) -> Arc<str> {
  ctx.t_args("pipeline_new.bar.creates", [("name", name.trim())])
}

/// The row's line: where it starts from, how many steps it has, and that nothing is saved.
pub(super) fn line(ctx: &Ctx, choice: Choice, source: Option<&Entry>, steps: usize) -> Arc<str> {
  let count = [("steps", steps.to_string())];
  match (choice, source) {
    (Choice::Copy, Some(source)) => ctx.t_args("pipeline_new.line.copy", [("name", source.name.to_string())]),
    (Choice::Copy, None) => ctx.t("pipeline_new.line.copyNone"),
    (Choice::Starter, _) => ctx.t_args("pipeline_new.line.starter", count),
    (Choice::Empty, _) if steps == 0 => ctx.t("pipeline_new.line.empty"),
    (Choice::Empty, _) if steps == 1 => ctx.t("pipeline_new.line.emptyOne"),
    (Choice::Empty, _) => ctx.t_args("pipeline_new.line.emptySteps", count),
  }
}

/// What the line over the name says: that it is new, and what it copies.
pub(super) fn what(ctx: &Ctx, source: Option<&Entry>) -> String {
  let copied = source.and_then(|source| source.number.map(|version| (source, version)));
  match copied {
    Some((source, version)) => ctx.t_args(
      "pipeline_new.whatCopy",
      [("name", source.name.to_string()), ("version", version.to_string())],
    ),
    None => ctx.t("pipeline_new.what"),
  }
  .to_string()
}
