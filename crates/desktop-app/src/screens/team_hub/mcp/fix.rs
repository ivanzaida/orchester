//! The pinned action of the panel of a team MCP server (`Actions` in `C4` `SpRvd` … `C8` `lSA4x`): the fix of the first
//! missing part, and at its end what the person is told about it. A server whose first missing part only the team can
//! change has no bar; one that lacks nothing has *Change path* when it runs a program.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ResourceId,
  secret::SecretName,
  sharing::{Missing, ProgramProblem},
};

use crate::{
  screens::team_hub::actions::Busy,
  theme::{HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    pressable::Press,
  },
};

/// The pinned actions' height (`Actions` `WpN6M`).
const HEIGHT: f32 = 56.0;

/// What a press on the bar does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Step {
  SwitchOn(ResourceId),
  /// Asks for the program's file and sets it, where none is set, its file is gone or the person changes it.
  ChoosePath(ResourceId),
  AddValue(SecretName),
  /// Reads what this computer has again, for a file that came back.
  CheckAgain,
}

/// What the bar offers for the first missing part: the main action, and *Check again* beside it for a program whose
/// file is gone.
pub(super) fn fix_of(part: &Missing) -> Option<(Step, ProgramState)> {
  match part {
    Missing::McpOff { server } => Some((Step::SwitchOn(*server), ProgramState::NotAsked)),
    Missing::McpProgram { server, problem } => Some((
      Step::ChoosePath(*server),
      match problem {
        ProgramProblem::NotSet => ProgramState::NotSet,
        ProgramProblem::FileGone(_) => ProgramState::FileGone,
      },
    )),
    Missing::Secret { name } => Some((Step::AddValue(name.clone()), ProgramState::NotAsked)),
    Missing::Connection { .. }
    | Missing::DecisionConnection { .. }
    | Missing::McpUnsupported { .. }
    | Missing::SecretUnreadable { .. }
    | Missing::LocalMcpProgram { .. }
    | Missing::LocalSecret { .. } => None,
  }
}

/// How the program's path stands, which words the path action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProgramState {
  /// The part is not about a program's path.
  NotAsked,
  NotSet,
  FileGone,
}

/// What the bar needs of the panel: the presses of each step, taken or not.
pub(super) struct Bar<'a> {
  pub missing: &'a [Missing],
  /// A program server that lacks nothing offers *Change path*.
  pub changes_path: Option<ResourceId>,
  pub busy: Busy,
  pub press: &'a dyn Fn(&Step) -> Press,
}

impl Bar<'_> {
  pub(super) fn render(&self, ctx: &Ctx) -> Option<Element> {
    let (main, hint, check_again) = match self.missing.first() {
      Some(part) => {
        let (step, state) = fix_of(part)?;
        let (button, hint) = self.words(ctx, &step, state);
        (button, hint, state == ProgramState::FileGone)
      }
      None => {
        let server = self.changes_path?;
        let button = self.button(
          ctx,
          &Step::ChoosePath(server),
          ButtonKind::Ghost,
          "team_hub.mcp.bar.changePath",
          Icon::FolderOpen,
        );
        (button, Some("team_hub.mcp.bar.changePathHint"), false)
      }
    };
    let again = check_again.then(|| {
      self.button(
        ctx,
        &Step::CheckAgain,
        ButtonKind::Secondary,
        "team_hub.mcp.bar.checkAgain",
        Icon::RefreshCw,
      )
    });
    Some(
      Row::new()
        .id("team-hub-actions")
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(HEIGHT))
        .padding(Padding::horizontal(HubSpacing::Sections.role()))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .background(PaletteColor::SurfacePanel)
        .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(main)
        .with_children(again)
        .child(Row::new().flex(1.0))
        .with_children(hint.map(|key| {
          Text::new(&ctx.t(key))
            .id("team-hub-actions-hint")
            .variant(HubText::Hint.role())
            .nowrap()
        }))
        .into(),
    )
  }

  /// The main button of `step` and the key of the hint at the bar's end.
  fn words(&self, ctx: &Ctx, step: &Step, state: ProgramState) -> (ActionButton, Option<&'static str>) {
    let (key, glyph, hint) = match (step, state) {
      (Step::SwitchOn(_), _) => ("team_hub.mcp.bar.turnOn", Icon::Power, "team_hub.mcp.bar.turnOnHint"),
      (Step::ChoosePath(_), ProgramState::FileGone) => ("team_hub.mcp.bar.chooseAnother", Icon::FolderOpen, ""),
      (Step::ChoosePath(_), _) => (
        "team_hub.mcp.bar.setPath",
        Icon::FolderOpen,
        "team_hub.mcp.bar.setPathHint",
      ),
      (Step::AddValue(_), _) => (
        "team_hub.mcp.bar.addValue",
        Icon::KeyRound,
        "team_hub.mcp.bar.addValueHint",
      ),
      (Step::CheckAgain, _) => ("team_hub.mcp.bar.checkAgain", Icon::RefreshCw, ""),
    };
    (
      self.button(ctx, step, ButtonKind::Primary, key, glyph),
      (!hint.is_empty()).then_some(hint),
    )
  }

  fn button(&self, ctx: &Ctx, step: &Step, kind: ButtonKind, key: &str, glyph: Icon) -> ActionButton {
    let button = ActionButton::new(format!("team-hub-fix-{}", name(step)), kind, ctx.t(key)).icon(glyph);
    match self.busy {
      Busy::Idle => button.press((self.press)(step)),
      Busy::Taking => button.loading(),
      Busy::Reading => button.disabled(),
    }
  }
}

/// What the id of the step's control says it is.
const fn name(step: &Step) -> &'static str {
  match step {
    Step::SwitchOn(_) => "switch-on",
    Step::ChoosePath(_) => "set-path",
    Step::AddValue(_) => "add-value",
    Step::CheckAgain => "check-again",
  }
}
