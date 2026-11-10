//! The foot of the stopped card in a team project (`Restore` in `Run state · Stopped · archived` of A22 `pUu78`):
//! restoring a team resource is the admins' (`docs/design/team-configuration.md`, rule 7), and this app has no restore
//! of its own, so a member's card has *Ask an admin to restore it*, which copies a request naming the resource, the
//! task and the run for the person to send, and says who the admins are; an admin's says where it is restored.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::TaskContext;
use crate::{
  screens::hub::team,
  ui::{
    button::{ActionButton, ButtonKind},
    clipboard::Clipboard,
    icon::Icon,
  },
};

/// What came of the last press of *Ask an admin to restore it*.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Asked {
  #[default]
  Not,
  Copied,
  Failed,
}

impl lurq::app::component::DevtoolsInspectable for Asked {}

/// The foot of the card of a team task, whose run an archive of `resource` stopped at run `run`; none in a local
/// project or while the team's configuration is not read.
pub(super) fn foot(
  ctx: &mut Ctx,
  task: &TaskContext,
  (resource, run): (&str, u32),
  asked: &Signal<Asked>,
) -> Option<Element> {
  task.team.as_ref()?;
  let team = team::of(&task.project)?;
  let (button, hint) = if team.may_change {
    (None, ctx.t("summary.archived.restoreOnServer"))
  } else {
    let request = ctx.t_args(
      "summary.archived.request",
      [
        ("resource", resource.to_owned()),
        ("project", task.project.open.project.name().to_string()),
        ("task", task.detail.task.key().to_string()),
        ("run", run.to_string()),
      ],
    );
    let (clipboard, said) = (Clipboard::of(ctx), asked.clone());
    let words = match asked.get() {
      Asked::Not => ctx.t("summary.archived.ask"),
      Asked::Copied => ctx.t("summary.archived.asked"),
      Asked::Failed => ctx.t("summary.archived.askFailed"),
    };
    let button = ActionButton::new("task-archived-ask", ButtonKind::Secondary, words)
      .icon(Icon::Undo2)
      .compact()
      .on_click(move || {
        said.set(if clipboard.copy(&request) {
          Asked::Copied
        } else {
          Asked::Failed
        });
      });
    let admins = team::admins(ctx, &team);
    (
      Some(button),
      ctx.t_args("summary.archived.adminsOnly", [("admins", admins)]),
    )
  };
  Some(
    Row::new()
      .id("task-archived-foot")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .with_children(button.map(Element::from))
      .child(Row::new().flex(1.0))
      .child(
        Text::new(&hint)
          .id("task-archived-foot-hint")
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
      .into(),
  )
}
