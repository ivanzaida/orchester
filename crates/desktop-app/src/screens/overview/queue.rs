//! *Queue* (`Queue` `fdyJW` in 01 `LXhun`): the queued tasks in start order (`orchester_workspace::overview::queue`),
//! each with why it waits in the app's words and its place in line; the first five, then *Show all N*. Its head holds
//! *Hold new starts*, which holds or releases the project's new starts as the Tasks list's menu does
//! (`orchester_workspace::tasks::hold`). A line opens its task.

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::layout_kind::Justify,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ProjectId,
  project::TaskKey,
  task_group::{HoldActor, HoldScope},
};
use orchester_workspace::{
  background,
  messages::Message,
  overview::{
    QUEUE_SHOWN,
    queue::{QueuedLine, place},
  },
  tasks::{
    hold::{HoldRefused, hold_in},
    outside::Outside,
  },
};

use super::section::{LINE_HEIGHT, head, key, line, mono_time, section, show_all, title};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList, TaskListTab, TaskTab, TaskView},
  services::Services,
  theme::{ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    pressable::{Press, opening},
  },
};

/// The place's room (`dRvuU`).
const PLACE_WIDTH: f32 = 30.0;

/// The section: `lines` in start order, with Hold new starts for the project at `root`, `held` while they are.
pub(super) fn queue(ctx: &mut Ctx, (project, root): (ProjectId, PathBuf), lines: &[QueuedLine], held: bool) -> Column {
  let hold = ctx.mount_keyed::<HoldStarts>("overview-queue-hold", HoldProps { project, root, held });
  let navigation = Navigation::of(ctx);
  let separator = ctx.t("overview.listSeparator");
  let shown: Vec<Element> = lines
    .iter()
    .take(QUEUE_SHOWN)
    .map(|queued| {
      let task: TaskKey = queued.key.parse().expect("a task's key reads back as a task key");
      let id = format!("overview-queue-{}", queued.key);
      let row = line(&id, LINE_HEIGHT, SpacingSize::Md)
        .child(icon(Icon::Clock3, IconSize::Md, PaletteColor::TextMuted))
        .child(key(&queued.key))
        .child(title(
          &format!("{id}-title"),
          &queued.title,
          ExtraTypography::Strong.role(),
          PaletteColor::TextPrimary,
        ))
        .child(
          Text::new(&message_text(ctx, &queued.reason.message(&separator)))
            .id(format!("{id}-why"))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextSecondary)
            .nowrap(),
        )
        .child(
          Row::new()
            .width(Dimension::Px(PLACE_WIDTH))
            .flex_shrink(0.0)
            .justify(Justify::End)
            .child(mono_time(
              &format!("{id}-place"),
              &message_text(ctx, &place(queued.place)),
            )),
        );
      let route = Route::Project(project, ProjectPage::Task(task, TaskView::of(TaskTab::default())));
      opening(navigation.to(route))
        .id(format!("{id}-open"))
        .width(Dimension::Pct(100.0))
        .child(row)
        .into()
    })
    .collect();
  let more = (lines.len() > QUEUE_SHOWN).then(|| {
    let label = ctx.t_args("overview.queue.showAll", [("count", lines.len().to_string())]);
    let open = navigation.to(Route::Project(
      project,
      ProjectPage::Tasks(TaskList::new(TaskListTab::Active)),
    ));
    show_all("overview-queue-all", &label, Press::Open(open))
  });
  section("overview-queue")
    .child(head(
      "overview-queue",
      &ctx.t("overview.queue.title"),
      Some(lines.len().to_string()),
      Some(hold),
    ))
    .with_children(shown)
    .with_children(more)
}

#[derive(Clone, PartialEq)]
struct HoldProps {
  project: ProjectId,
  root: PathBuf,
  held: bool,
}

impl DevtoolsInspectable for HoldProps {}

/// Holding or releasing the project's new starts.
#[derive(Clone)]
struct Holding {
  root: PathBuf,
  held: bool,
  actor: HoldActor,
}

impl DevtoolsInspectable for Holding {}

/// *Hold new starts*, or *Release new starts* while they are held, with why a change was refused under it.
struct HoldStarts {
  hold: FutureAction<Holding, (), Message>,
}

impl Component for HoldStarts {
  type Props = HoldProps;

  fn create(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let project = ctx.props::<HoldProps>().project;
    let hold = ctx.future_action(move |Holding { root, held, actor }: Holding| {
      let changes = services.changes.clone();
      async move {
        let kept = background::run(move || hold_in(&root, (HoldScope::Project, actor), held)).await?;
        kept.map_err(HoldRefused::message)?;
        // The tasks are read again, the holds with them, as after any change to them.
        changes.record(project, Outside::Tasks);
        Ok(())
      }
    });
    Self { hold }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let HoldProps { root, held, .. } = ctx.props::<HoldProps>().clone();
    let state = self.hold.state().get();
    let (glyph, label) = if held {
      (Icon::Play, ctx.t("overview.queue.release"))
    } else {
      (Icon::CirclePause, ctx.t("overview.queue.hold"))
    };
    let button = ActionButton::new("overview-queue-hold", ButtonKind::Ghost, label).icon(glyph);
    let button = if state.is_pending() {
      button.loading()
    } else {
      let hold = self.hold.clone();
      button.on_click(move || {
        hold.run(Holding {
          root: root.clone(),
          held: !held,
          actor: crate::screens::tasks::actor_now(),
        });
      })
    };
    let failed = state.error.as_ref().map(|error| {
      field_error(
        "overview-queue-hold-error",
        &ctx.t_args("overview.queue.failed", [("reason", &*message_text(ctx, error))]),
      )
    });
    Column::new().child(button).with_children(failed)
  }
}
