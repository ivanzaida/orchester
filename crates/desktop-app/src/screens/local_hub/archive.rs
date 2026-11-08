//! Archive in a local resource's panel (`…` in the head of `Agents · Archive · used by · confirmation` L11): the head's
//! menu holds *Archive*, which opens a modal over the page that says what names the resource (*Used by*) and the runs
//! of the project that will stop, read from the project's database. Archiving is never refused for being used.
//! *Archive and stop N runs* reads what uses it once more and, if the runs that stop are not the ones the list showed,
//! keeps the modal open with the new list; otherwise it archives the resource and cancels those runs. It leaves the
//! lists and pickers; *Archived* lists it and restores it ([`super::archived`]).
//!
//! Opening the confirmation is anyone's, automation's included: it changes nothing. Confirming it is a person's alone,
//! as every other change that stops work is.

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
use orchester_core::{id::ProjectId, resource::ResourceKind, run::Answerer};
use orchester_workspace::{
  local_archive::{self, Archivable, NotArchived},
  readiness::archive_blocks_profiles,
  used_by::{self, UsedByError},
};

use crate::{
  screens::{
    hub::page_head::MORE_MENU_WIDTH,
    settings::orchester::PersonOnly,
    team_hub::archive::{
      Failed, ReadFailed,
      dialog::{self, Confirm, DialogProps, State},
    },
  },
  services::Services,
  ui::{
    icon::Icon,
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
  },
};

/// The id the person-only check of the confirmation's *Archive* is made under.
const CONFIRM: &str = "local-hub-archive-confirm";

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub root: PathBuf,
  pub project: ProjectId,
  pub what: Archivable,
  pub kind: ResourceKind,
  pub name: Arc<str>,
}
impl DevtoolsInspectable for Props {}

/// The … menu of a panel's head for the entry `props` names, with the confirmation it opens.
pub(super) fn menu(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("local-hub-archive-{:?}", props.what);
  ctx.mount_keyed::<ArchiveMenu>(&key, props)
}

/// What uses the resource, as the project's database answered.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Used(UsedBy);
impl DevtoolsInspectable for Used {}

struct ArchiveMenu {
  /// The … menu.
  menu: Signal<bool>,
  anchor: ElementRef,
  /// The confirmation.
  open: Signal<bool>,
  /// Read again, as *Try again* asks.
  retry: Signal<u64>,
  /// Archives it unless the runs it stops changed since the list showed them.
  archive: FutureAction<Vec<UsedByRun>, (), Failed>,
  person: PersonOnly,
}

/// What uses `what` in the project at `root`.
async fn read(root: PathBuf, what: Archivable, services: &Services) -> Result<UsedBy, UsedByError> {
  match what {
    Archivable::Resource(id) => services.servers.used_by(&root, id).await,
    Archivable::McpServer(server) => used_by::used_by_mcp_server(&root, server).await,
  }
}

impl Component for ArchiveMenu {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    let retry = ctx.signal(0_u64);
    let again = retry.clone();
    let archive = ctx.future_action(move |shown: Vec<UsedByRun>| {
      let (services, props, again) = (services.clone(), props.clone(), again.clone());
      async move {
        let used = read(props.root.clone(), props.what, &services).await.map_err(|error| {
          Failed::Unread(ReadFailed {
            not_reached: error.is_not_reached(),
          })
        })?;
        if used.runs != shown {
          again.update(|count| *count += 1);
          return Err(Failed::RunsChanged);
        }
        local_archive::archive(&services.runners, &props.root, props.what, Answerer::Person)
          .await
          .map_err(|refused| match refused {
            NotArchived::Failed(message) => Failed::Said(message),
            NotArchived::PersonOnly => Failed::Said(NotArchived::person_only_message()),
          })?;
        for change in props.what.changes(Some(props.kind)) {
          services.changes.record(props.project, *change);
        }
        Ok(())
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
    let item = MenuItem::new("local-hub-archive", ctx.t("local_hub.archive.action"), {
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
    let button = IconButton::new("local-hub-more", Icon::Ellipsis, ctx.t("local_hub.archive.more"))
      .small()
      .element(self.anchor.clone())
      .on_click(move || toggle.update(|open| *open = !*open))
      .render(ctx);
    let panel = menu_panel("local-hub-more-menu")
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
        (props.root.clone(), props.what, open, retry),
        move |(root, what, open, _)| {
          let services = services.clone();
          async move {
            if !open {
              return Ok(None);
            }
            read(root, what, &services)
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
    let confirm: Confirm = Arc::new(move || {
      if person.allows(CONFIRM) {
        action.run(shown.clone());
      }
    });
    dialog::dialog(
      ctx,
      &DialogProps {
        name: props.name.clone(),
        blocks_profiles: archive_blocks_profiles(props.kind),
        local: true,
      },
      State {
        read,
        archiving: archiving.is_pending(),
        failed: archiving.error.clone().filter(|_| !archiving.is_pending()),
        person_refused: self.person.reason(ctx, CONFIRM),
      },
      (&self.open, &self.retry, confirm),
    )
  }
}
