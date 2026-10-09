//! The Details pane (`MaWhI` in `oryKr`): in the rail's place, under its head (*Details* and
//! *Steps*, which brings the rail back), the invocation panel of the step picked in the rail, else of
//! the latest launch, or of the step whose tokens were chosen in its divider. Closed by default and
//! remembered for every task (the application database's `ui_state`): the tab's job is to follow and
//! to talk, and the figures are looked up now and then.

use std::path::Path;

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::id::StepExecutionId;

use super::{super::activity::invocation_panel, nav};
use crate::{
  background,
  messages::{self, Message},
  navigation::Navigation,
  tasks::{detail::RunView, transcript::Entry},
  theme::ExtraColor,
  ui::{button::ActionButton, icon::Icon},
  workspace::Workspace,
};

/// The head's height and its *Steps* button's (`Qwrbk`, `z9JnYJ`).
const HEAD_HEIGHT: f32 = 52.0;
const STEPS_HEIGHT: f32 = 28.0;

/// Whether the pane is open and which step it is about.
#[derive(Clone)]
pub(super) struct DetailsPane {
  /// What the person chose in this tab; `None` until they choose, when the remembered state holds.
  pub(super) open: Signal<Option<bool>>,
  /// The step execution whose tokens were chosen in its divider; `None` follows the rail.
  pub(super) on: Signal<DetailsOn>,
  pub(super) save: SaveAction,
}

pub(super) type SaveAction = FutureAction<bool, (), Message>;

/// The step execution Details is about once its tokens were chosen in its divider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DetailsOn(pub(super) Option<StepExecutionId>);

impl lurq::app::component::DevtoolsInspectable for DetailsOn {}

fn workspace(ctx: &mut Ctx) -> Workspace {
  ctx
    .use_context::<Navigation>()
    .expect("the tab host provides Navigation")
    .workspace()
    .clone()
}

/// Where the application database keeps whether the pane is open (`ui_state`), and the two values.
const DETAILS_KEY: &str = "conversation.details";
const DETAILS_OPEN: &str = "open";
const DETAILS_CLOSED: &str = "closed";

/// Remembers whether the pane is open; a failure only loses the choice for the next task, so it is
/// logged.
pub(super) fn save_action(ctx: &mut Ctx) -> SaveAction {
  let workspace = workspace(ctx);
  ctx.future_action(move |open: bool| {
    let workspace = workspace.clone();
    async move {
      background::run(move || {
        let value = if open { DETAILS_OPEN } else { DETAILS_CLOSED };
        workspace.set_ui_state(DETAILS_KEY, value)
      })
      .await?
      .map_err(|error| {
        tracing::warn!(%error, "whether the Details pane is open was not remembered");
        messages::store_error(&error)
      })
    }
  })
}

/// Whether the person kept the pane open last time; closed while that is read, or when it cannot
/// be, which the log says.
pub(super) fn remembered(ctx: &mut Ctx) -> bool {
  let workspace = workspace(ctx);
  let remembered = ctx.future((), move |()| {
    let workspace = workspace.clone();
    async move {
      background::run(move || {
        workspace
          .ui_state(DETAILS_KEY)
          .map(|value| value.as_deref() == Some(DETAILS_OPEN))
      })
      .await?
      .map_err(|error| {
        tracing::warn!(%error, "whether the Details pane is open cannot be read; it is closed");
        messages::store_error(&error)
      })
    }
  });
  remembered.state().get().data.unwrap_or(false)
}

impl DetailsPane {
  /// Whether the pane shows: the person's choice in this tab, else what they chose last.
  pub(super) fn is_open(&self, remembered: bool) -> bool {
    self.open.get().unwrap_or(remembered)
  }

  /// Opens the pane while it is `open` closed, or closes it, and remembers it.
  pub(super) fn toggle(&self, open: bool) -> impl Fn() + Send + Sync + 'static {
    let (chosen, on, save) = (self.open.clone(), self.on.clone(), self.save.clone());
    move || {
      let open = !open;
      chosen.set(Some(open));
      if !open {
        on.set(DetailsOn(None));
      }
      save.run(open);
    }
  }

  /// Opens the pane on `execution`, as choosing its tokens in its divider does.
  pub(super) fn open_on(&self, execution: StepExecutionId) -> impl Fn() + Send + Sync + 'static {
    let (chosen, on, save) = (self.open.clone(), self.on.clone(), self.save.clone());
    move || {
      chosen.set(Some(true));
      on.set(DetailsOn(Some(execution)));
      save.run(true);
    }
  }
}

/// The pane: about the step its tokens opened it on, else the latest execution of the step picked
/// in the rail, `picked`.
pub(super) fn details_pane(
  ctx: &mut Ctx,
  (run, repo_root): (&RunView, &Path),
  entries: &[Entry],
  (details, picked): (&DetailsPane, Option<StepExecutionId>),
) -> Element {
  let on = details.on.get().0.or(picked);
  let nav = nav(ctx);
  let head = head(ctx, details);
  invocation_panel(ctx, (&nav, repo_root), (run, entries), on, head)
}

/// *Details* and *Steps*, which closes the pane and brings the rail back.
fn head(ctx: &Ctx, details: &DetailsPane) -> Element {
  let steps = ActionButton::new(ctx.t("runs.conversation.rail.steps"))
    .icon(Icon::List)
    .id("conversation-details-steps")
    .height(Dimension::Px(STEPS_HEIGHT))
    .on_click(details.toggle(true))
    .render(ctx);
  Row::new()
    .id("conversation-details-head")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding_horizontal(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(&ctx.t("runs.conversation.details"))
        .variant(TypographyStyle::Label)
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(steps)
    .into()
}
