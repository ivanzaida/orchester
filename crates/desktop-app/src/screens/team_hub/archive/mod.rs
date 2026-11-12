//! Archive in a resource's panel (`…` in the head of `A23 · Archive · used by · confirmation` `Pnpbe`;
//! `docs/design/team-configuration.md`, decision 11): the head's … menu holds *Archive*, which opens a modal over the
//! page that says what names the resource (*Used by*) and the runs that will stop, read from the team server. Archiving
//! is never refused for being used. *Archive and stop N runs* reads what uses it once more and, if the runs that stop
//! are not the ones the list showed, keeps the modal open with the new list; otherwise it asks the team server to
//! archive the resource. The runs that pin it stop on every computer when the server's event stream says so
//! ([`orchester_workspace::sharing::sync`]), and each task names the resource and who archived it (A22,
//! [`crate::screens::task`]).
//!
//! Opening the confirmation is anyone's, automation's included: it changes nothing. Confirming it is a person's alone,
//! as every other change that stops work is ([`PersonOnly`], and the workspace refuses automation again); only an
//! admin of the team has *Archive*.

pub(in crate::screens) mod dialog;
mod tasks;
pub(in crate::screens) mod words;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_api::{UsedBy, UsedByRun};
use orchester_core::{id::ResourceId, run::Answerer};
use orchester_workspace::{
  messages::Message, readiness::archive_blocks_profiles, sharing::project::NotArchived, used_by::UsedByError,
};

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
pub(in crate::screens) struct Props {
  pub root: PathBuf,
  pub resource: ResourceId,
  pub name: Arc<str>,
  /// Whether a profile that names the resource stays not ready once it is archived
  /// ([`orchester_workspace::readiness::archive_blocks_profiles`]).
  pub blocks_profiles: bool,
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
        blocks_profiles: archive_blocks_profiles(entry.kind.core()),
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
pub(in crate::screens) struct ReadFailed {
  /// The team server could not be reached.
  pub not_reached: bool,
}
impl DevtoolsInspectable for ReadFailed {}

/// Why the resource was not archived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens) enum Failed {
  /// The team server refused it with its error `code`.
  Refused(String),
  Said(Message),
  /// What uses it could not be read again before archiving, so nothing was sent.
  Unread(ReadFailed),
  /// The runs that stop are not the ones the list showed: the list is read again, and nothing was archived.
  RunsChanged,
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
  /// Archives it unless the runs it stops changed since the list showed them (`runs`).
  archive: FutureAction<Vec<UsedByRun>, (), Failed>,
  person: PersonOnly,
}

impl Component for ArchiveMenu {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    let retry = ctx.signal(0_u64);
    let again = retry.clone();
    let archive = ctx.future_action(move |shown: Vec<UsedByRun>| {
      let (services, root, resource) = (services.clone(), props.root.clone(), props.resource);
      let again = again.clone();
      async move {
        let used = services.servers.used_by(&root, resource).await.map_err(|error| {
          Failed::Unread(ReadFailed {
            not_reached: error.is_not_reached(),
          })
        })?;
        if used.runs != shown {
          again.update(|count| *count += 1);
          return Err(Failed::RunsChanged);
        }
        services
          .servers
          .archive_resource(&root, resource, Answerer::Person)
          .await
          .map_err(|refused| match refused {
            NotArchived::Refused { code } => Failed::Refused(code),
            NotArchived::Failed(message) => Failed::Said(message),
            NotArchived::PersonOnly => Failed::Said(NotArchived::person_only_message()),
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
      retry,
      archive,
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let item = MenuItem::new("team-hub-archive", ctx.t("team_archive.action"), {
      let (open, retry, archive) = (self.open.clone(), self.retry.clone(), self.archive.state());
      move || {
        retry.update(|count| *count += 1);
        // A refusal of an earlier confirmation is not this one's.
        archive.set(FutureState::idle());
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
    let shown = read
      .as_ref()
      .and_then(|read| read.as_ref().ok())
      .map(|used| used.runs.clone())
      .unwrap_or_default();
    let confirm: dialog::Confirm = Arc::new(move || {
      if person.allows(CONFIRM) {
        action.run(shown.clone());
      }
    });
    dialog::dialog(
      ctx,
      &dialog::DialogProps {
        name: props.name.clone(),
        blocks_profiles: props.blocks_profiles,
        local: false,
        versioned: true,
      },
      dialog::State {
        read,
        lines: Vec::new(),
        tasks: None,
        archiving: archiving.is_pending(),
        failed: archiving.error.clone().filter(|_| !archiving.is_pending()),
        stopped: None,
        person_refused: self.person.reason(ctx, CONFIRM),
      },
      (&self.open, &self.retry, confirm),
    )
  }
}
