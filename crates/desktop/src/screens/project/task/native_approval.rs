//! What the approval dialog says about a native agent's command (04.H9 `wvh5j`, the `GbNSA` dialog at
//! 640): the shell its string goes to, the full string in a mono block that scrolls instead of
//! shortening it, the working directory and time limit, and that it runs outside any sandbox. Deny
//! does not fail the step: the agent is told and continues (decision 2).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, ScrollVertical, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::EngineVendor,
  resource::ModelId,
  run::{NativeCommandAsk, PendingToolApproval},
};

use super::activity::engine;
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, select_menu_scrollbar},
  ui::{
    icon::Icon,
    notice::Notice,
    property::{ValueStyle, property, styled_property},
  },
};

/// Width of the dialog for a native command (`coKjY` in `wvh5j`).
pub(super) const DIALOG_WIDTH: f32 = 640.0;
/// The tallest the command block grows before its string scrolls inside it (decision of 2026-10-01:
/// the string is never shortened).
const COMMAND_MAX_HEIGHT: f32 = 160.0;

/// The dialog's explanation for a native command, by whether it goes to a shell and whether the step
/// only reads.
pub(super) const fn explanation_key(native: NativeCommandAsk, read_only: bool) -> &'static str {
  match (native.shell.is_some(), read_only) {
    (true, false) => "runs.toolApproval.native.shellExplanation",
    (true, true) => "runs.toolApproval.native.shellExplanationReadOnly",
    (false, false) => "runs.toolApproval.native.explanation",
    (false, true) => "runs.toolApproval.native.explanationReadOnly",
  }
}

/// The rows under the header (`eYhOs`): agent, shell, command, working directory, time limit, the
/// sandbox notice and the scope of an approval.
pub(super) fn details(
  ctx: &Ctx,
  approval: &PendingToolApproval,
  native: NativeCommandAsk,
  (vendor, model): (Option<&EngineVendor>, Option<&ModelId>),
) -> Vec<Element> {
  let shell = native.shell.map(|shell| {
    let value = ctx.t_args(
      "runs.toolApproval.native.shellValue",
      [("shell", shell.name()), ("invocation", shell.invocation())],
    );
    styled_property(&ctx.t("runs.toolApproval.native.shell"), &value, ValueStyle::Mono)
  });
  let command = if native.shell.is_some() {
    command_block(ctx, &approval.action)
  } else {
    styled_property(&ctx.t("runs.toolApproval.command"), &approval.action, ValueStyle::Mono)
  };
  let time_limit = ctx.t_args(
    "runs.toolApproval.native.timeLimitValue",
    [("seconds", native.time_limit_seconds.to_string().as_str())],
  );
  // The agent's role, with what it runs on when that can be read: "Builder · OpenAI API / deepseek-v4-pro".
  let agent = match model {
    Some(model) => ctx.t_args(
      "runs.activity.panel.agentValue",
      [
        ("agent", approval.agent.as_str()),
        ("engine", &*engine(ctx, vendor, model.as_str())),
      ],
    ),
    None => approval.agent.as_str().into(),
  };
  let mut rows = vec![property(&ctx.t("runs.toolApproval.agent"), &agent)];
  rows.extend(shell);
  rows.push(command);
  rows.extend(
    approval
      .target
      .as_deref()
      .map(|target| styled_property(&ctx.t("runs.toolApproval.cwd"), target, ValueStyle::Mono)),
  );
  rows.push(property(&ctx.t("runs.toolApproval.native.timeLimit"), &time_limit));
  rows.push(
    Notice::new(
      Icon::ShieldOff,
      ctx.t("runs.toolApproval.native.unsandboxedTitle"),
      ctx.t("runs.toolApproval.native.unsandboxedDetail"),
    )
    .id("tool-approval-unsandboxed")
    .into(),
  );
  rows
}

/// The full string the shell receives (`bwWuT`): its label, then the string in mono on the canvas,
/// wrapped and scrolling when it is long.
fn command_block(ctx: &Ctx, command: &str) -> Element {
  let text = Text::new(command)
    .variant(ExtraTypography::Output.role())
    .color(PaletteColor::TextPrimary)
    .selectable(true);
  let block = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Inline.role())
    .padding_horizontal(ExtraSpacing::Stack.role())
    .child(text);
  let frame = Column::new()
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceBase)
    .border_inside(BorderSize::Sm, ExtraColor::Hairline.role())
    .rounded(RadiusSize::Sm)
    .child(
      ScrollVertical::new(block)
        .width(Dimension::Pct(100.0))
        .max_height(Dimension::Px(COMMAND_MAX_HEIGHT))
        .scrollbar(select_menu_scrollbar()),
    );
  Column::new()
    .id("tool-approval-shell-command")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("runs.toolApproval.native.commandLabel")).variant(TypographyStyle::Caption))
    .child(frame)
    .into()
}
