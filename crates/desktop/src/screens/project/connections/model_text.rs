//! How a native connection's models read (04.H4 `lJG42`, 04.H6 `zHowy`): where each comes from, its
//! limits and its efforts.

use std::{num::NonZeroU32, sync::Arc};

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::{ModelSource, NativeModel},
  time::Timestamp,
};

use crate::{display, ui::message_text};

/// *From the endpoint · 5m ago*, *From the preset* or *Added by hand*.
pub fn source(ctx: &Ctx, source: ModelSource) -> Arc<str> {
  match source {
    ModelSource::Endpoint { at } => ctx.t_args(
      "connections.models.source.endpoint",
      [("time", &*message_text(ctx, &display::elapsed(Timestamp::now(), at)))],
    ),
    ModelSource::Preset => ctx.t("connections.models.source.preset"),
    ModelSource::Manual => ctx.t("connections.models.source.manual"),
  }
}

/// The limits that are known, e.g. *200K context · 64K output*; `None` when neither is.
pub fn limits(ctx: &Ctx, model: &NativeModel) -> Option<Arc<str>> {
  let context = model
    .context_window()
    .map(|tokens| ctx.t_args("connections.models.context", [("tokens", count(tokens).as_str())]));
  let output = model
    .max_output()
    .map(|tokens| ctx.t_args("connections.models.maxOutput", [("tokens", count(tokens).as_str())]));
  match (context, output) {
    (Some(context), Some(output)) => Some(ctx.t_args(
      "connections.models.joined",
      [("first", &*context), ("second", &*output)],
    )),
    (one, other) => one.or(other),
  }
}

/// The model's efforts in order, e.g. *low, medium, high*, or that it takes none.
pub fn efforts(ctx: &Ctx, model: &NativeModel) -> Arc<str> {
  if model.efforts().is_empty() {
    return ctx.t("connections.models.noEfforts");
  }
  let separator = ctx.t("common.listSeparator");
  model
    .efforts()
    .iter()
    .map(|effort| effort.as_str())
    .collect::<Vec<_>>()
    .join(&separator)
    .into()
}

/// Source, limits and efforts in one line, as a model row reads under its id.
pub fn summary(ctx: &Ctx, model: &NativeModel) -> Arc<str> {
  let mut parts = vec![source(ctx, model.source())];
  parts.extend(limits(ctx, model));
  parts.push(efforts(ctx, model));
  let separator = ctx.t("connections.models.separator");
  parts.join(&*separator).into()
}

/// A token count as the design writes it: whole thousands and millions short (`200K`, `1M`), any
/// other count in full. A number, not words, so it is not translated.
pub fn count(tokens: NonZeroU32) -> String {
  let tokens = tokens.get();
  match tokens {
    _ if tokens.is_multiple_of(1_000_000) => format!("{}M", tokens / 1_000_000),
    _ if tokens.is_multiple_of(1_000) => format!("{}K", tokens / 1_000),
    _ => tokens.to_string(),
  }
}
