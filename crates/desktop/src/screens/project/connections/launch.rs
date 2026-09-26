//! What actually starts when a connection's program runs, for the inspectors: shown only when it
//! is not the program file itself, i.e. a Windows command script replaced by the Node.js script
//! it launches, or one that runs through `cmd.exe`. No design node shows it yet.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_exec::launch::{Launch, Via};

use crate::{display, ui::property::property};

/// The *Runs* property, when `launch` is not the program file itself.
pub fn launch_property(ctx: &Ctx, launch: Option<&Launch>, path_chars: usize) -> Option<Element> {
  let launch = launch?;
  let value = match launch.via() {
    Via::Direct => return None,
    Via::NodeScript { script } => ctx.t_args(
      "connections.runs.nodeScript",
      [("script", display::shorten_path(script, path_chars).as_str())],
    ),
    Via::CommandScript => ctx.t_args(
      "connections.runs.commandScript",
      [("script", display::shorten_path(launch.program(), path_chars).as_str())],
    ),
  };
  Some(property(&ctx.t("connections.property.runs"), &value))
}
