//! The GitHub repository detected in the chosen folder (`GitHub detection · Detected` `iyZCd`, its states in
//! `gh5wV`): a block under the folder that says what detection found, what it means and what can be done —
//! *Check again*, *Change GitHub account or access* ([`access`]), *Choose program…* when `gh` was not found,
//! *Use this account* when `gh` acts as another account than the project expects —, with the command that fixes
//! the state to run in a terminal. Orchester never runs those commands. Before a folder is chosen, a notice says
//! GitHub is detected from it (`Notice · GitHub is detected from the folder` `dhX0E`). GitHub is optional:
//! only git holds Create back ([`super::bar`]) — a folder that is not a git repository, a repository without a
//! commit, git not installed —, and the block offers to fix that in place ([`git_init`]).

mod access;
pub(super) mod git_init;
mod words;

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::{GhAdvice, GitHubLogin},
  name::Name,
  repository::{Detection, DetectionState, GitHubDetection},
};
use orchester_workspace::project_setup::ExpectedAccount;

use self::words::{Property, Value};
use super::{Actions, Expected, Form};
use crate::{
  services::path_picker::PathKind,
  theme::{ExtraSpacing, ExtraTypography, IconSize, ProjectsText},
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::Copier,
    command_row::CommandRow,
    github_mark::github_mark,
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
  },
};

/// A property's label column and row (`X7na6R` in `Row · Repository` `Y7zqv`), and the head's glyph tile
/// (`Glyph` `VCpFK`).
const LABEL_WIDTH: f32 = 128.0;
const PROPERTY_HEIGHT: f32 = 28.0;
const GLYPH_TILE: f32 = 28.0;

/// The block for the chosen folder, or the notice while none is chosen.
pub(super) fn block(ctx: &mut Ctx, form: &Form, actions: &Actions, detection: Option<&Detection>) -> Option<Element> {
  let Some(folder) = form.folder.get().0 else {
    return Some(
      Notice::new(
        "github-from-folder",
        NoticeTone::Info,
        ctx.t("setup.detectedFromFolder"),
      )
      .icon(Icon::FolderGit2)
      .body(ctx.t("setup.detectedFromFolderDetail"))
      .into(),
    );
  };
  let local = actions.local(Some(&folder));
  if let Some(shown) = git_init::Shown::of(local.as_ref()) {
    return Some(git_init::block(ctx, form, actions, &shown, &folder));
  }
  let expected = detection.and_then(|detection| form.expected_for(detection));
  let state = detection.map(|detection| detection.state(expected.as_ref()));
  let found = detection.and_then(Detection::github);
  let name = form.shown_name();
  let project = name
    .as_ref()
    .map_or_else(|| ctx.t("setup.unnamed").to_string(), |name| name.as_str().to_owned());
  let properties = words::properties(
    ctx,
    (detection, local.as_ref()),
    state.as_ref(),
    expected.as_ref(),
    &folder,
  );
  let mut block = frame(state.as_ref().map_or("checking", state_name))
    .child(head(ctx, Head::of(ctx, state.as_ref(), found)))
    .child(properties_column(properties))
    .with_children(git_init::created_line(ctx, form, &folder))
    .child(
      Text::new(&words::note(
        ctx,
        state.as_ref(),
        found,
        name.as_ref().map(Name::as_str),
      ))
      .id("github-detection-note")
      .variant(TypographyStyle::Description)
      .width(Dimension::Pct(100.0)),
    );
  let Some(state) = state else {
    // Detecting: the note alone, nothing to press yet.
    return Some(block.into());
  };
  if let Some(fix) = fix(ctx, &state, detection) {
    block = block.child(fix);
  }
  let asked = found.filter(|found| form.access.get() && found.check().is_some());
  block = match (asked, detection) {
    (Some(found), _) => block.child(access::panel(ctx, form, actions, found, (expected.as_ref(), &project))),
    (None, Some(detection)) => block.child(buttons(ctx, form, actions, (&state, detection), &folder)),
    // A state comes from a detection.
    (None, None) => block,
  };
  Some(block.into())
}

/// The block around what it says of the folder in `state` (`GitHub detection` `iyZCd`).
fn frame(state: &'static str) -> Column {
  Column::new()
    .id("github-detection")
    .describe("state", state)
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
}

/// What the head says (`Head` `ph3Wf`): the status's icon in `mark`, its word's key in `color`, the title, and
/// whether GitHub, which is optional, is what the block is about.
struct Head {
  glyph: Icon,
  mark: PaletteColor,
  color: PaletteColor,
  status: &'static str,
  title: Arc<str>,
  optional: bool,
}

impl Head {
  /// The head of a detection in `state`; `None` while detecting.
  fn of(ctx: &Ctx, state: Option<&DetectionState>, found: Option<&GitHubDetection>) -> Self {
    let (glyph, mark, color, status) = words::status(state);
    Self {
      glyph,
      mark,
      color,
      status,
      title: words::heading(ctx, state, found),
      // GitHub is optional, but a folder that is no repository is not (`gutA8`).
      optional: !matches!(state, Some(DetectionState::NotAGitRepository)),
    }
  }
}

/// The glyph, the title with the status under it, and *Optional* (`Head` `ph3Wf`).
fn head(ctx: &Ctx, head: Head) -> Row {
  let tile = Row::new()
    .width(Dimension::Px(GLYPH_TILE))
    .height(Dimension::Px(GLYPH_TILE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(github_mark(ctx, IconSize::Md, PaletteColor::TextSecondary));
  let status = Row::new()
    .id("github-detection-status")
    .describe("status", head.status)
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(head.glyph, IconSize::Sm, head.mark))
    .child(
      Text::new(&ctx.t(head.status))
        .variant(TypographyStyle::Label)
        .color(head.color),
    );
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(tile)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&head.title)
            .id("github-detection-heading")
            .variant(ProjectsText::Heading.role()),
        )
        .child(status),
    )
    .with_children(
      head
        .optional
        .then(|| Text::new(&ctx.t("setup.github.optional")).variant(TypographyStyle::Caption)),
    )
}

/// The block's properties (`Properties` `Mcur1`).
fn properties_column(properties: Vec<Property>) -> Column {
  Column::new()
    .id("github-detection-properties")
    .width(Dimension::Pct(100.0))
    .with_children(properties.into_iter().map(property))
}

/// What a property is, and its value (`Row · Repository` `Y7zqv`).
fn property(property: Property) -> Element {
  let value = match property.look {
    Value::Mono => Text::new(&property.value).variant(ProjectsText::Path.role()),
    Value::Text(color) => Text::new(&property.value)
      .variant(ExtraTypography::Property.role())
      .color(color),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(PROPERTY_HEIGHT))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&property.label)
        .variant(ExtraTypography::Property.role())
        .width(Dimension::Px(LABEL_WIDTH))
        .flex_shrink(0.0),
    )
    .child(value.flex(1.0).min_width(Dimension::Px(0.0)))
    .into()
}

/// The command that fixes the state, introduced by *Run this in a terminal* where the note does not say it
/// already (`Fix` `Qg5cA`).
fn fix(ctx: &mut Ctx, state: &DetectionState, detection: Option<&Detection>) -> Option<Element> {
  let host = detection
    .and_then(Detection::github)
    .map(|found| found.repo().host().clone());
  let (id, command, introduced) = match state {
    DetectionState::NotSignedIn => ("github-command-login", GhAdvice::Login.command(host.as_ref()?), false),
    DetectionState::WrongAccount { expected, .. } => (
      "github-command-switch",
      GhAdvice::Switch(Some(expected.clone())).command(host.as_ref()?),
      true,
    ),
    DetectionState::MissingScope { .. } => (
      "github-command-refresh",
      GhAdvice::Refresh.command(host.as_ref()?),
      true,
    ),
    _ => return None,
  };
  let row = command_row(ctx, id, &command);
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .with_children(
        introduced.then(|| Text::new(&ctx.t("setup.github.runThis")).variant(ExtraTypography::Note.role())),
      )
      .child(row)
      .into(),
  )
}

/// `command` in its box with Copy, which puts it where the window's copier puts text.
fn command_row(ctx: &mut Ctx, id: &str, command: &str) -> Element {
  let (copier, copied) = (Copier::of(ctx), command.to_owned());
  CommandRow::new(id.to_owned(), command.to_owned(), ctx.t("common.copy"), move || {
    copier.copy(&copied)
  })
  .dollar_prompt()
  .render(ctx)
}

/// *Check again*, then what the state offers (`Actions` `cUldA`, `yName`). While a detection or the program
/// picker runs, they wait.
fn buttons(
  ctx: &Ctx,
  form: &Form,
  actions: &Actions,
  (state, detection): (&DetectionState, &Detection),
  folder: &Path,
) -> Element {
  let busy = actions.detect.state().get().is_pending() || actions.program.state().get().is_pending();
  let wait = |button: ActionButton, run: Arc<dyn Fn() + Send + Sync>| {
    if busy {
      button.disabled()
    } else {
      button.on_click(move || run())
    }
  };
  let secondary = |id: &str, key: &str, glyph| {
    ActionButton::new(id.to_owned(), ButtonKind::Secondary, ctx.t(key))
      .icon(glyph)
      .in_form()
  };
  let check = {
    let (detect, folder, program) = (actions.clone(), folder.to_path_buf(), form.program.clone());
    Arc::new(move || detect.detect(folder.clone(), program.get_untracked().0)) as Arc<dyn Fn() + Send + Sync>
  };
  let open_access = {
    let access = form.access.clone();
    Arc::new(move || access.set(true)) as Arc<dyn Fn() + Send + Sync>
  };
  let mut row: Vec<Element> = vec![
    wait(
      secondary("github-check", "setup.github.checkAgain", Icon::RefreshCw),
      check,
    )
    .into(),
  ];
  let change = || {
    wait(
      secondary(
        "github-change-account",
        "setup.github.changeAccount",
        Icon::UserRoundCog,
      ),
      open_access.clone(),
    )
  };
  match state {
    DetectionState::Detected { .. } | DetectionState::NoAccess { .. } => row.push(change().into()),
    DetectionState::NotGitHub => {
      let reason = ctx.t("setup.github.addConnectionUnavailable");
      row.push(
        secondary("github-add-connection", "setup.github.addConnection", Icon::Plus)
          .unavailable(reason)
          .into(),
      );
    }
    DetectionState::GhNotFound => {
      let (pick, title) = (actions.program.clone(), ctx.t("setup.github.chooseProgramTitle"));
      let choose = Arc::new(move || pick.run((PathKind::File, title.clone()))) as Arc<dyn Fn() + Send + Sync>;
      row.push(
        wait(
          secondary("github-choose-program", "setup.github.chooseProgram", Icon::FolderOpen),
          choose,
        )
        .into(),
      );
    }
    DetectionState::WrongAccount { active, .. } => {
      row.push(change().into());
      row.push(use_account(
        ctx,
        form,
        (active, ExpectedAccount::acting(folder, detection)),
        busy,
      ));
    }
    _ => {}
  }
  Row::new()
    .id("github-detection-actions")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(row)
    .child(Spacer::new().flex(1.0))
    .into()
}

/// *Use this account (ivan-work)*: the project will expect the account `gh` acts as now; `gh` is not changed.
fn use_account(
  ctx: &Ctx,
  form: &Form,
  (active, acting): (&GitHubLogin, Option<ExpectedAccount>),
  busy: bool,
) -> Element {
  let label = ctx.t_args("setup.github.useAccount", [("account", active.as_str())]);
  let button = ActionButton::new("github-use-account", ButtonKind::Secondary, label)
    .icon(Icon::UserRoundCheck)
    .in_form();
  if busy {
    return button.disabled().into();
  }
  let expected = form.expected.clone();
  button.on_click(move || expected.set(Expected(acting.clone()))).into()
}

/// The name a test or a tool reads the state by.
const fn state_name(state: &DetectionState) -> &'static str {
  match state {
    DetectionState::Detected { .. } => "detected",
    DetectionState::NotAGitRepository => "not_repository",
    DetectionState::GitFailed => "git_failed",
    DetectionState::NoRemote => "local_only",
    DetectionState::NotGitHub => "not_github",
    DetectionState::GhNotFound => "gh_not_found",
    DetectionState::NotSignedIn => "not_signed_in",
    DetectionState::WrongAccount { .. } => "wrong_account",
    DetectionState::MissingScope { .. } => "missing_scope",
    DetectionState::NoAccess { .. } => "no_access",
    DetectionState::Unrecognized => "unrecognized",
    DetectionState::CouldNotRun => "could_not_run",
    DetectionState::Unreachable => "unreachable",
    DetectionState::RateLimited => "rate_limited",
    DetectionState::NoDefaultBranch => "empty_repository",
  }
}
