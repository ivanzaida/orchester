//! *Join ORC on Kontur team* (10.21 `UlHPB`, a page under Project settings › Sharing; failure
//! 10.26 D): the project's local tasks, all ticked to start, with the server's similar titles as a
//! hint. Ticked tasks are uploaded with new numbers; a task with a run that has not ended stays
//! ticked and goes up claimed. When joining fails nothing changed here and the ticks are kept;
//! *Try again* does not upload twice what reached the server before.

mod rows;

use std::collections::BTreeSet;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, ScrollVertical, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::{IdempotencyKey, ServerId, TaskId};

use super::{ProjectNav, page};
use crate::{
  background,
  messages::Message,
  navigation::route::{Page, SettingsTab},
  programs::Programs,
  sharing::{
    ServerEntry,
    feed::{JustJoined, ServerFeed},
    project::{JoinFailed, JoinReview, Joined, repository_identity},
  },
  theme::ExtraTypography,
  ui::{button::ActionButton, icon::Icon, message_text, notice::Notice, panel::RecoveryPanel},
  workspace::OpenProject,
};

/// The width of the page's description (`R52wl`).
const DESCRIPTION_WIDTH: f32 = 760.0;

#[derive(Clone, PartialEq)]
pub struct JoinProps {
  pub open: OpenProject,
  pub server: ServerId,
}

impl lurq::app::component::DevtoolsInspectable for JoinProps {}

/// The tasks the person unticked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Unticked(BTreeSet<TaskId>);

impl lurq::app::component::DevtoolsInspectable for Unticked {}

/// The key the join is sent under for this visit of the page: *Try again* sends it again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct JoinKey(IdempotencyKey);

impl lurq::app::component::DevtoolsInspectable for JoinKey {}

/// Why the join failed, as the page says it (10.26 D).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Failure {
  reason: Message,
  /// Whether the server holds tasks of the project already; `None` when the connection was lost
  /// while they were sent.
  uploaded: Option<bool>,
}

impl lurq::app::component::DevtoolsInspectable for Failure {}

type JoinAction = FutureAction<(ServerEntry, JoinReview, Vec<TaskId>, IdempotencyKey), Joined, Failure>;

pub struct JoinPage {
  unticked: Signal<Unticked>,
  key: Signal<JoinKey>,
  /// Bumped by *Back to the review*: the failure notice goes until the next attempt.
  dismissed: Signal<bool>,
  nav: ProjectNav,
}

impl Component for JoinPage {
  type Props = JoinProps;

  fn create(ctx: &mut Ctx) -> Self {
    let page = Self {
      unticked: ctx.signal(Unticked::default()),
      key: ctx.signal(JoinKey(IdempotencyKey::generate())),
      dismissed: ctx.signal(false),
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    };
    let action = join_action(ctx);
    page.watch(ctx, &action);
    page
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<JoinProps>().clone();
    let action = join_action(ctx);
    let entry = ServerFeed::of(ctx).list.get().entry(props.server).cloned();
    let review = entry.as_ref().map(|_| self.review(ctx, &props.open, props.server));
    let project = props.open.project.name().as_str();
    let (key, count) = match &review {
      Some(Ok(Some(review))) => (review.key.as_str().to_owned(), Some(review.tasks.len())),
      _ => (String::new(), None),
    };
    let server = entry
      .as_ref()
      .map(|entry| entry.stored.name.clone())
      .unwrap_or_default();
    let breadcrumb = ctx.t_args("sharing.join.breadcrumb", [("project", project), ("key", key.as_str())]);
    let trailing = self.topbar_actions(ctx, (&review, entry.as_ref()), &action);
    let mut body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_horizontal(SpacingSize::Section)
      .padding_bottom(SpacingSize::Section)
      .spacing(SpacingSize::Lg);
    match review {
      None => {
        body = body.child(RecoveryPanel::new(
          Icon::AlertCircle,
          &ctx.t("sharing.join.loadFailed"),
          &ctx.t("sharing.errors.serverMissing"),
        ));
      }
      Some(Err(error)) => {
        body = body.child(RecoveryPanel::new(
          Icon::AlertCircle,
          &ctx.t("sharing.join.loadFailed"),
          &message_text(ctx, &error),
        ));
      }
      Some(Ok(None)) => {
        body = body.child(Text::new(&ctx.t("sharing.join.loading")).variant(ExtraTypography::Detail.role()));
      }
      Some(Ok(Some(review))) => {
        let failure = entry
          .as_ref()
          .and_then(|entry| self.failure(ctx, (&review, entry), &props.open, &action));
        body = body
          .with_children(failure)
          .child(self.review_bar(ctx, &review))
          .child(self.table(ctx, &review))
          .child(rows::note(ctx, &review, &self.ticked(&review)));
      }
    }
    Column::new()
      .id("join-page")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(page::topbar(&breadcrumb, trailing))
      .child(
        ScrollVertical::new(
          Column::new()
            .width(Dimension::Pct(100.0))
            .child(heading(ctx, (&key, &server), count))
            .child(body),
        )
        .width(Dimension::Pct(100.0))
        .flex(1.0),
      )
  }
}

impl JoinPage {
  /// A join that finished shows the project's Sharing tab with what changed; one that failed
  /// shows why here, until the next attempt.
  fn watch(&self, ctx: &mut Ctx, action: &JoinAction) {
    let navigation = ctx
      .use_context::<crate::navigation::Navigation>()
      .expect("the root provides Navigation before mounting screens");
    let (nav, joined, dismissed) = (self.nav.clone(), ServerFeed::of(ctx).joined, self.dismissed.clone());
    let root = ctx.props::<JoinProps>().open.repo_root.clone();
    ctx.watch(&action.state(), move |state| {
      if state.is_pending() {
        dismissed.set(false);
        return;
      }
      if let (true, Some(done)) = (state.is_fulfilled(), state.data.clone()) {
        navigation.places().add_project(done.open);
        joined.set(JustJoined(Some((root.clone(), done.dropped))));
        nav.tasks_changed();
        nav.replace(Page::Settings(SettingsTab::Sharing));
      }
    });
  }

  /// The review of the project's tasks on `entry`'s server; `Ok(None)` while it is read.
  fn review(&self, ctx: &mut Ctx, open: &OpenProject, server: ServerId) -> Result<Option<JoinReview>, Message> {
    let programs = ctx
      .use_context::<Programs>()
      .expect("the root provides Programs before mounting screens");
    let servers = ServerFeed::of(ctx).servers;
    // By the server's id: its entry changes with every connection check, which must not ask again.
    let deps = (open.repo_root.clone(), server);
    let load = ctx.future(deps, move |(root, server): (std::path::PathBuf, ServerId)| {
      let (programs, servers) = (programs.clone(), servers.clone());
      async move {
        let read = (root.clone(), servers.clone());
        let (identity, entry) = background::run(move || {
          let identity = repository_identity(&programs, &read.0);
          let entry = read
            .1
            .entries()
            .map(|entries| entries.into_iter().find(|entry| entry.stored.id == server));
          (identity, entry)
        })
        .await?;
        let identity = identity.map_err(|_| Message::new("sharing.join.noIdentity"))?;
        let entry = entry
          .map_err(|_| Message::new("sharing.errors.unavailable"))?
          .ok_or_else(|| Message::new("sharing.errors.serverMissing"))?;
        servers
          .join_review(&entry, &root, identity)
          .await
          .map_err(|error| error.message(&entry.address))
      }
    });
    let state = load.state().get();
    let pending = state.is_pending();
    match (state.data, state.error) {
      (_, Some(error)) if !pending => Err(error),
      (data, _) => Ok(data),
    }
  }

  /// The tasks that go up: every task not unticked, and every task whose run has not ended.
  fn ticked(&self, review: &JoinReview) -> Vec<TaskId> {
    let unticked = self.unticked.get();
    review
      .tasks
      .iter()
      .filter(|row| row.locked() || !unticked.0.contains(&row.task.id()))
      .map(|row| row.task.id())
      .collect()
  }

  /// *Cancel* and *Join and upload N* (`CeOHq` row).
  fn topbar_actions(
    &self,
    ctx: &Ctx,
    (review, entry): (&Option<Result<Option<JoinReview>, Message>>, Option<&ServerEntry>),
    action: &JoinAction,
  ) -> Vec<Element> {
    let nav = self.nav.clone();
    let cancel = ActionButton::new(ctx.t("common.cancel"))
      .id("join-cancel")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
      .on_click(move || nav.show(Page::Settings(SettingsTab::Sharing)))
      .render(ctx);
    let ready = match (review, entry) {
      (Some(Ok(Some(review))), Some(entry)) => Some((review, entry)),
      _ => None,
    };
    let count = ready.map_or(0, |(review, _)| self.ticked(review).len());
    let label = match count {
      0 => ctx.t("sharing.join.confirmNone"),
      1 => ctx.t("sharing.join.confirm.one"),
      count => ctx.t_args("sharing.join.confirm.other", [("count", count.to_string().as_str())]),
    };
    let button = ActionButton::new(label)
      .primary()
      .icon(Icon::Users)
      .id("join-confirm")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT));
    let button = match ready {
      Some((review, entry)) if !action.state().get().is_pending() => {
        let (action, review, entry, ticked, key) = (
          action.clone(),
          review.clone(),
          entry.clone(),
          self.ticked(review),
          self.key.get().0,
        );
        button.on_click(move || action.run((entry.clone(), review.clone(), ticked.clone(), key)))
      }
      _ => button.disabled(),
    };
    vec![cancel, button.render(ctx)]
  }

  /// Why joining failed (10.26 D), with *Try again* and *Back to the review*.
  fn failure(
    &self,
    ctx: &Ctx,
    (review, entry): (&JoinReview, &ServerEntry),
    open: &OpenProject,
    action: &JoinAction,
  ) -> Option<Element> {
    let state = action.state().get();
    let shown = state.is_rejected() && !self.dismissed.get();
    let failure = state.error.filter(|_| shown)?;
    let key = review.key.as_str();
    let server = entry.stored.name.as_str();
    let after = match failure.uploaded {
      Some(true) => ctx.t_args("sharing.join.failedUploaded", [("server", server)]),
      None => ctx.t_args(
        "sharing.join.failedUnknown",
        [("project", open.project.name().as_str()), ("server", server)],
      ),
      Some(false) => ctx.t_args(
        "sharing.join.failedNothing",
        [
          ("project", open.project.name().as_str()),
          ("key", open.project.key().as_str()),
          ("server", server),
        ],
      ),
    };
    let retry = {
      let (action, review, entry) = (action.clone(), review.clone(), entry.clone());
      let (ticked, key) = (self.ticked(&review), self.key.get().0);
      move || action.run((entry.clone(), review.clone(), ticked.clone(), key))
    };
    let dismissed = self.dismissed.clone();
    Some(
      Notice::new(
        Icon::AlertCircle,
        ctx.t_args("sharing.join.failed", [("key", key)]),
        message_text(ctx, &failure.reason),
      )
      .emphasized()
      .id("join-failed")
      .below(Text::new(&after).variant(ExtraTypography::Detail.role()))
      .below(rows::failure_actions(ctx, retry, move || dismissed.set(true)))
      .into(),
    )
  }
}

/// *Join ORC on Kontur team* and what the page does (`IkZhh`, `R52wl`).
/// `count` is the number of local tasks, once the review is read; until then the title names the
/// server only.
fn heading(ctx: &Ctx, (key, server): (&str, &str), count: Option<usize>) -> Element {
  let (title, description) = match count {
    None => (ctx.t_args("sharing.join.titlePending", [("server", server)]), None),
    Some(1) => (
      ctx.t_args("sharing.join.title", [("key", key), ("server", server)]),
      Some(ctx.t_args("sharing.join.description.one", [("key", key)])),
    ),
    Some(count) => (
      ctx.t_args("sharing.join.title", [("key", key), ("server", server)]),
      Some(ctx.t_args(
        "sharing.join.description.other",
        [("key", key), ("count", count.to_string().as_str())],
      )),
    ),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(crate::theme::ExtraSpacing::Rail.role())
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(SpacingSize::Lg)
    .spacing(crate::theme::ExtraSpacing::Stack.role())
    .child(Text::new(&title).variant(ExtraTypography::PageTitle.role()))
    .with_children(description.map(|description| {
      Text::new(&description)
        .variant(ExtraTypography::Explanation.role())
        .width(Dimension::Px(DESCRIPTION_WIDTH))
    }))
    .into()
}

fn join_action(ctx: &mut Ctx) -> JoinAction {
  let servers = ServerFeed::of(ctx).servers;
  let root = ctx.props::<JoinProps>().open.repo_root.clone();
  ctx.future_action(
    move |(entry, review, ticked, key): (ServerEntry, JoinReview, Vec<TaskId>, IdempotencyKey)| {
      let (servers, root) = (servers.clone(), root.clone());
      async move {
        servers
          .join(&entry, &root, &review, (ticked, key))
          .await
          .map_err(|JoinFailed { uploaded, error }| Failure {
            reason: error.message(&entry.address),
            uploaded,
          })
      }
    },
  )
}
