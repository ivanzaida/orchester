//! The top of the Tasks list, one 32 high row (`Header` `AL7qv` in `Tasks · redesign` `Lw1Gb`): the title in 20/600,
//! then at the end the search (240 wide, its `/` key cap while empty; `/` reaches it), Filter and Display (ghost
//! buttons), *More actions* (`…`: holding new starts and Create multiple, `queue`) and New task, 8 apart. Until the
//! tasks are read, Filter, Display and `…` cannot be pressed. A project without tasks shows the title alone (`03 No
//! tasks yet` `f54MO`). A linked project's list says where it lives under the row (`Shared line` in `Xr4Wb`); once the
//! person is no longer a member, New task is locked and says why there.
//!
//! The search is part of the list's path (`?q=`), taking the current entry's place as it is typed, so Back leaves the
//! list rather than replaying the typing; the filters and the order belong to the open list.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::dimension::Dimension,
};
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::browse::{Facets, TaskSort};

use super::{
  display::{DisplayControl, DisplayProps},
  facets::{FilterControl, FilterProps, Options},
  queue::{QueueControls, QueuePart, QueueProps},
  shared::{self, Shared},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::Icon,
    icon_button::IconButton,
    unavailable::reason_at_end,
  },
};

/// The header's row (`AL7qv`) and its search (`Search` `c8xTL`).
const ROW_HEIGHT: f32 = 32.0;
const SEARCH_WIDTH: f32 = 240.0;

/// What the person chose to narrow and order the list, and the search's input, which `/` focuses.
pub(super) struct Choices {
  pub search: Signal<String>,
  pub facets: Signal<Facets>,
  pub sort: Signal<TaskSort>,
  pub search_input: ElementRef,
}

/// What the header is drawn from.
pub(super) struct Header<'a> {
  pub project: ProjectId,
  pub choices: &'a Choices,
  /// The facets the Filter offers, once the tasks are read.
  pub options: Option<Arc<Options>>,
  /// The holds on new starts, once the tasks are read.
  pub queue: Option<QueueProps>,
  pub shared: Option<&'a Shared>,
}

/// The header row, with why New task is locked and a linked project's line under it.
pub(super) fn header(ctx: &mut Ctx, header: Header<'_>) -> Column {
  let Header {
    project,
    choices,
    options,
    queue,
    shared,
  } = header;
  let read_only = shared.and_then(|shared| shared.read_only(ctx));
  let row = title_row(ctx)
    .child(Spacer::new().flex(1.0))
    .child(search(ctx, choices))
    .with_children(controls(ctx, (project, choices), (options, queue), read_only.clone()));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(row)
    .with_children(read_only.map(|reason| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .child(Spacer::new().flex(1.0))
        .child(reason_at_end("tasks-new-task-reason", &reason))
    }))
    .with_children(shared.map(|shared| shared::line(ctx, project, shared)))
}

/// The search, as the list's path holds it.
fn search(ctx: &mut Ctx, choices: &Choices) -> lurq::node::Element {
  FieldInput::new("tasks-search", ctx.t("tasks.search.label"), choices.search.clone())
    .search()
    .keys(ctx.t("tasks.search.keys"))
    .clearable(ctx.t("tasks.search.clear"))
    .placeholder(ctx.t("tasks.search.placeholder"))
    .width(Dimension::Px(SEARCH_WIDTH))
    .element(choices.search_input.clone())
    .render(ctx)
}

/// Filter, Display, `…` and New task, locked for `read_only`'s reason; Create multiple says the same reason.
fn controls(
  ctx: &mut Ctx,
  (project, choices): (ProjectId, &Choices),
  (options, queue): (Option<Arc<Options>>, Option<QueueProps>),
  read_only: Option<Arc<str>>,
) -> Vec<lurq::node::Element> {
  let ready = options.is_some();
  let filter = ctx.mount_keyed::<FilterControl>(
    &format!("tasks-filter-{project}"),
    FilterProps {
      facets: choices.facets.clone(),
      options,
    },
  );
  let display = ctx.mount_keyed::<DisplayControl>(
    &format!("tasks-display-{project}"),
    DisplayProps {
      sort: choices.sort.clone(),
      ready,
    },
  );
  let reason = read_only.clone().unwrap_or_else(|| ctx.t("tasks.createMultipleReason"));
  let more = match queue {
    Some(queue) => ctx.mount_keyed::<QueueControls>(
      &format!("tasks-queue-menu-{project}"),
      QueueProps {
        part: QueuePart::Menu { reason },
        ..queue
      },
    ),
    None => IconButton::new("tasks-more", Icon::Ellipsis, ctx.t("tasks.more")).render(ctx),
  };
  let navigation = Navigation::of(ctx);
  let new = new_task(ctx, "tasks-new-task", project, &navigation);
  let new = match read_only {
    Some(reason) => new.unavailable(reason),
    None => new,
  };
  vec![filter, display, more, new.into()]
}

/// The header of a project without tasks: its title alone.
pub(super) fn title_only(ctx: &Ctx) -> Row {
  title_row(ctx)
}

/// The row with the title at its start.
fn title_row(ctx: &Ctx) -> Row {
  Row::new()
    .id("tasks-header")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("tasks.title"))
        .id("tasks-title")
        .variant(TypographyStyle::Heading)
        .nowrap(),
    )
}

/// New task, which opens the New task form.
pub(super) fn new_task(ctx: &Ctx, id: &str, project: ProjectId, navigation: &Navigation) -> ActionButton {
  ActionButton::new(id.to_owned(), ButtonKind::Primary, ctx.t("tasks.newTask"))
    .icon(Icon::Plus)
    .opens(navigation.to(Route::Project(project, ProjectPage::NewTask)))
}

/// Puts `text` in the active tab's path as the Tasks list's search, in place of the current entry; a path that already
/// searches for it stays as it is.
pub(super) fn follow_search(navigation: &Navigation, text: &str) {
  let Route::Project(project, ProjectPage::Tasks(list)) = navigation.route_now() else {
    return;
  };
  if list.search().unwrap_or_default() != text {
    navigation.replace(Route::Project(project, ProjectPage::Tasks(list.with_search(text))));
  }
}
