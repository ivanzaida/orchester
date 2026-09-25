//! The detected repository block (`XcbNT` in 01.03, states 01.C1–C10 in `GojEY`): heading and
//! status, what detection found, and what to do next. In project setup and in Project settings
//! it sits inside a form, so its buttons and Copy take Tab stops.

mod copy;

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Rect, Row, Spacer, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{GhAdvice, GitHubLogin},
  repository::{Detection, DetectionState},
};

use super::status::command_ids;
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    command_row::{CommandPlace, Copied, command_row},
    github_mark::small_github_mark,
    icon::{Icon, IconSize, icon},
    status_label::status_label,
  },
};

/// Button height (`FvVwL`) and divider thickness (`G4CT0C`).
const BUTTON_HEIGHT: f32 = 36.0;
const DIVIDER: f32 = 1.0;
/// The command that makes a folder a git repository (`SwsjG`). Orchester never runs it.
const GIT_INIT: &str = "git init";

type Handler = Arc<dyn Fn() + Send + Sync>;

/// Where the block is shown; the notes differ once the project exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectionPlace {
  Setup,
  Settings,
}

/// What the block shows: `detection` is `None` while detecting (01.C1).
pub struct DetectionBlock<'a> {
  pub detection: Option<&'a Detection>,
  /// The account the project expects, if it expects one yet.
  pub expected: Option<&'a GitHubLogin>,
  pub project: &'a str,
  pub folder: &'a Path,
  pub place: DetectionPlace,
  pub copied: &'a Signal<Copied>,
}

/// The block's actions. A missing handler disables its button (work is running), except for
/// Add GitHub connection, which is unavailable where no project exists yet.
#[derive(Default)]
pub struct DetectionActions {
  pub check: Option<Handler>,
  pub change_account: Option<Handler>,
  pub choose_program: Option<Handler>,
  pub use_account: Option<Handler>,
  pub add_connection: Option<Handler>,
  /// Links the detected repository (Project settings), when it can be linked.
  pub link: Option<Handler>,
}

impl DetectionBlock<'_> {
  pub fn render(&self, ctx: &Ctx, actions: &DetectionActions) -> Element {
    let state = self.detection.map(|detection| detection.state(self.expected));
    let found = self.detection.and_then(Detection::github);
    let properties = copy::properties(ctx, self.detection, state.as_ref(), self.expected, self.folder);
    let note = copy::note(ctx, state.as_ref(), found, self.project, self.place);
    let mut block = Column::new()
      .id("github-detection")
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Step.role())
      .spacing(SpacingSize::Md)
      .background(PaletteColor::SurfacePanel)
      .border_inside(BorderSize::Sm, ExtraColor::Hairline.role())
      .rounded(RadiusSize::Lg)
      .child(self.header(ctx, state.as_ref()))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Inline.role())
          .with_children(properties),
      );
    let Some(state) = state else {
      // Detecting: the note alone, no divider and no actions (`acpiK`).
      return block.child(footer(&note, Vec::new())).into();
    };
    block = block.child(divider());
    let buttons = self.buttons(ctx, &state, actions);
    match self.command(&state) {
      Some((ids, command)) => block
        .child(note_text(&note))
        .child(command_row(
          ctx,
          ids.0,
          ids.1,
          &command,
          CommandPlace::Dialog,
          self.copied,
        ))
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            // The buttons are as tall as the row; their focus rings reach past it.
            .overflow_visible()
            .spacing(SpacingSize::Sm)
            .justify(Justify::End)
            .align_items(Alignment::Center)
            .with_children(buttons),
        )
        .into(),
      None => block.child(footer(&note, buttons)).into(),
    }
  }

  /// Icon, heading and status label (`c38d9D`).
  fn header(&self, ctx: &Ctx, state: Option<&DetectionState>) -> Element {
    let found = self.detection.and_then(Detection::github);
    let mark = if matches!(state, Some(DetectionState::NotAGitRepository)) {
      icon(Icon::FolderX, IconSize::Option, PaletteColor::TextPrimary).into()
    } else {
      small_github_mark(ctx, PaletteColor::TextPrimary)
    };
    let (status_icon, status_key) = copy::status(state);
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .child(mark)
      .child(
        Text::new(&copy::heading(ctx, state, found))
          .id("github-detection-heading")
          .variant(TypographyStyle::Label),
      )
      .child(Spacer::new().flex(1.0))
      .child(status_label(status_icon, &ctx.t(status_key)))
      .into()
  }

  /// The command that fixes the state, with the ids of its row and Copy button.
  fn command(&self, state: &DetectionState) -> Option<((&'static str, &'static str), String)> {
    if matches!(state, DetectionState::NotAGitRepository) {
      return Some((("git-init-command", "git-init-command-copy"), GIT_INIT.to_owned()));
    }
    let found = self.detection.and_then(Detection::github)?;
    let advice = match state {
      DetectionState::NotSignedIn => GhAdvice::Login,
      DetectionState::WrongAccount { expected, .. } => GhAdvice::Switch(Some(expected.clone())),
      DetectionState::MissingScope { .. } => GhAdvice::Refresh,
      _ => return None,
    };
    Some((command_ids(&advice), advice.command(found.repo().host())))
  }

  /// Check again, then what the state offers (`FvVwL`, `w5XOc`, `pjQvl`, `W88kb`, `u7tRBX`,
  /// `HN7cO`), then Link repository where the project can link it.
  fn buttons(&self, ctx: &Ctx, state: &DetectionState, actions: &DetectionActions) -> Vec<Element> {
    let mut buttons = vec![button(
      ctx,
      "detection-check",
      ctx.t("github.checkAgain"),
      Icon::RefreshCw,
      actions.check.clone(),
    )];
    let change = || {
      button(
        ctx,
        "detection-change-account",
        ctx.t("github.detection.changeAccount"),
        Icon::UserCog,
        actions.change_account.clone(),
      )
    };
    match state {
      DetectionState::Detected { .. } | DetectionState::NoAccess { .. } => buttons.push(change()),
      DetectionState::NotGitHub => {
        let add = ActionButton::new(ctx.t("github.detection.addConnection"))
          .icon(Icon::Plus)
          .id("detection-add-connection")
          .in_form()
          .height(Dimension::Px(BUTTON_HEIGHT));
        buttons.push(match actions.add_connection.clone() {
          Some(handler) => add.on_click(move || handler()).render(ctx),
          // No project exists yet to own the connection: unavailable, with its lock.
          None => add.render(ctx),
        });
      }
      DetectionState::GhNotFound => buttons.push(button(
        ctx,
        "detection-choose-program",
        ctx.t("github.connection.chooseProgram"),
        Icon::FileSearch,
        actions.choose_program.clone(),
      )),
      DetectionState::WrongAccount { active, .. } => {
        buttons.push(change());
        buttons.push(button(
          ctx,
          "detection-use-account",
          ctx.t_args("github.connection.useAccount", [("account", active.as_str())]),
          Icon::UserCheck,
          actions.use_account.clone(),
        ));
      }
      DetectionState::NotAGitRepository if self.place == DetectionPlace::Setup => buttons.push(
        ActionButton::new(ctx.t("setup.project.create"))
          .primary()
          .icon(Icon::ArrowRight)
          .id("detection-create")
          .in_form()
          .height(Dimension::Px(BUTTON_HEIGHT))
          .on_click(|| {})
          .enabled(false)
          .render(ctx),
      ),
      _ => {}
    }
    if let Some(link) = actions.link.clone() {
      buttons.push(
        ActionButton::new(ctx.t("github.detection.link"))
          .primary()
          .icon(Icon::GitPullRequest)
          .id("detection-link")
          .in_form()
          .height(Dimension::Px(BUTTON_HEIGHT))
          .on_click(move || link())
          .render(ctx),
      );
    }
    buttons
  }
}

/// The note beside the actions (`sMKsD`).
fn footer(note: &str, buttons: Vec<Element>) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    // The buttons are as tall as the row; their focus rings reach past it.
    .overflow_visible()
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(note_text(note).flex(1.0))
    .with_children(buttons)
    .into()
}

fn note_text(note: &str) -> Text {
  Text::new(note)
    .id("github-detection-note")
    .variant(ExtraTypography::Detail.role())
}

fn divider() -> Element {
  Rect::new(Dimension::Pct(100.0), Dimension::Px(DIVIDER))
    .background(ExtraColor::Hairline.role())
    .into()
}

/// A secondary form button, disabled without a handler.
fn button(ctx: &Ctx, id: &'static str, text: Arc<str>, glyph: Icon, handler: Option<Handler>) -> Element {
  let button = ActionButton::new(text)
    .icon(glyph)
    .id(id)
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT));
  match handler {
    Some(handler) => button.on_click(move || handler()).render(ctx),
    // Disabled, not unavailable: it waits for the running work.
    None => button.on_click(|| {}).enabled(false).render(ctx),
  }
}
