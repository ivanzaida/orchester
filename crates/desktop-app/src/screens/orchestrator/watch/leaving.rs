//! *Before you leave* (`Parts · Before you leave` `k0jNa` a–d on the board `Orchestrator autonomy · frames to
//! approve`; under the leading line in `L01` `QMwUB`, inside the `/watching` answer in `L02` `HpcQJ`): what would stop
//! the work a conversation leads to the end while nobody is at the computer
//! (`orchester_workspace::orchestrator::leaving`, `docs/plan/orchestrator-autonomy/OA-19-before-you-leave.md`).
//!
//! A card on `warning_muted` in a `warning` stroke, radius md, 10 over and under and 12 at its sides, its parts 8
//! apart: a 13 `door-open` in `warning`, *Before you leave* (12.5/600), how many things can stop the target (12
//! `text_secondary`) and *Check again* (12/500 `text_secondary`); under them, 21 in and 6 apart, a line per finding in
//! 12 `text_secondary` with the place that fixes it at its end (12/500 `text_primary`), a finding without such a place
//! with none, and last, muted, what the app cannot check. With nothing found it is one quiet line, *Ready to leave*,
//! which still names what was not checked. For a target the conversation only follows there is nothing at all.
//!
//! It is read when it shows and again on *Check again*, off the UI thread, with no agent turn; nothing polls it.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ChatId, ProjectId},
  orchestrator::WatchTarget,
  project::TaskKey,
};
use orchester_store::StoreError;
use orchester_workspace::{
  background,
  messages::{self, Message},
  orchestrator::leaving::{AppSet, Leaving, NotChecked, Weak, leaving_of},
};

use super::{Said, line_row};
use crate::{
  navigation::Navigation,
  project::settled_read::settled_read,
  routes::{PipelinesFilter, PipelinesView, ProjectPage, ProjectSettingsTab, Route, TaskTab, TaskView},
  screens::orchestrator::timeline::TurnEnv,
  services::Services,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, OrchestratorSpacing, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::{Press, pressed},
  },
};

/// What one *Before you leave* is read for and where it shows.
#[derive(Clone, PartialEq)]
pub(in crate::screens::orchestrator) struct LeavingProps {
  id: String,
  root: PathBuf,
  project: ProjectId,
  chat: ChatId,
  target: WatchTarget,
  /// The target as the card names it.
  name: Arc<str>,
  /// The room over the card or the line, which is there only while one of them shows.
  over: SpacingSize,
}

impl DevtoolsInspectable for LeavingProps {}

/// *Before you leave* for each of `led`, the targets conversation `env` shows leads to the end, each by its name;
/// `id` names them, and `over` is the room over each. Nothing for a conversation that is not kept yet.
pub(in crate::screens::orchestrator) fn cards(
  ctx: &mut Ctx,
  (id, over): (&str, SpacingSize),
  env: &TurnEnv,
  led: impl IntoIterator<Item = (WatchTarget, Arc<str>)>,
) -> Vec<Element> {
  let Some(chat) = env.scope.as_ref().map(|scope| scope.chat) else {
    return Vec::new();
  };
  led
    .into_iter()
    .enumerate()
    .map(|(at, (target, name))| {
      let id = format!("{id}-leaving-{at}");
      ctx.mount_keyed::<BeforeYouLeave>(
        &id.clone(),
        LeavingProps {
          id,
          root: env.root.clone(),
          project: env.project,
          chat,
          target,
          name,
          over,
        },
      )
    })
    .collect()
}

/// What was read; `None` for a target the conversation does not lead to the end.
#[derive(Clone, PartialEq)]
struct Read(Option<Arc<Leaving>>);

impl DevtoolsInspectable for Read {}

pub(in crate::screens::orchestrator) struct BeforeYouLeave {
  /// How often the person asked to check again.
  again: Signal<u64>,
}

impl Component for BeforeYouLeave {
  type Props = LeavingProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self { again: ctx.signal(0) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LeavingProps>().clone();
    let services = Services::of(ctx);
    let deps = (props.root.clone(), props.chat, props.target, self.again.get());
    let state = settled_read(ctx, "before you leave", deps, move |(root, chat, target, _)| {
      let (workspace, programs) = (services.workspace.clone(), services.programs.clone());
      async move {
        background::run(move || {
          let set = || -> Result<AppSet, StoreError> {
            Ok(AppSet {
              drives: workspace.orchestrator_drives(&root)?,
              keeps_awake: workspace.keeps_awake()?,
            })
          };
          let app = set().map_err(|error| messages::store_error(&error))?;
          leaving_of((&root, &programs), (chat, target), app).map(|leaving| Read(leaving.map(Arc::new)))
        })
        .await
        .and_then(|read| read)
      }
    });
    let again = self.again.clone();
    let check = action(
      &format!("{}-again", props.id),
      (
        &ctx.t("orchestrator.leaving.checkAgain"),
        Some(PaletteColor::TextSecondary),
      ),
      Press::run(move || again.update(|asked| *asked += 1)),
    );
    // A reading that failed says so even when an earlier one succeeded; the last one stays while it is read again.
    let rejected = state.is_rejected();
    let shown: Option<Element> = match (state.data, state.error) {
      (_, Some(why)) if rejected => Some(unreadable(ctx, &props, &why, check)),
      (Some(Read(Some(leaving))), _) if leaving.ready() => Some(ready(ctx, &props, &leaving, check)),
      (Some(Read(Some(leaving))), _) => Some(card(ctx, &props, &leaving, check)),
      (None, Some(why)) => Some(unreadable(ctx, &props, &why, check)),
      (Some(Read(None)) | None, _) => None,
    };
    let room = Column::new().width(Dimension::Pct(100.0));
    match shown {
      Some(shown) => room.padding(Padding::new().top(props.over)).child(shown),
      None => room,
    }
  }
}

/// The quiet line of a reading that failed: why, and *Check again*.
fn unreadable(ctx: &Ctx, props: &LeavingProps, why: &Message, check: Element) -> Element {
  let reason = message_text(ctx, why);
  let said = Said {
    glyph: (Icon::DoorOpen, PaletteColor::TextMuted),
    words: ctx.t("orchestrator.leaving.title"),
    tag: None,
    aside: Some(ctx.t_args("orchestrator.leaving.unreadable", [("reason", &*reason)])),
  };
  line_row(&props.id, "leaving_unreadable", said, Some(check)).into()
}

/// *Ready to leave*: nothing found, and what was not checked.
fn ready(ctx: &Ctx, props: &LeavingProps, leaving: &Leaving, check: Element) -> Element {
  let target = ("target", props.name.to_string());
  let aside = match leaving.not_checked.first() {
    Some(not) => {
      let unchecked = ctx.t(match not {
        NotChecked::Servers => "orchestrator.leaving.unchecked.servers",
      });
      ctx.t_args(
        "orchestrator.leaving.readyAside",
        [target, ("unchecked", unchecked.to_string())],
      )
    }
    None => ctx.t_args("orchestrator.leaving.readyAsideAll", [target]),
  };
  let said = Said {
    glyph: (Icon::DoorOpen, PaletteColor::TextMuted),
    words: ctx.t("orchestrator.leaving.ready"),
    tag: None,
    aside: Some(aside),
  };
  line_row(&props.id, "ready_to_leave", said, Some(check)).into()
}

/// The card: its head, a line per finding, and what was not checked.
fn card(ctx: &mut Ctx, props: &LeavingProps, leaving: &Leaving, check: Element) -> Element {
  let mut lines: Vec<Element> = Vec::new();
  let mut stopping = 0;
  for weak in &leaving.weak {
    for (words, place) in findings(ctx, props.project, weak) {
      // Closing the window is what the person must not do, not something found wrong.
      stopping += usize::from(*weak != Weak::StopsWithWindow);
      let id = format!("{}-finding-{}", props.id, lines.len());
      lines.push(finding(ctx, (&id, weak.name()), (&words, None), place));
    }
  }
  for not in &leaving.not_checked {
    let words = ctx.t(match not {
      NotChecked::Servers => "orchestrator.leaving.notChecked.servers",
    });
    let id = format!("{}-not-checked-{}", props.id, lines.len());
    let muted = Some(PaletteColor::TextMuted);
    lines.push(finding(ctx, (&id, "not_checked"), (&words, muted), None));
  }
  let count = if stopping == 1 {
    "orchestrator.leaving.count.one"
  } else {
    "orchestrator.leaving.count.other"
  };
  let count = ctx.t_args(
    count,
    [("count", stopping.to_string()), ("target", props.name.to_string())],
  );
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::DoorOpen, IconSize::Base, PaletteColor::Warning).flex_shrink(0.0))
    .child(
      Text::new(&ctx.t("orchestrator.leaving.title"))
        .variant(OrchestratorText::LeavingTitle.role())
        .nowrap()
        .flex_shrink(0.0),
    )
    .child(
      Text::new(&count)
        .id(format!("{}-count", props.id))
        .variant(OrchestratorText::LineEnd.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(check);
  Column::new()
    .id(props.id.clone())
    .describe("outcome", "before_you_leave")
    .describe("stopping", stopping.to_string())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      OrchestratorSpacing::LeavingSide.role(),
      ExtraSpacing::Loose.role(),
    ))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::WarningMuted)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Warning))
    .rounded(RadiusSize::Md)
    .child(head)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().left(OrchestratorSpacing::LeavingIndent.role()))
        .spacing(ExtraSpacing::Snug.role())
        .with_children(lines),
    )
    .into()
}

/// The place that fixes a finding: its link's words and where it goes.
type Place = (Arc<str>, Route);

/// The lines of `weak`, each with its words and the place that fixes it; a finding with no such place in the app has
/// none. Pipelines without a Deliver step are a line each.
fn findings(ctx: &Ctx, project: ProjectId, weak: &Weak) -> Vec<(Arc<str>, Option<Place>)> {
  let page = |page: ProjectPage| Route::Project(project, page);
  let settings = |tab: ProjectSettingsTab| {
    Some((
      ctx.t("orchestrator.leaving.link.projectSettings"),
      page(ProjectPage::Settings(tab)),
    ))
  };
  let one = |words: Arc<str>, place: Option<Place>| vec![(words, place)];
  match weak {
    Weak::DriveOff => one(
      ctx.t("orchestrator.leaving.driveOff"),
      settings(ProjectSettingsTab::Orchestrator),
    ),
    Weak::OrchestratorNotReady(_) => {
      let place = (
        ctx.t("orchestrator.leaving.link.orchestratorSettings"),
        page(ProjectPage::OrchestratorSettings),
      );
      one(ctx.t("orchestrator.leaving.orchestratorNotReady"), Some(place))
    }
    Weak::NoPipeline(tasks) => {
      let key = if tasks.len() == 1 {
        "orchestrator.leaving.noPipeline.one"
      } else {
        "orchestrator.leaving.noPipeline.other"
      };
      // The first of them is the place: each is fixed on its own page.
      let place = tasks.first().and_then(|task| {
        let key: TaskKey = task.parse().ok()?;
        let words = ctx.t_args("orchestrator.leaving.link.task", [("task", task.as_str())]);
        Some((words, page(ProjectPage::Task(key, TaskView::of(TaskTab::Summary)))))
      });
      one(ctx.t_args(key, [("tasks", tasks.join(", "))]), place)
    }
    Weak::NoDeliverStep { tasks, pipelines } if pipelines.is_empty() => one(
      ctx.t_args("orchestrator.leaving.noDeliverStepOf", [("tasks", tasks.join(", "))]),
      None,
    ),
    Weak::NoDeliverStep { pipelines, .. } => pipelines
      .iter()
      .map(|pipeline| {
        let view = PipelinesView::new(PipelinesFilter::default(), Some(pipeline.id));
        let place = (
          ctx.t("orchestrator.leaving.link.pipeline"),
          page(ProjectPage::PipelinesHub(view)),
        );
        let words = ctx.t_args("orchestrator.leaving.noDeliverStep", [("pipeline", &*pipeline.name)]);
        (words, Some(place))
      })
      .collect(),
    // The person's own checkout: nothing in the app commits or stashes it.
    Weak::CheckoutHasChanges { .. } => one(ctx.t("orchestrator.leaving.checkoutHasChanges"), None),
    Weak::NoDailyBudget => one(
      ctx.t("orchestrator.leaving.noDailyBudget"),
      settings(ProjectSettingsTab::Runs),
    ),
    Weak::MaySleep => one(
      ctx.t("orchestrator.leaving.maySleep"),
      Some((ctx.t("orchestrator.leaving.link.settings"), Route::General)),
    ),
    Weak::StopsWithWindow => one(ctx.t("orchestrator.leaving.stopsWithWindow"), None),
  }
}

/// One line of the card, named `id`: `words`, in `color` when they are not a finding's, and at its end the place
/// that fixes it; `kind` is what tools read of it.
fn finding(
  ctx: &mut Ctx,
  (id, kind): (&str, &'static str),
  (words, color): (&str, Option<PaletteColor>),
  place: Option<Place>,
) -> Element {
  let place = place.map(|(label, route)| {
    let open = Press::Open(Navigation::of(ctx).to(route));
    action(&format!("{id}-place"), (&label, None), open)
  });
  let words = Text::new(words)
    .id(format!("{id}-words"))
    .variant(OrchestratorText::LineEnd.role())
    .flex(1.0)
    .min_width(Dimension::Px(0.0));
  Row::new()
    .id(id.to_owned())
    .describe("finding", kind)
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(match color {
      Some(color) => words.color(color),
      None => words,
    })
    .with_children(place)
    .into()
}

/// Words that do something, named `id`, in 12/500 `text_primary` or the quieter `color`, the `hover` fill under
/// the pointer.
fn action(id: &str, (words, color): (&str, Option<PaletteColor>), press: Press) -> Element {
  let label = Text::new(words).variant(OrchestratorText::LeavingLink.role()).nowrap();
  pressed(press)
    .id(id.to_owned())
    .describe("a11y_name", words.to_owned())
    .flex_shrink(0.0)
    .rounded(ExtraRadius::Link.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .cursor(CursorIcon::Pointer)
    .child(match color {
      Some(color) => label.color(color),
      None => label,
    })
    .into()
}
