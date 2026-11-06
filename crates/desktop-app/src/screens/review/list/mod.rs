//! Review's list (`01 · Review` `i6S42`, `03` `QpQTI`, `04` `M1LAx` of `Review - v2` `j1Drb`): the tasks whose run
//! finished, in three groups under the head (the title, the search; `/` reaches it) and a lead line. *Waiting for you*,
//! *With the agent* and *Approved today* each fold under their header; a row, and the Review button a hovered row
//! shows in place of its counts, open the task's review at `/project/<id>/review/<KEY>`. The search narrows the rows by
//! key and title.
//!
//! Reading never makes the page jump (rule 9 of `b6M3lS`): the head shows at once; *Waiting for you* takes the room of
//! as many bones as the sidebar's count says (`waiting_count`, which the group's rows always match), and with none it
//! is drawn at once as a group of 0 with its one row-high line. *With the agent* and *Approved today* are not drawn
//! until read, and then come under what is there. Opened again, the list last read, kept in the project's feed, shows
//! while it is read anew; *Approved today* then starts folded, read already.

mod bones;
mod head;
mod items;
mod line;
mod read;
mod row;
mod words;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
    theme::SpacingSize,
  },
  components::{Column, VirtualizedList},
  core::{ElementRef, Signal},
  layout::layout_kind::ScrollState,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tasks::review_list::ReviewList;

use self::{
  items::{Folded, Folding},
  line::{ReviewLine, height, key_of},
  read::Read,
};
use crate::{
  project::ShownProject,
  theme::{TasksSpacing, list_scrollbar},
  ui::{message::message_text, skeleton::FromFirstFrame, state::load_error},
};

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
}

impl DevtoolsInspectable for Props {}

/// Review's list of `project`.
pub fn screen(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<ReviewScreen>(
    &format!("review-{}", project.open.project.id()),
    Props {
      project: project.clone(),
    },
  )
}

struct ReviewScreen {
  search: Signal<String>,
  search_input: ElementRef,
  /// `/` asked for the search: the next render focuses it.
  focus_search: Signal<bool>,
  folded: Signal<Folded>,
  /// Counts Try again after a failed read.
  again: Signal<u64>,
  scroll: ScrollState,
}

impl Component for ReviewScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    // A list read before is read: its *Approved today* stays folded until the person opens it.
    let read_before = ctx.props::<Props>().project.feed.review.with_untracked(Option::is_some);
    Self {
      search: ctx.signal(String::new()),
      search_input: ElementRef::default(),
      focus_search: ctx.signal(false),
      folded: ctx.signal(Folded::opened(read_before)),
      again: ctx.signal(0),
      scroll: ScrollState::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    ctx.provide(FromFirstFrame);
    ctx.provide(Folding(self.folded.clone()));
    let Props { project } = ctx.props::<Props>().clone();
    if self.focus_search.get() {
      ctx.focus(&self.search_input);
      self.focus_search.set(false);
    }
    // The page keeps 28 at its left; the body keeps the 28 at its right itself, so the list can run to the panel's
    // edge with its scrollbar there.
    let page = Column::new()
      .id("screen-review")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Xl).left(TasksSpacing::Sides.role()))
      .spacing(TasksSpacing::Stack.role())
      .on_key_down(slash(self.focus_search.clone()))
      .child(inset(head::head(ctx, &self.search, &self.search_input)))
      .child(inset(head::lead(ctx)));
    page.child(self.body(ctx, &project))
  }
}

impl ReviewScreen {
  fn body(&self, ctx: &mut Ctx, project: &ShownProject) -> Element {
    match read::read(ctx, project, self.again.get()) {
      Read::Unknown => bones::reading(ctx, None),
      // Nothing waits: `03` at once, never after bones.
      Read::Reading { waiting: 0 } => self.list(ctx, project, &ReviewList::default()),
      Read::Reading { waiting } => bones::reading(ctx, Some(waiting)),
      Read::TasksFailed(reason) => {
        let feed = project.feed.clone();
        let body = ctx.t_args("review.readFailedBody", [("reason", message_text(ctx, &reason))]);
        inset(load_error(
          ctx,
          "review-load-error",
          (&ctx.t("review.readFailed"), &body),
          // Reading the tasks again is what a save asks for too.
          move || feed.saved(),
        ))
        .into()
      }
      Read::Failed(reason) => {
        let again = self.again.clone();
        let body = ctx.t_args("review.readFailedBody", [("reason", message_text(ctx, &reason))]);
        inset(load_error(
          ctx,
          "review-load-error",
          (&ctx.t("review.readFailed"), &body),
          move || again.update(|count| *count += 1),
        ))
        .into()
      }
      Read::Ready(list) => self.list(ctx, project, &list),
    }
  }

  /// The list as read: the groups' header lines and rows, windowed, the lines 2 apart.
  fn list(&self, ctx: &mut Ctx, project: &ShownProject, list: &ReviewList) -> Element {
    let lines = items::lines(
      ctx,
      project.open.project.id(),
      list,
      (&self.search.get(), &self.folded.get()),
    );
    let total: f32 = lines.iter().map(height).sum();
    let scrollbar = list_scrollbar(&ctx.theme().scrollbar());
    let list = VirtualizedList::new_shared(ctx, std::sync::Arc::new(lines))
      .list_key(format!("review-{}", self.search.get()))
      .with_scroll_state(self.scroll.clone())
      .scrollbar(scrollbar)
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .max_height(Dimension::Px(total))
      .mount_keyed::<ReviewLine, _, _, _>(key_of, Clone::clone);
    Column::new()
      .id("review-list")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .max_height(Dimension::Px(total))
      .child(list)
      .into()
  }
}

/// `part` 28 from the panel's right edge, where the page leaves that room to the list's scrollbar.
fn inset(part: impl Into<Element>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().right(TasksSpacing::Sides.role()))
    .child(part)
}

/// `/` asks for the search, unless a text field has the keys or a modifier is held.
fn slash(focus: Signal<bool>) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  move |event: KeyboardEvent| {
    if event.default_prevented() || event.text_input_focused || event.ctrl || event.alt || event.meta {
      return;
    }
    if event.key == "/" {
      event.prevent_default();
      focus.set(true);
    }
  }
}
