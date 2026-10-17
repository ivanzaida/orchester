//! *Share KON-11 on KTR?* in place over the Summary (`Not shared · Share task · confirm in place` `TfVtK`): where the
//! task goes and as which key, the server's similar task, what happens to its tags and links, and that its runs,
//! conversation and memory stay here. *Share as KTR-45*, which only a person presses, uploads it through the same
//! import as joining; the page then follows the task to its new key. *Keep it here* puts the question away.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{id::IdempotencyKey, project::TaskKey};
use orchester_workspace::{background, sharing::project::ProjectServer};

use super::Linked;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  screens::{
    settings::orchester::{PersonOnly, Said, sharing_said},
    task::{PageState, TaskContext},
  },
  services::Services,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
  },
};

/// *Share as KTR-45*, which only a person presses.
const SHARE: &str = "task-share-confirm";

#[derive(Clone, PartialEq)]
pub struct Props {
  pub task: TaskContext,
  pub linked: Option<Linked>,
  pub next: Option<TaskKey>,
}

impl DevtoolsInspectable for Props {}

/// The key the task was shared as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedAs(TaskKey);

impl DevtoolsInspectable for SharedAs {}

pub struct ShareQuestion {
  share: FutureAction<(), SharedAs, Said>,
  person: PersonOnly,
}

impl Component for ShareQuestion {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let (servers, root, task) = (
      Services::of(ctx).servers,
      props.task.project.open.repo_root.clone(),
      props.task.detail.task.id(),
    );
    let share = ctx.future_action(move |(): ()| {
      let (servers, root) = (servers.clone(), root.clone());
      async move {
        let server = background::run(move || ProjectServer::open(&servers, &root))
          .await
          .map_err(|_| Said::new("cloud.errors.local"))?
          .map_err(|error| sharing_said(&error))?
          .ok_or_else(|| Said::new("sharing.linked.notLinked"))?;
        server
          .share_kept(task, IdempotencyKey::generate())
          .await
          .map(SharedAs)
          .map_err(|error| sharing_said(&error))
      }
    });
    let (navigation, project, feed) = (
      Navigation::of(ctx),
      props.task.project.open.project.id(),
      props.task.project.feed,
    );
    ctx.watch(&share.state(), move |state| {
      if let Some(SharedAs(key)) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        // The task is the server's now, under its new key, which the project's tasks read again have.
        feed.saved();
        navigation.replace(Route::Project(
          project,
          ProjectPage::Task(key.clone(), TaskView::of(TaskTab::Summary)),
        ));
      }
    });
    Self {
      share,
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let local = props.task.detail.task.key().to_string();
    let (project, key, server) = props.linked.as_ref().map_or_else(
      || (String::new(), String::new(), String::new()),
      |linked| {
        (
          props.task.project.open.project.name().as_str().to_owned(),
          linked.sharing.link.key.as_str().to_owned(),
          linked
            .sharing
            .server
            .as_ref()
            .map(|(name, _)| name.clone())
            .unwrap_or_default(),
        )
      },
    );
    let next = props.next.as_ref().map(ToString::to_string);
    let lines = lines(ctx, &props, (&local, (&project, &key, &server)), next.as_deref());
    let state = self.share.state().get();
    let failure = state.error.clone().filter(|_| !state.is_pending());
    Column::new()
      .id("task-share-question")
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Inset.role())
      .spacing(SpacingSize::Sm)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .rounded(ExtraRadius::Tile.role())
      .child(
        Text::new(&ctx.t_args(
          "task.share.title",
          [("task", local.as_str()), ("project", key.as_str())],
        ))
        .id("task-share-title")
        .variant(ExtraTypography::FormSection.role()),
      )
      .with_children(lines)
      .child(self.buttons(ctx, next.as_deref(), state.is_pending()))
      .with_children(self.person.reason(ctx, SHARE))
      .with_children(failure.map(|reason| field_error("task-share-failed", &reason.text(ctx))))
  }
}

impl ShareQuestion {
  /// *Share as KTR-45* (*Share task* while the next key isn't known) and *Keep it here*.
  fn buttons(&self, ctx: &mut Ctx, next: Option<&str>, pending: bool) -> Row {
    let label = match next {
      Some(next) => ctx.t_args("task.share.confirm", [("key", next)]),
      None => ctx.t("task.share.confirmNext"),
    };
    let share = ActionButton::new(SHARE, ButtonKind::Primary, label).icon(Icon::Upload);
    let share = if pending {
      share.loading()
    } else {
      let (action, person) = (self.share.clone(), self.person.clone());
      share.on_click(move || {
        if person.allows(SHARE) {
          action.run(());
        }
      })
    };
    let asking = PageState::of(ctx).share;
    let keep = ActionButton::new("task-share-keep", ButtonKind::Secondary, ctx.t("task.share.keep"));
    let keep = if pending {
      keep.disabled()
    } else {
      keep.on_click(move || asking.set(false))
    };
    Row::new()
      .padding(lurq::node::padding::Padding::new().top(SpacingSize::Xs))
      .spacing(SpacingSize::Sm)
      .child(share)
      .child(keep)
  }
}

/// What the share does: where the task goes and as which key, the server's similar task, its tags and links, and what
/// stays here.
fn lines(
  ctx: &Ctx,
  props: &Props,
  (local, (project, key, server)): (&str, (&str, &str, &str)),
  next: Option<&str>,
) -> Vec<Element> {
  let goes = match next {
    Some(next) => ctx.t_args(
      "task.share.goes",
      [("project", project), ("server", server), ("next", next)],
    ),
    None => ctx.t_args(
      "task.share.goesNext",
      [("project", project), ("server", server), ("key", key)],
    ),
  };
  let mut lines = vec![line("task-share-goes", Icon::Upload, &goes)];
  if let Some(similar) = props
    .linked
    .as_ref()
    .and_then(|linked| linked.kept.as_ref()?.similar.as_ref())
  {
    let words = ctx.t_args(
      "task.share.similar",
      [
        ("project", key),
        ("key", similar.key.as_str()),
        ("title", similar.title.as_str()),
        ("by", similar.created_by.as_str()),
        ("task", local),
      ],
    );
    lines.push(line("task-share-similar", Icon::Copy, &words));
  }
  lines.push(line(
    "task-share-tags",
    Icon::Tag,
    &ctx.t_args("task.share.tags", [("project", key)]),
  ));
  lines.push(line("task-share-stays", Icon::Archive, &ctx.t("task.share.stays")));
  lines
}

/// One thing the share does, after its icon.
fn line(id: &str, glyph: Icon, words: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextSecondary))
    .child(Text::new(words).variant(ExtraTypography::Prose.role()).flex(1.0))
    .into()
}
