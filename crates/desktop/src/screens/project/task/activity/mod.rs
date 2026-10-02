//! The parts of the agent transcript (02.I `SfXU1`) the Conversation tab shows: an entry (`N4SlQq`)
//! with its time, glyph, header and output, a step execution's header (`mDnc3`), a return or
//! restart before it (02.B7 `zyusO`), the invocation panel of its Details pane (`n0cEOG`) and the
//! live footer (`P2Qc0`). The tab itself is `super::conversation`.

mod entry;
mod first_edit;
mod footer;
mod mark;
mod mcp;
mod mcp_call;
mod native;
mod panel;
mod problem;
mod step_header;
mod toggles;

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::connection::EngineVendor;

pub(in crate::screens::project) use self::mcp_call::{category_key, subject_key};
pub(super) use self::{
  entry::{EntryProps, EntryView},
  footer::{LiveFooter, LiveFooterProps},
  mark::mark_row,
  panel::{PANEL_WIDTH as DETAILS_WIDTH, invocation_panel},
  step_header::step_header,
  toggles::{OpenRows, Toggles},
};
use crate::screens::project::connections::status::{preset_name_key, short_key as provider_short_key};

/// Who receives an agent's prompts and the model it runs on, as the design writes it ("Codex / Sol",
/// "DeepSeek / deepseek-v4-pro", "gateway.kontur.lan / lab-coder-32b"); the model alone once the
/// profile's connection is gone. Who that is follows core's `AgentConnection::vendor`.
pub(super) fn engine(ctx: &Ctx, vendor: Option<&EngineVendor>, model: &str) -> Arc<str> {
  let name = match vendor {
    Some(EngineVendor::Provider(provider)) => ctx.t(provider_short_key(*provider)),
    Some(EngineVendor::Preset(preset)) => ctx.t(preset_name_key(*preset)),
    Some(EngineVendor::Host(host)) => host.as_str().into(),
    None => return model.into(),
  };
  ctx.t_args("runs.activity.engine", [("provider", &*name), ("model", model)])
}
