//! What actually starts when a connection's program runs, for the inspectors: shown only when it
//! is not the program file itself, i.e. a Windows command script replaced by the Node.js script
//! it launches, or one that runs through `cmd.exe`. No design node shows it yet; the row reads
//! like the program path (`xJ8W9`), with the program that runs the script named in its label.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_exec::launch::{Launch, Via};

use crate::{
  display,
  ui::property::{ValueStyle, styled_property},
};

/// The *Runs with …* property, when `launch` is not the program file itself.
pub fn launch_property(ctx: &Ctx, launch: Option<&Launch>, path_chars: usize) -> Option<Element> {
  let launch = launch?;
  let (key, script) = match launch.via() {
    Via::Direct => return None,
    Via::NodeScript { script } => ("connections.runs.nodeScript", script.as_path()),
    Via::CommandScript => ("connections.runs.commandScript", launch.program()),
  };
  Some(styled_property(
    &ctx.t(key),
    &display::shorten_path(script, path_chars),
    ValueStyle::Path,
  ))
}
