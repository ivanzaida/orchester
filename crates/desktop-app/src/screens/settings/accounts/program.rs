//! Each agent CLI's program in Settings › Accounts (`Agents on accounts` A1 `aOR4w`, A3 `t2YD7`): one quiet line
//! under the CLI's title with the version the program reports and where it is, and *Change…*, which asks the
//! system's file picker for another one. When none is set and none is found, or the one set does not answer as the
//! CLI, the line says so in `danger` with *Choose program…*: the CLI's accounts stay and can't run until then.
//! Every project's connection on an account of the CLI is made on this program.

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::account::AccountCli;
use orchester_workspace::{accounts::CliProgram, background, display, messages::Message};

use super::words::{cli_key, t, ta};
use crate::{
  services::{Services, accounts::OnThisComputer},
  theme::{ExtraColor, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The path's room on the line before it is shortened in its middle.
const PATH_CHARS: usize = 72;

pub(super) type ChooseAction = FutureAction<AccountCli, (), Message>;

/// Asks the system's file picker for the program of a CLI and keeps the file chosen; nothing when it is closed.
pub(super) fn choose(ctx: &mut Ctx) -> ChooseAction {
  let workspace = Services::of(ctx).workspace;
  ctx.future_action(move |cli: AccountCli| {
    let workspace = workspace.clone();
    async move {
      background::run(move || {
        let Some(program) = rfd::FileDialog::new().pick_file() else {
          return Ok(());
        };
        workspace.set_agent_program(cli, &program).map_err(|error| {
          tracing::warn!(%error, "an agent CLI's program was not kept");
          error.message()
        })
      })
      .await?
    }
  })
}

/// The line of `cli`'s program under its group's title; nothing until the programs are read.
pub(super) fn line(
  ctx: &Ctx,
  here: Option<&OnThisComputer>,
  cli: AccountCli,
  choose: &ChooseAction,
) -> Option<Element> {
  let program = here?.program(cli)?;
  let id = format!("accounts-program-{}", cli.as_str());
  let button = |key: &str| {
    let button = ActionButton::new(format!("{id}-choose"), ButtonKind::Ghost, t(ctx, key)).in_heading();
    if choose.state().get().is_pending() {
      button.disabled()
    } else {
      let choose = choose.clone();
      button.on_click(move || choose.run(cli))
    }
  };
  let line = Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .describe("state", if program.runs() { "found" } else { "not-found" });
  let line = match (&program.version, &program.path) {
    (Some(version), Some(path)) => line
      .child(icon(Icon::Terminal, IconSize::Sm, PaletteColor::MutedForeground))
      .child(
        Text::new(&format!("{} {version}", cli.program()))
          .id(format!("{id}-version"))
          .variant(ExtraTypography::MonoKey.role())
          .color(ExtraColor::TextSecondary.role())
          .nowrap(),
      )
      .child(
        Text::new(&display::shorten_path(path, PATH_CHARS))
          .id(format!("{id}-path"))
          .variant(ExtraTypography::MonoKey.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap(),
      )
      .child(button("accounts.program.change")),
    (_, path) => line
      .child(icon(Icon::TriangleAlert, IconSize::Sm, PaletteColor::Destructive))
      .child(
        Text::new(&missing(ctx, program, path.as_deref()))
          .id(format!("{id}-missing"))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::Destructive)
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(button("accounts.program.choose")),
  };
  Some(line.into())
}

/// Why the CLI's accounts can't run: no program was found, or the one set does not answer as the CLI.
fn missing(ctx: &Ctx, program: &CliProgram, path: Option<&std::path::Path>) -> String {
  let cli = t(ctx, cli_key(program.cli));
  match path {
    Some(path) => ta(
      ctx,
      "accounts.program.notAnswering",
      [("cli", cli), ("path", display::shorten_path(path, PATH_CHARS))],
    ),
    None => ta(ctx, "accounts.program.notFound", [("cli", cli)]),
  }
}
