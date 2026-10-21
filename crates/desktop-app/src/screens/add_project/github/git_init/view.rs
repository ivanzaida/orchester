//! The GitHub block while git holds Create back (`OdlXb`): the head, the folder's properties and the note, then
//! *Create a git repository here* (`ohtvI`): what `git init` and the first commit do, the choice of the first
//! commit, the progress or the failure, the actions and the same work as commands for a terminal. With git not
//! installed the action is unavailable and the block says how to install git (`KsWqe`).

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::repository::init::{FirstCommit, INITIAL_COMMIT, InitError, Survey, SurveyError};

use super::{
  super::{command_row, frame, head, properties_column},
  Choice, Progress, Shown, first_commit,
  install::INSTALL_GIT,
  start, words,
};
use crate::{
  screens::add_project::{Actions, Form},
  services::path_picker::PathKind,
  theme::{DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize, ProjectsText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    option_card::OptionCard,
    unavailable::reason_line,
  },
};

/// A line's icon slot (`Icon slot` `v7Ht71`), and the error's (`E05Ei`), which keep the icon with the first line.
const LINE_SLOT: (f32, f32) = (14.0, 18.0);
const ERROR_SLOT: (f32, f32) = (13.0, 17.0);
/// The commands the block makes, for a terminal (`eZ99l`).
const GIT_INIT: &str = "git init";
const GIT_ADD: &str = "git add -A";

/// What the block is drawn from.
struct Git<'a> {
  shown: &'a Shown,
  folder: &'a Path,
  surveyed: Option<&'a Result<Survey, SurveyError>>,
  failure: Option<&'a InitError>,
  progress: Option<Progress>,
  first: FirstCommit,
}

/// The block for a folder in `shown`.
pub(in super::super) fn block(ctx: &mut Ctx, form: &Form, actions: &Actions, shown: &Shown, folder: &Path) -> Element {
  let surveyed = actions.git.survey_of(folder);
  let attempt = form.attempt.get();
  let failure = attempt.in_folder(folder).and_then(|result| result.as_ref().err());
  let git = Git {
    shown,
    folder,
    surveyed: surveyed.as_ref(),
    failure,
    progress: actions.git.progress(folder),
    first: first_commit(form, surveyed.as_ref()),
  };
  let failed = failure.is_some();
  let section = match shown {
    Shown::Missing => missing(ctx, form, actions, folder),
    Shown::NotRepository | Shown::NoCommits { .. } => create(ctx, form, actions, &git),
  };
  frame(words::state_name(shown))
    .child(head(ctx, words::head(ctx, shown, failed)))
    .child(properties_column(words::properties(ctx, (shown, folder), git.surveyed)))
    .child(
      Text::new(&words::note(ctx, shown, failed))
        .id("github-detection-note")
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    )
    .child(section)
    .into()
}

/// The line that says the repository was made here with its first commit (`VSLvH` in `Tz5KP`), while the folder
/// it was made in is chosen.
pub(in super::super) fn created_line(ctx: &Ctx, form: &Form, folder: &Path) -> Option<Element> {
  let attempt = form.attempt.get();
  let made = attempt.in_folder(folder)?.as_ref().ok()?;
  Some(
    line(Icon::CircleCheck, PaletteColor::Success, &words::done(ctx, made))
      .id("git-init-done")
      .into(),
  )
}

/// *Create a git repository here* with its rule above it (`ohtvI`); its title faded when it cannot be done
/// (`JVNfI`).
fn section(ctx: &Ctx, unavailable: bool) -> Column {
  let title = Text::new(&ctx.t("setup.git.create.title"))
    .id("git-init-title")
    .variant(ProjectsText::CreateTitle.role());
  Column::new()
    .id("git-init")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(if unavailable {
      title.opacity(DISABLED_OPACITY)
    } else {
      title
    })
}

fn create(ctx: &mut Ctx, form: &Form, actions: &Actions, git: &Git<'_>) -> Element {
  let fresh = matches!(git.shown, Shown::NotRepository);
  let idle = git.progress.is_none();
  let mut section = section(ctx, false);
  if fresh {
    section = section
      .child(line(
        Icon::GitBranch,
        PaletteColor::TextMuted,
        &words::branch_line(ctx, git.surveyed),
      ))
      .child(line(
        Icon::GitCommitHorizontal,
        PaletteColor::TextMuted,
        &ctx.t("setup.git.create.then"),
      ));
  }
  section = section.child(choices(ctx, form, git));
  if fresh && idle && git.first == FirstCommit::Files {
    section = section.child(hint_line(Icon::Eye, &ctx.t("setup.git.create.review")).id("git-init-review"));
  }
  if let Some(progress) = &git.progress {
    section = section.child(progress_lines(ctx, progress, git));
  }
  if let Some(failure) = git.failure.filter(|_| idle) {
    section = section.with_children(failure_lines(ctx, failure, git.folder));
  }
  section = section.child(buttons(ctx, form, actions, git));
  let identity = matches!(git.failure, Some(InitError::NoIdentity(_)));
  if idle && !identity {
    section = section.child(fallback(ctx, git));
  }
  section.into()
}

/// *FIRST COMMIT* and its two choices (`vj81K`); an empty folder has only the empty one, and while git works
/// neither can be changed.
fn choices(ctx: &Ctx, form: &Form, git: &Git<'_>) -> Column {
  let choose = |first: FirstCommit| {
    let chosen = form.first.clone();
    move || chosen.set(Choice(Some(first)))
  };
  let card = |first: FirstCommit, (id, glyph, title): (&str, Icon, &str), detail: Arc<str>| {
    let card = OptionCard::new(id, (glyph, ctx.t(title)), detail, git.first == first, choose(first));
    if git.progress.is_some() { card.waiting() } else { card }
  };
  let mut files = card(
    FirstCommit::Files,
    ("git-init-first-files", Icon::Files, "setup.git.create.files"),
    words::files_detail(ctx, git.surveyed),
  );
  if matches!(git.surveyed, Some(Ok(survey)) if survey.files == 0) {
    files = files.unavailable(ctx.t("setup.git.create.filesEmpty"));
  }
  let empty = card(
    FirstCommit::Empty,
    ("git-init-first-empty", Icon::CircleDashed, "setup.git.create.empty"),
    words::empty_detail(ctx, git.surveyed),
  );
  Column::new()
    .id("git-init-first")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("setup.git.create.firstCommit")).variant(ProjectsText::ChoiceLabel.role()))
    .child(files)
    .child(empty)
}

/// What git has done and is doing (`Progress` `Lj60r`): the repository made, then the turning loader and the
/// commit under way.
fn progress_lines(ctx: &Ctx, progress: &Progress, git: &Git<'_>) -> Column {
  let (made, working) = match progress {
    Progress::Initializing => (None, words::initializing(ctx, git.surveyed)),
    Progress::Committing { made } => (
      made
        .as_ref()
        .map(|branch| ctx.t_args("setup.git.progress.initialized", [("branch", branch.as_str())])),
      words::committing(ctx, git.first, git.surveyed),
    ),
  };
  Column::new()
    .id("git-init-progress")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .with_children(made.map(|made| line(Icon::CircleCheck, PaletteColor::Success, &made)))
    .child(
      Row::new()
        .id("git-init-working")
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::LoaderCircle, IconSize::Base, PaletteColor::TextMuted))
        .child(Text::new(&working).variant(ExtraTypography::Note.role())),
    )
}

/// Why it failed, in `danger`, with git's own words under it (`Error` `e5MlF`, `git output` `eoAWr`); when git does
/// not know the author, how to tell it (`oryGp`).
fn failure_lines(ctx: &mut Ctx, failure: &InitError, folder: &Path) -> Vec<Element> {
  let (line, said) = words::failure(ctx, failure, folder);
  let mut lines: Vec<Element> = vec![
    Row::new()
      .id("git-init-error")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Start)
      .describe("role", "form-error")
      .child(slot(ERROR_SLOT, Icon::CircleAlert, PaletteColor::Danger))
      .child(
        Text::new(&line)
          .variant(ExtraTypography::Prose.role())
          .color(PaletteColor::Danger)
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .into(),
  ];
  if let Some(said) = said {
    lines.push(
      Column::new()
        .id("git-init-output")
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
        .background(ExtraColor::CodeBackground.role())
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .child(
          Text::new(said)
            .variant(ProjectsText::GitOutput.role())
            .width(Dimension::Pct(100.0)),
        )
        .into(),
    );
  }
  if matches!(failure, InitError::NoIdentity(_)) {
    lines.push(
      Text::new(&ctx.t("setup.git.identity.intro"))
        .variant(ExtraTypography::Note.role())
        .into(),
    );
    for (id, key) in [
      ("git-identity-name", "setup.git.identity.name"),
      ("git-identity-email", "setup.git.identity.email"),
    ] {
      let command = ctx.t(key);
      lines.push(command_row(ctx, id, &command));
    }
  }
  lines
}

/// Create, Try again or *Make the first commit*, then *Choose another folder…* and *Check again* (`E1eSbF`). While
/// git works or the folder is read, they wait; Create waits for the files to be counted too.
fn buttons(ctx: &Ctx, form: &Form, actions: &Actions, git: &Git<'_>) -> Element {
  let detecting = actions.detect.state().get().is_pending();
  let busy = git.progress.is_some();
  let commit_only = matches!(git.shown, Shown::NoCommits { .. });
  let (key, glyph) = match (busy, git.failure.is_some(), commit_only) {
    (true, _, false) => ("setup.git.create.creating", Icon::GitBranchPlus),
    (true, _, true) => ("setup.git.create.committing", Icon::GitCommitHorizontal),
    (false, true, _) => ("setup.git.create.tryAgain", Icon::RotateCcw),
    (false, false, false) => ("setup.git.create.create", Icon::GitBranchPlus),
    (false, false, true) => ("setup.git.create.commit", Icon::GitCommitHorizontal),
  };
  let primary = ActionButton::new("git-init-create", ButtonKind::Primary, ctx.t(key))
    .icon(glyph)
    .in_form();
  let primary = if busy {
    primary.loading()
  } else if detecting || git.surveyed.is_none() {
    primary.disabled()
  } else {
    let (form, actions, shown, folder, first) = (
      form.clone(),
      actions.clone(),
      git.shown.clone(),
      git.folder.to_path_buf(),
      git.first,
    );
    primary.on_click(move || start(&form, &actions, (&shown, &folder), first))
  };
  Row::new()
    .id("git-init-actions")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(primary)
    .child(choose_folder(ctx, actions, busy))
    .child(check_again(ctx, form, actions, git.folder, busy || detecting))
    .into()
}

/// *Choose another folder…*: the folder field's picker.
fn choose_folder(ctx: &Ctx, actions: &Actions, waits: bool) -> ActionButton {
  let button = ActionButton::new(
    "git-init-choose-folder",
    ButtonKind::Secondary,
    ctx.t("setup.git.create.chooseAnother"),
  )
  .icon(Icon::FolderOpen)
  .in_form();
  if waits {
    return button.disabled();
  }
  let (pick, title) = (actions.folder.clone(), ctx.t("setup.chooseFolderTitle"));
  button.on_click(move || pick.run((PathKind::Folder, title.clone())))
}

/// *Check again*: reads the folder again.
fn check_again(ctx: &Ctx, form: &Form, actions: &Actions, folder: &Path, waits: bool) -> ActionButton {
  let button = ActionButton::new(
    "git-init-check",
    ButtonKind::Secondary,
    ctx.t("setup.github.checkAgain"),
  )
  .icon(Icon::RefreshCw)
  .in_form();
  if waits {
    return button.disabled();
  }
  let (detect, folder, program) = (actions.clone(), folder.to_path_buf(), form.program.clone());
  button.on_click(move || detect.detect(folder.clone(), program.get_untracked().0))
}

/// The same work as commands for a terminal (`Fallback · terminal` `UrR0Z`), for the first commit chosen.
fn fallback(ctx: &mut Ctx, git: &Git<'_>) -> Element {
  let (intro, init) = match git.shown {
    Shown::NotRepository => ("setup.git.create.fallback", true),
    Shown::NoCommits { .. } | Shown::Missing => ("setup.git.create.fallbackCommit", false),
  };
  let commit = match git.first {
    FirstCommit::Files => format!("git commit -m \"{INITIAL_COMMIT}\""),
    FirstCommit::Empty => format!("git commit --allow-empty -m \"{INITIAL_COMMIT}\""),
  };
  let mut commands = Vec::new();
  if init {
    commands.push(("git-init-command-init", GIT_INIT.to_owned()));
  }
  if git.first == FirstCommit::Files {
    commands.push(("git-init-command-add", GIT_ADD.to_owned()));
  }
  commands.push(("git-init-command-commit", commit));
  let intro = Text::new(&ctx.t(intro)).variant(ExtraTypography::Note.role());
  let rows: Vec<Element> = commands
    .into_iter()
    .map(|(id, command)| command_row(ctx, id, &command))
    .collect();
  Column::new()
    .id("git-init-fallback")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(intro)
    .with_children(rows)
    .into()
}

/// git is not installed (`KsWqe`): Create unavailable and why, *Check again*, and how to install git here.
fn missing(ctx: &mut Ctx, form: &Form, actions: &Actions, folder: &Path) -> Element {
  let reason = ctx.t("setup.git.create.unavailable");
  let detecting = actions.detect.state().get().is_pending();
  let create = ActionButton::new("git-init-create", ButtonKind::Primary, ctx.t("setup.git.create.create"))
    .icon(Icon::GitBranchPlus)
    .in_form()
    .unavailable_with_icon(reason.clone());
  let unavailable = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .id("git-init-actions")
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(create)
        .child(check_again(ctx, form, actions, folder, detecting)),
    )
    .child(reason_line("git-init-unavailable", &reason));
  let section = section(ctx, true).child(unavailable);
  let section = match INSTALL_GIT {
    Some(command) => section
      .child(Text::new(&ctx.t("setup.git.install.intro")).variant(ExtraTypography::Note.role()))
      .child(command_row(ctx, "git-install-command", command)),
    None => section.child(Text::new(&ctx.t("setup.git.install.unsupported")).variant(ExtraTypography::Note.role())),
  };
  section.into()
}

/// An icon and a line of the block (`Line` `E6oG2`): 12 `text_secondary` at 1.5, the icon in its slot.
fn line(glyph: Icon, color: PaletteColor, text: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(slot(LINE_SLOT, glyph, color))
    .child(
      Text::new(text)
        .variant(TypographyStyle::Description)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// A quieter line, in 11.5 `text_muted` (`t3HjiX`).
fn hint_line(glyph: Icon, text: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(slot(LINE_SLOT, glyph, PaletteColor::TextMuted))
    .child(
      Text::new(text)
        .variant(ExtraTypography::Note.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// A 13 icon centered in a slot as high as the first line next to it.
fn slot((width, height): (f32, f32), glyph: Icon, color: PaletteColor) -> Row {
  Row::new()
    .width(Dimension::Px(width))
    .height(Dimension::Px(height))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color))
}
