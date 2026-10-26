//! A tool's Inputs & outputs tab (`04d` `pg4y6`), read from its saved command: a command tool takes no inputs of
//! its own, runs its program with the Configuration tab's arguments, and gives its exit code — which decides the
//! step — and the latest part of what it prints, kept in the run's transcript.
use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::ToolAction;
use orchester_workspace::tools::data::ToolEntry;

use crate::{
  screens::hub::parts::{Entry, Tone, boxed, group, head, quiet, status},
  theme::HubSpacing,
};

pub(super) fn inputs(ctx: &Ctx, entry: &ToolEntry) -> Element {
  let ToolAction::Command(command) = entry.tool.action() else {
    return Column::new().into();
  };
  let codes: Vec<String> = command.exit_codes().iter().map(|code| code.to_string()).collect();
  let inputs = Entry::new("tool-inputs-none", ctx.t("tools.inputsNone")).hint(ctx.t("tools.inputsNoneDetail"));
  let exit = Entry::new("tool-output-exit", ctx.t("tools.outputExit"))
    .hint(ctx.t_args(
      "tools.outputExitDetail",
      [("codes", codes.join(", ")), ("timeout", command.timeout().secs().to_string())],
    ))
    .end(status("tool-output-exit-decides", Tone::Muted, &ctx.t("tools.outputDecides")));
  let output = Entry::new("tool-output-text", ctx.t("tools.outputText"))
    .hint(ctx.t_args(
      "tools.outputTextDetail",
      [("limit", command.output_limit().kib().to_string())],
    ))
    .end(quiet(&ctx.t("tools.outputKind")));
  Column::new()
    .id("tool-inputs-outputs")
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Sections.role())
    .child(group(
      "tool-inputs",
      head("tool-inputs", &ctx.t("tools.inputsGroup"), None, None),
      boxed("tool-inputs-rows", [inputs.into()]),
    ))
    .child(group(
      "tool-outputs",
      head("tool-outputs", &ctx.t("tools.outputsGroup"), None, None),
      boxed("tool-outputs-rows", [exit.into(), output.into()]),
    ))
    .into()
}
