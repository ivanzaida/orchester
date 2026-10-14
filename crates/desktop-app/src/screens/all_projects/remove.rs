//! *Remove … from the list?* in place under the project's row (`Remove confirmation` `J4XaiO` in `kwpO2`):
//! that Orchester only forgets the project, the folder and its `.orchester` data staying on disk, then Cancel
//! and *Remove from list*. Enter confirms, Escape or Cancel keeps the project. While agents are at work in it,
//! *Remove from list* is unavailable and the question says why at the left of the actions (`nrFbI`).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::keyboard_event::KeyboardEvent,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_store::RegisteredProject;
use orchester_workspace::{
  messages::Message,
  project_list::{self, ListedProject},
};
use parking_lot::Mutex;

use super::{Ask, Removing};
use crate::{
  project::Projects,
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize, ProjectsSpacing, ProjectsText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
  },
};

/// Longest folder path shown in full; a longer one keeps its drive and last folders.
const PATH_CHARS: usize = 72;
/// The slot that keeps the problem's icon with its first line (`Icon slot` `OdFme`).
const PROBLEM_SLOT: f32 = 18.0;

pub(super) fn question(ctx: &mut Ctx, listed: &ListedProject, ask: &Ask) -> Element {
  let props = QuestionProps {
    project: listed.project.clone(),
    refused: listed.removal_refused(),
    ask: ask.clone(),
  };
  ctx.mount_keyed::<Question>(&format!("remove-project-{}", listed.project.id), props)
}

#[derive(Clone)]
struct QuestionProps {
  project: RegisteredProject,
  /// Why it cannot be removed now: agents are at work in it.
  refused: Option<Message>,
  ask: Ask,
}

impl PartialEq for QuestionProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.refused == other.refused
  }
}

impl DevtoolsInspectable for QuestionProps {}

/// The project to forget.
#[derive(Clone, Debug, PartialEq)]
struct Target(RegisteredProject);

impl DevtoolsInspectable for Target {}

/// The project forgotten.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Removed(ProjectId);

impl DevtoolsInspectable for Removed {}

type RemoveAction = FutureAction<Target, Removed, Message>;

struct Question {
  /// Whether the removal's end is followed already; once per question.
  followed: Mutex<bool>,
}

impl Component for Question {
  type Props = QuestionProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      followed: Mutex::new(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // First in every render: lurq keeps a component's futures by the order it asks for them.
    let action = remove_action(ctx);
    self.follow(ctx, &action);
    let props = ctx.props::<QuestionProps>().clone();
    let state = action.state().get();
    let pending = state.is_pending();
    let problem = props
      .refused
      .clone()
      .or_else(|| state.error.clone().filter(|_| !pending && state.is_rejected()));
    let confirm = (props.refused.is_none() && !pending).then(|| {
      let (action, project) = (action.clone(), props.project.clone());
      Arc::new(move || action.run(Target(project.clone()))) as Arc<dyn Fn() + Send + Sync>
    });
    let name = props.project.name.to_string();
    let text = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .child(
        Text::new(&ctx.t_args("projects.removal.title", [("name", name.as_str())]))
          .id("remove-project-title")
          .variant(ProjectsText::Heading.role()),
      )
      .child(Text::new(&ctx.t("projects.removal.explanation")).variant(TypographyStyle::Description));
    let cancel = props.ask.removing.clone();
    Column::new()
      .id("remove-project-question")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Tight.role())
          .right(SpacingSize::Md)
          .bottom(ExtraSpacing::Inset.role())
          .left(ProjectsSpacing::RowIndent.role()),
      )
      .spacing(ExtraSpacing::Loose.role())
      .on_key_down(keys(confirm.clone(), cancel))
      .child(text)
      .child(kept(ctx, &props.project))
      .child(actions(ctx, &props, problem.as_ref(), confirm, pending))
  }
}

impl Question {
  /// Forgotten: the list is read again, and a tab that shows the project finds it gone.
  fn follow(&self, ctx: &mut Ctx, action: &RemoveAction) {
    let mut followed = self.followed.lock();
    if *followed {
      return;
    }
    *followed = true;
    let ask = ctx.props::<QuestionProps>().ask.clone();
    let projects = Projects::of(ctx);
    ctx.watch(&action.state(), move |state| {
      let Some(Removed(project)) = state.data.filter(|_| state.is_fulfilled() && !state.is_pending()) else {
        return;
      };
      ask.removing.set(Removing(None));
      ask.revision.update(|revision| *revision += 1);
      projects.retry(project);
    });
  }
}

/// Enter confirms when the project can be removed; Escape keeps it.
fn keys(
  confirm: Option<Arc<dyn Fn() + Send + Sync>>,
  removing: lurq::core::Signal<Removing>,
) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  move |event: KeyboardEvent| {
    if event.text_input_focused || event.composing {
      return;
    }
    match event.key.as_str() {
      "Escape" => {
        removing.set(Removing(None));
        event.prevent_default();
      }
      "Enter" => {
        if let Some(confirm) = &confirm {
          confirm();
        }
        event.prevent_default();
      }
      _ => {}
    }
  }
}

/// The folder and the project's data, both staying on disk (`Kept` `Ud5Fy`).
fn kept(ctx: &Ctx, project: &RegisteredProject) -> Column {
  let line = |glyph, value: &str, words: &str| {
    Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
      .child(Text::new(value).variant(ProjectsText::Path.role()))
      .child(Text::new(words).variant(ExtraTypography::Note.role()))
  };
  Column::new()
    .id("remove-project-kept")
    .spacing(ExtraSpacing::Snug.role())
    .child(line(
      Icon::Folder,
      &orchester_workspace::display::shorten_path(&project.repo_root, PATH_CHARS),
      &ctx.t("projects.removal.staysOnDisk"),
    ))
    .child(line(
      Icon::Database,
      &ctx.t("projects.removal.data"),
      &ctx.t("projects.removal.dataStays"),
    ))
}

/// Why it cannot be removed, at the left; Cancel and *Remove from list* (`Actions` `cRo0R`, `nrFbI`).
fn actions(
  ctx: &Ctx,
  props: &QuestionProps,
  problem: Option<&Message>,
  confirm: Option<Arc<dyn Fn() + Send + Sync>>,
  pending: bool,
) -> Row {
  let left: Element = match problem {
    Some(problem) => Row::new()
      .id("remove-project-problem")
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Start)
      .child(
        Row::new()
          .height(Dimension::Px(PROBLEM_SLOT))
          .align_items(Alignment::Center)
          .child(icon(Icon::CircleAlert, IconSize::Base, PaletteColor::TextSecondary)),
      )
      .child(
        Text::new(&message_text(ctx, problem))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextPrimary)
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .into(),
    None => Spacer::new().flex(1.0).into(),
  };
  let removing = props.ask.removing.clone();
  let cancel = ActionButton::new(
    "remove-project-cancel",
    ButtonKind::Secondary,
    ctx.t("projects.removal.cancel"),
  )
  .in_form()
  .on_click(move || removing.set(Removing(None)));
  let label = if pending {
    ctx.t("projects.removal.removing")
  } else {
    ctx.t("projects.removal.confirm")
  };
  let remove = ActionButton::new("remove-project-confirm", ButtonKind::Primary, label)
    .icon(Icon::X)
    .in_form();
  let remove = match (pending, confirm) {
    (true, _) => remove.loading(),
    (false, Some(confirm)) => remove.on_click(move || confirm()),
    (false, None) => remove.disabled(),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(left)
    .child(cancel)
    .child(remove)
}

fn remove_action(ctx: &mut Ctx) -> RemoveAction {
  let services = Services::of(ctx);
  let (workspace, runners) = (services.workspace, services.runners);
  ctx.future_action(move |Target(project): Target| {
    let (workspace, runners) = (workspace.clone(), runners.clone());
    async move {
      let id = project.id;
      project_list::remove(&workspace, &runners, &project)
        .await
        .map(|()| Removed(id))
    }
  })
}
