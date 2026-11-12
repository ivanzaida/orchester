//! A hold the Orchestrator's agent put on new starts (`Held` `zxw1l` in `4 · Report and the rest` `VZKyw`): a
//! `warning_muted` line, radius 6, 36 high, 10 in at its start and 6 at its end, its parts 10 apart: a 13 `pause` in
//! `warning`, what it held (the whole project, or the group of the task it named) in 12.5, how many tasks wait and
//! their keys in 12 `text_muted`, when, and *Resume new starts* (the person's release, as Tasks releases it). That no
//! waiting task starts until the person resumes, and that it was checked as *Pause scheduling* in the queue, is its
//! tooltip.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
    theme::{PaletteColor, RadiusSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::HeldWork,
  task_group::{HoldActor, HoldScope},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  background, display,
  messages::Message,
  tasks::hold::{HoldRefused, hold_in},
};

use crate::{
  mcp::input_gate::InputGate,
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize, OrchestratorText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    tooltip::Tooltip,
  },
};

/// The line's height (`zxw1l`), and the keys of waiting tasks it names before it counts the rest.
const HEIGHT: f32 = 36.0;
const NAMED: usize = 3;

#[derive(Clone, PartialEq)]
pub(super) struct HeldProps {
  pub(super) id: String,
  pub(super) project: (ProjectId, String),
  pub(super) root: PathBuf,
  /// The task the hold named, whose group it held; `None` for the whole project.
  pub(super) task: Option<String>,
  pub(super) held: HeldWork,
  pub(super) at: Timestamp,
}

impl DevtoolsInspectable for HeldProps {}

/// A release of what the line held.
#[derive(Clone)]
struct Resuming {
  root: PathBuf,
  scope: HoldScope,
  /// Who pressed: a person, or automation driving the window.
  actor: HoldActor,
}

impl DevtoolsInspectable for Resuming {}

pub(super) struct HeldCard {
  resume: FutureAction<Resuming, (), Message>,
  gate: Option<InputGate>,
}

impl Component for HeldCard {
  type Props = HeldProps;

  fn create(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let project = ctx.props::<HeldProps>().project.0;
    let resume = ctx.future_action(move |Resuming { root, scope, actor }: Resuming| {
      let changes = services.changes.clone();
      async move {
        let released = background::run(move || hold_in(&root, (scope, actor), false)).await?;
        released.map_err(HoldRefused::message)?;
        changes.record(project, orchester_workspace::tasks::outside::Outside::Tasks);
        Ok(())
      }
    });
    Self {
      resume,
      gate: ctx.use_context::<InputGate>(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HeldProps>().clone();
    let id = props.id.as_str();
    let name = props.project.1.as_str();
    let (title, effect) = match &props.task {
      Some(task) => (
        ctx.t_args("orchestrator.held.groupTitle", [("task", task.as_str())]),
        ctx.t("orchestrator.held.groupEffect"),
      ),
      None => (
        ctx.t_args("orchestrator.held.projectTitle", [("project", name)]),
        ctx.t_args("orchestrator.held.projectEffect", [("project", name)]),
      ),
    };
    let tip: Arc<str> = ctx.t_args(
      "orchestrator.held.tip",
      [
        ("effect", &*effect),
        (
          "checked",
          &*ctx.t_args("orchestrator.held.checked", [("project", name)]),
        ),
      ],
    );
    let line = Row::new()
      .id(id.to_owned())
      .describe("outcome", "held")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(HEIGHT))
      .padding(
        Padding::new()
          .left(ExtraSpacing::Loose.role())
          .right(ExtraSpacing::Snug.role()),
      )
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .background(PaletteColor::WarningMuted)
      .rounded(RadiusSize::Md)
      .child(icon(Icon::Pause, IconSize::Base, PaletteColor::Warning))
      .child(
        Text::new(&title)
          .id(format!("{id}-title"))
          .variant(OrchestratorText::Line.role())
          .nowrap()
          .flex_shrink(0.0),
      )
      .child(
        Text::new(&waiting(ctx, &props.held))
          .id(format!("{id}-waiting"))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .flex_shrink(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(Spacer::new().flex(1.0))
      .child(
        Text::new(&display::clock(props.at))
          .variant(ExtraTypography::MonoNote.role())
          .nowrap(),
      )
      .child(self.resume_button(ctx, &props));
    let failed = self.resume.state().get().error.map(|error| {
      Text::new(&message_text(ctx, &error))
        .id(format!("{id}-error"))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::Danger)
        .width(Dimension::Pct(100.0))
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(Tooltip::new(format!("{id}-tip"), tip).wrap(ctx, line))
      .with_children(failed)
  }
}

impl HeldCard {
  /// *Resume new starts*, primary (`zxw1l`); it takes no press while the release is under way.
  fn resume_button(&self, ctx: &Ctx, props: &HeldProps) -> ActionButton {
    let resume = ActionButton::new(
      format!("{}-resume", props.id),
      ButtonKind::Primary,
      ctx.t("orchestrator.held.resume"),
    )
    .icon(Icon::Play)
    .small();
    if self.resume.state().get().is_pending() {
      return resume.loading();
    }
    let (action, root, gate) = (self.resume.clone(), props.root.clone(), self.gate.clone());
    let scope = props.held.group.map_or(HoldScope::Project, HoldScope::Group);
    resume.on_click(move || {
      // The line is the chat's, which keeps no word of who holds the scope now: an MCP client's press reads it.
      let by_person = is_synthetic_input() && held_by_person(&root, scope);
      if let Some(actor) = crate::screens::tasks::hold_actor(gate.as_ref(), false, by_person) {
        action.run(Resuming {
          root: root.clone(),
          scope,
          actor,
        });
      }
    })
  }
}

/// Whether a person set the hold on `scope` in the project at `root`, as stored now. Read in the handler of an MCP
/// client's press only, a single query; a read that fails says no, and the workspace refuses the release itself.
fn held_by_person(root: &Path, scope: HoldScope) -> bool {
  match ProjectDb::open(root).and_then(|db| db.queue_holds()) {
    Ok(holds) => holds.by_person.contains(&scope),
    Err(error) => {
      tracing::warn!(%error, "who holds new starts could not be read");
      false
    }
  }
}

/// How many tasks wait, with the first keys and how many more: `5 waiting · KON-8, KON-17, KON-18 +2`.
fn waiting(ctx: &Ctx, held: &HeldWork) -> Arc<str> {
  let count = held.waiting.len();
  if count == 0 {
    return ctx.t("orchestrator.held.waiting.none");
  }
  let mut keys = held.waiting[..count.min(NAMED)].join(&ctx.t("orchestrator.held.separator"));
  if count > NAMED {
    keys = ctx
      .t_args(
        "orchestrator.held.more",
        [("keys", keys), ("count", (count - NAMED).to_string())],
      )
      .to_string();
  }
  ctx.t_args(
    "orchestrator.held.waitingKeys",
    [("count", count.to_string()), ("keys", keys)],
  )
}
