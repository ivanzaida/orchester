//! What the clone panel of *Link a folder* draws (06 `go2aM`, 07 `phoYc`, Row 3b): the folder row with what Git
//! would do there or why it can't, *Clone repository…*, and how the last clone went.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_api::SharedProject;
use orchester_workspace::sharing::project::clone::{CloneEnd, CloneRefusal, Destination};

use super::{Chosen, view::titles};
use crate::{
  theme::{ControlHeight, ExtraSpacing, IconSize, StartupSpacing, StartupText},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::{FieldInput, field, field_error},
    icon::{Icon, icon},
    message::message_text,
  },
};

pub(super) const INTO: &str = "join-team-link-clone-into";
pub(super) const START: &str = "join-team-link-clone-start";

/// *Choose…*: the picker's title and the folder name it adds to the folder picked.
pub(super) type Choose = FutureAction<(Arc<str>, String), Chosen, ()>;

/// The panel: its title and what it does, then `parts` (`No checkout yet` in 06 `go2aM`).
pub(super) fn panel(ctx: &Ctx, project: &SharedProject, parts: Vec<Element>) -> Element {
  let hint = project.remote_hint.as_ref().map_or("", |hint| hint.as_str());
  Column::new()
    .id("join-team-link-clone")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(titles(
      "join-team-link-clone",
      &ctx.t("startup.shared.link.clone.title"),
      &ctx.t_args("startup.shared.link.clone.sub", [("hint", hint)]),
    ))
    .with_children(parts)
    .into()
}

/// *Clone into* and *Choose…*, which lines up with the field's box, and under the row what Git would do with the
/// folder, or why it can't (`d3Cx27`, `DNRz3`). `name` is the folder Choose… adds to the one picked.
pub(super) fn into_row(
  ctx: &mut Ctx,
  (into, choose): (&Signal<String>, &Choose),
  (project, name): (&SharedProject, &str),
  destination: Option<&Destination>,
) -> Vec<Element> {
  let label = ctx.t("startup.shared.link.clone.into");
  let input = FieldInput::new(INTO, label.clone(), into.clone())
    .placeholder(ctx.t("startup.shared.link.clone.intoPlaceholder"))
    .text_style(StartupText::FieldMono.role())
    .box_height(ControlHeight::Lg.value());
  let refused = destination.is_some_and(|destination| !destination.allows());
  let input = if refused { input.invalid() } else { input }.render(ctx);
  let button = ActionButton::new(
    "join-team-link-clone-choose",
    ButtonKind::Secondary,
    ctx.t("startup.shared.link.choose"),
  )
  .icon(Icon::FolderOpen)
  .large()
  .roomy()
  .in_form();
  let button = if choose.state().get().is_pending() {
    button.loading()
  } else {
    let choose = choose.clone();
    let asked = (
      ctx.t_args(
        "startup.shared.link.clone.chooseTitle",
        [("project", project.name.as_str())],
      ),
      name.to_owned(),
    );
    button.on_click(move || choose.run(asked.clone()))
  };
  let row = Row::new()
    .id("join-team-link-clone-row")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::End)
    .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(field(
      &format!("{INTO}-field"),
      Some(&label),
      input,
      None,
    )))
    .child(button)
    .into();
  let mut parts = vec![row];
  parts.extend(destination.map(|destination| target(ctx, destination, &into.get())));
  parts
}

/// What Git would do with the folder, or why it can't.
fn target(ctx: &Ctx, destination: &Destination, typed: &str) -> Element {
  let error = |words: &str| field_error("join-team-link-clone-error", words).into();
  match destination {
    Destination::New { parent } => {
      let name = Path::new(typed.trim())
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
      let parent = parent.display().to_string();
      line(
        "join-team-link-clone-target",
        (Icon::FolderPlus, PaletteColor::TextMuted),
        &ctx.t_args(
          "startup.shared.link.clone.creates",
          [("name", name.as_str()), ("parent", parent.as_str())],
        ),
      )
    }
    Destination::EmptyFolder => line(
      "join-team-link-clone-target",
      (Icon::Folder, PaletteColor::TextMuted),
      &ctx.t("startup.shared.link.clone.emptyFolder"),
    ),
    Destination::Occupied => error(&ctx.t("startup.shared.link.clone.occupied")),
    Destination::NoParent { parent } => error(&ctx.t_args(
      "startup.shared.link.clone.noParent",
      [("parent", parent.display().to_string().as_str())],
    )),
    Destination::NotAbsolute => error(&ctx.t("startup.shared.link.clone.notAbsolute")),
    Destination::NotLocal => error(&ctx.t("startup.shared.link.clone.notLocal")),
    Destination::Unreadable => error(&ctx.t("startup.shared.link.clone.unreadable")),
  }
}

/// *Clone repository…*: loading while the console runs, unavailable while the folder can't take a clone.
pub(super) fn start_button(ctx: &Ctx, running: bool, start: Option<impl Fn() + Send + Sync + 'static>) -> ActionButton {
  if running {
    return ActionButton::new(START, ButtonKind::Secondary, ctx.t("startup.shared.link.clone.running")).loading();
  }
  let button = ActionButton::new(START, ButtonKind::Secondary, ctx.t("startup.shared.link.clone.start"))
    .icon(Icon::SquareTerminal);
  match start {
    Some(start) => button.on_click(start),
    None => button.disabled(),
  }
}

/// While the console runs (07 `phoYc`).
pub(super) fn waiting(ctx: &Ctx) -> Element {
  Callout::new(
    "join-team-link-clone-waiting",
    CalloutTone::Info,
    ctx.t("startup.shared.link.clone.waiting.title"),
  )
  .icon(Icon::SquareTerminal)
  .body(ctx.t("startup.shared.link.clone.waiting.body"))
  .into()
}

/// How the last clone ended (07b–07f).
pub(super) fn ended(ctx: &Ctx, project: &SharedProject, end: &CloneEnd) -> Vec<Element> {
  match end {
    CloneEnd::Cloned { folder } => vec![line(
      "join-team-link-clone-done",
      (Icon::CircleCheck, PaletteColor::Success),
      &ctx.t_args(
        "startup.shared.link.clone.done",
        [("path", folder.display().to_string().as_str())],
      ),
    )],
    CloneEnd::NotCheckout { folder, check } => vec![
      Callout::new(
        "join-team-link-clone-not-checkout",
        CalloutTone::Warning,
        ctx.t("startup.shared.link.clone.notCheckout.title"),
      )
      .body(message_text(ctx, &check.message(project)))
      .into(),
      left(ctx, folder),
    ],
    CloneEnd::Failed { code, left: kept } => {
      let hint = project.remote_hint.as_ref().map_or("", |hint| hint.as_str());
      let failed = Callout::new(
        "join-team-link-clone-failed",
        CalloutTone::Danger,
        ctx.t_args(
          "startup.shared.link.clone.failed.title",
          [("code", code.to_string().as_str())],
        ),
      )
      .body(ctx.t_args("startup.shared.link.clone.failed.body", [("hint", hint)]))
      .into();
      [Some(failed), kept.as_deref().map(|kept| left(ctx, kept))]
        .into_iter()
        .flatten()
        .collect()
    }
    CloneEnd::Closed { left: kept } => {
      let closed = Callout::new(
        "join-team-link-clone-closed",
        CalloutTone::Warning,
        ctx.t("startup.shared.link.clone.closed.title"),
      )
      .body(ctx.t("startup.shared.link.clone.closed.body"))
      .into();
      [Some(closed), kept.as_deref().map(|kept| left(ctx, kept))]
        .into_iter()
        .flatten()
        .collect()
    }
  }
}

/// Why no clone ran. A refused folder is said under its row, by the check that runs again as the clone ends; a
/// project without an address has no panel.
pub(super) fn refused(ctx: &Ctx, refusal: &CloneRefusal) -> Option<Element> {
  let (id, title, body) = match refusal {
    CloneRefusal::Destination(_) | CloneRefusal::NoUrl => return None,
    CloneRefusal::GitMissing => (
      "join-team-link-clone-git-missing",
      "startup.shared.link.clone.gitMissing.title",
      "startup.shared.link.clone.gitMissing.body",
    ),
    CloneRefusal::NotOpened => (
      "join-team-link-clone-not-opened",
      "startup.shared.link.clone.notOpened.title",
      "startup.shared.link.clone.notOpened.body",
    ),
    CloneRefusal::NotStarted => (
      "join-team-link-clone-not-started",
      "startup.shared.link.clone.notStarted.title",
      "startup.shared.link.clone.notStarted.body",
    ),
    CloneRefusal::Unsupported => (
      "join-team-link-clone-unsupported",
      "startup.shared.link.clone.unsupported.title",
      "startup.shared.link.clone.unsupported.body",
    ),
    CloneRefusal::Lost | CloneRefusal::Interrupted => (
      "join-team-link-clone-lost",
      "startup.shared.link.clone.lost.title",
      "startup.shared.link.clone.lost.body",
    ),
  };
  Some(
    Callout::new(id, CalloutTone::Danger, ctx.t(title))
      .body(ctx.t(body))
      .into(),
  )
}

/// The folder a clone left, kept.
fn left(ctx: &Ctx, folder: &Path) -> Element {
  line(
    "join-team-link-clone-left",
    (Icon::Folder, PaletteColor::TextMuted),
    &ctx.t_args(
      "startup.shared.link.clone.left",
      [("path", folder.display().to_string().as_str())],
    ),
  )
}

/// A 13 mark and what it says, 6 apart, as the folder check's line (`Check` `N3vRO2`).
fn line(id: &str, (glyph, color): (Icon, PaletteColor), text: &str) -> Element {
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color).flex_shrink(0.0))
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(StartupText::CheckLine.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// The actions row of the panel: the button and, under it, why a press of synthetic input did nothing.
pub(super) fn actions(button: ActionButton, person_only: Option<Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(StartupSpacing::Row.role())
    .child(Row::new().align_items(Alignment::Center).child(button.large().roomy()))
    .with_children(person_only)
    .into()
}
