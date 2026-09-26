//! What actually starts when a connection's program runs, for the inspectors: shown only when it
//! is not the program file itself. One rule for both inspectors: *Runs with* names the program
//! that starts (`cmd.exe` for a command script, `Node.js` for an npm launcher), and *Script*
//! follows only when that program runs a file other than the program file, i.e. the Node.js
//! script of a launcher, in the program path's style (`xJ8W9`). No design node shows it yet.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_exec::launch::{Launch, Via};

use crate::{
  display,
  ui::property::{ValueStyle, property, styled_property},
};

/// Product names of the programs that run a launcher script; identifiers, not translated.
const NODE: &str = "Node.js";
const CMD: &str = "cmd.exe";

/// The *Runs with* and *Script* properties, when `launch` is not the program file itself.
pub fn launch_properties(ctx: &Ctx, launch: Option<&Launch>, path_chars: usize) -> Vec<Element> {
  let Some(launch) = launch else {
    return Vec::new();
  };
  let runs = |program: &str| property(&ctx.t("connections.property.runs"), program);
  match launch.via() {
    Via::Direct => Vec::new(),
    Via::CommandScript => vec![runs(CMD)],
    Via::NodeScript { script } => vec![
      runs(NODE),
      styled_property(
        &ctx.t("connections.property.script"),
        &display::shorten_path(script, path_chars),
        ValueStyle::Path,
      ),
    ],
  }
}
