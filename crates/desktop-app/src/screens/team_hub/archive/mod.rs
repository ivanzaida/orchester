//! Archive in a resource's panel (`…` in the head of `A23 · Archive · used by · confirmation` `Pnpbe`, local `L11`
//! `dS97b`; `docs/design/team-configuration.md`, decision 11): the head's … menu holds *Archive*, which opens a modal
//! over the page that says what names the resource (*Used by*) and the runs that will stop, read from the team server
//! (or, in a local project, from the project's database). Archiving is never refused for being used. *Archive and
//! stop N runs* asks the team server to archive the resource; the runs that pin it stop on every computer when the
//! server's event stream says so ([`orchester_workspace::sharing::sync`]), and each task names the resource and who
//! archived it (A22, [`crate::screens::task`]).
//!
//! Opening the confirmation is anyone's, automation's included: it changes nothing. Confirming it is a person's alone,
//! as every other change that stops work is ([`PersonOnly`]); only an admin of the team has *Archive*.

mod dialog;
mod words;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_api::UsedBy;
use orchester_core::id::ResourceId;
use orchester_workspace::{messages::Message, sharing::project::NotArchived, used_by::UsedByError};

use super::model::Entry;
use crate::{
  screens::{hub::page_head::MORE_MENU_WIDTH, settings::orchester::PersonOnly},
  services::Services,
  ui::{
    icon::Icon,
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
  },
};

/// The id the person-only check of the confirmation's *Archive* is made under.
pub(super) const CONFIRM: &str = "team-hub-archive-confirm";

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub root: PathBuf,
  pub resource: ResourceId,
  pub name: Arc<str>,
}
impl DevtoolsInspectable for Props {}

/// The … menu of a panel's head for `entry`, with the confirmation it opens; `None` for a person who may not archive.
pub(super) fn menu(ctx: &mut Ctx, root: &std::path::Path, entry: &Entry, may_change: bool) -> Option<Element> {
  may_change.then(|| {
    ctx.mount_keyed::<ArchiveMenu>(
      &format!("team-hub-archive-{}", entry.id),
      Props {
        root: root.to_path_buf(),
        resource: entry.id,
        name: Arc::from(entry.name.as_str()),
      },
    )
  })
}

/// What uses the resource, as the team server (or the project's database) answered.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Used(UsedBy);
impl DevtoolsInspectable for Used {}

/// Why what uses the resource is not known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ReadFailed {
  /// The team server could not be reached.
  pub not_reached: bool,
}
impl DevtoolsInspectable for ReadFailed {}

/// Why the resource was not archived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Failed {
  /// The team server refused it with its error `code`.
  Refused(String),
  Said(Message),
}
impl DevtoolsInspectable for Failed {}

struct ArchiveMenu {
  /// The … menu.
  menu: Signal<bool>,
  anchor: ElementRef,
  /// The confirmation.
  open: Signal<bool>,
  /// Read again, as *Try again* asks.
  retry: Signal<u64>,
  archive: FutureAction<(), (), Failed>,
  person: PersonOnly,
}

impl Component for ArchiveMenu {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    let archive = ctx.future_action(move |(): ()| {
      let (services, root, resource) = (services.clone(), props.root.clone(), props.resource);
      async move {
        services
          .servers
          .archive_resource(&root, resource)
          .await
          .map_err(|refused| match refused {
            NotArchived::Refused { code } => Failed::Refused(code),
            NotArchived::Failed(message) => Failed::Said(message),
          })
      }
    });
    let open = ctx.signal(false);
    // An archived resource closes the confirmation; a refusal stays in it, with what it says.
    let shut = open.clone();
    ctx.watch(&archive.state(), move |state| {
      if state.is_fulfilled() {
        shut.set(false);
      }
    });
    Self {
      menu: ctx.signal(false),
      anchor: ctx.element_ref(),
      open,
      retry: ctx.signal(0),
      archive,
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let item = MenuItem::new("team-hub-archive", ctx.t("team_archive.action"), {
      let (open, retry) = (self.open.clone(), self.retry.clone());
      move || {
        retry.update(|count| *count += 1);
        open.set(true);
      }
    })
    .icon(Icon::Archive)
    .render(ctx, &self.menu);
    let toggle = self.menu.clone();
    let button = IconButton::new("team-hub-more", Icon::Ellipsis, ctx.t("team_archive.more"))
      .small()
      .element(self.anchor.clone())
      .on_click(move || toggle.update(|open| *open = !*open))
      .render(ctx);
    let panel = menu_panel("team-hub-more-menu")
      .width(Dimension::Px(MORE_MENU_WIDTH))
      .child(item);
    let confirmation = self.confirmation(ctx, &props);
    Column::new()
      .child(button)
      .child(menu_at_end(self.anchor.clone(), self.menu.clone(), panel))
      .child(confirmation)
  }
}

impl ArchiveMenu {
  /// What uses the resource, read while the confirmation is open; a closed one reads nothing.
  fn read(&self, ctx: &mut Ctx, props: &Props) -> Option<Result<UsedBy, ReadFailed>> {
    let services = Services::of(ctx);
    let (open, retry) = (self.open.get(), self.retry.get());
    let state = ctx
      .future(
        (props.root.clone(), props.resource, open, retry),
        move |(root, resource, open, _)| {
          let services = services.clone();
          async move {
            if !open {
              return Ok(None);
            }
            services
              .servers
              .used_by(&root, resource)
              .await
              .map(|used| Some(Used(used)))
              .map_err(|error: UsedByError| ReadFailed {
                not_reached: error.is_not_reached(),
              })
          }
        },
      )
      .state()
      .get();
    match (state.data.as_ref().and_then(Option::as_ref), &state.error) {
      (Some(used), _) if !state.is_pending() => Some(Ok(used.0.clone())),
      (_, Some(failed)) if !state.is_pending() => Some(Err(*failed)),
      _ => None,
    }
  }

  fn confirmation(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let read = self.read(ctx, props);
    let archiving = self.archive.state().get();
    let (action, person) = (self.archive.clone(), self.person.clone());
    let confirm: dialog::Confirm = Arc::new(move || {
      if person.allows(CONFIRM) {
        action.run(());
      }
    });
    dialog::dialog(
      ctx,
      props,
      dialog::State {
        read,
        archiving: archiving.is_pending(),
        failed: archiving.error.clone().filter(|_| !archiving.is_pending()),
        person_refused: self.person.reason(ctx, CONFIRM),
      },
      (&self.open, &self.retry, confirm),
    )
  }
}
