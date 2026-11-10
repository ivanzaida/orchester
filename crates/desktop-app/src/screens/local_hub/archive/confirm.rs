//! The confirmation of archiving a local resource (`Agents · Archive · used by · confirmation` L11): a modal over the
//! page that says what names the resource (*Used by*) and the runs of the project that will stop, read from the
//! project's database. *Archive and stop N runs* reads what uses it once more and, if the runs that stop are not the
//! ones the list showed, keeps the modal open with the new list; otherwise it archives the resource and cancels those
//! runs. It leaves the lists and pickers; *Archived* lists it and restores it ([`super::super::archived`]).
//!
//! A run that could not be stopped is said before the lists read the archive again: the entry leaves them and its panel
//! with the confirmation, so the same modal turns into the notice and *Close* ends it.
//!
//! Opening the confirmation is anyone's, automation's included: it changes nothing. Confirming it is a person's alone,
//! as every other change that stops work is.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_api::{UsedBy, UsedByRun};
use orchester_core::{id::ProjectId, resource::ResourceKind};
use orchester_workspace::{
  local_archive::{self, Archivable},
  messages::Message,
  readiness::archive_blocks_profiles,
  used_by::{self, RunLine, UsedByError},
};

use crate::{
  screens::{
    settings::orchester::PersonOnly,
    team_hub::archive::{
      Failed, ReadFailed,
      dialog::{self, Confirm, DialogProps, State},
    },
  },
  services::Services,
};

type Handler = Arc<dyn Fn() + Send + Sync>;

/// What the confirmation is about, and what follows it.
#[derive(Clone)]
pub(in crate::screens) struct Props {
  pub root: PathBuf,
  pub project: ProjectId,
  pub what: Archivable,
  pub kind: ResourceKind,
  pub name: Arc<str>,
  /// The confirmation was left without archiving.
  pub on_keep: Handler,
  /// It was archived; the changes are recorded already.
  pub on_archived: Handler,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.root, self.project, self.what, self.kind, &self.name)
      == (&other.root, other.project, other.what, other.kind, &other.name)
  }
}
impl DevtoolsInspectable for Props {}

/// The confirmation for the entry `props` names.
pub(in crate::screens) fn question(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("local-hub-archive-confirm-{:?}", props.what);
  ctx.mount_keyed::<Confirmation>(&key, props)
}

/// What uses the resource, as the project's database answered.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Used(UsedBy, Vec<RunLine>);
impl DevtoolsInspectable for Used {}

struct Confirmation {
  /// The modal; leaving it ends the question.
  open: Signal<bool>,
  /// Read again, as *Try again* asks.
  retry: Signal<u64>,
  /// Archives it unless the runs it stops changed since the list showed them; what it answers is what to tell the
  /// person when some of the runs could not be stopped.
  archive: FutureAction<Vec<UsedByRun>, Option<Message>, Failed>,
  person: PersonOnly,
}

/// What uses `what` in the project at `root`.
async fn read(root: PathBuf, what: Archivable, services: &Services) -> Result<UsedBy, UsedByError> {
  match what {
    Archivable::Resource(id) => services.servers.used_by(&root, id).await,
    Archivable::McpServer(server) => used_by::used_by_mcp_server(&root, server).await,
  }
}

impl Component for Confirmation {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    let retry = ctx.signal(0_u64);
    let again = retry.clone();
    let (archived, keep) = (props.on_archived.clone(), props.on_keep.clone());
    let owned = props.clone();
    let archive = ctx.future_action(move |shown: Vec<UsedByRun>| {
      let (services, props, again) = (services.clone(), owned.clone(), again.clone());
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
        let archived = local_archive::archive(&services.runners, &props.root, props.what)
          .await
          .map_err(Failed::Said)?;
        Ok(archived.left_running_message())
      }
    });
    let open = ctx.signal(true);
    let (changes, props) = (Services::of(ctx).changes, props.clone());
    let finish = Arc::new(move || {
      for change in props.what.changes(Some(props.kind)) {
        changes.record(props.project, *change);
      }
      archived();
    });
    // An archived resource ends the question unless a run could not be stopped: that is said first, and the lists read
    // the archive when the person closes it. A refusal stays in the question, with what it says.
    let done = finish.clone();
    ctx.watch(&archive.state(), move |state| {
      if state.is_fulfilled() && matches!(state.data, Some(None)) {
        done();
      }
    });
    let said = archive.state();
    ctx.watch(&open, move |open| {
      if *open {
        return;
      }
      if matches!(said.get_untracked().data, Some(Some(_))) {
        finish();
      } else {
        keep();
      }
    });
    Self {
      open,
      retry,
      archive,
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.confirmation(ctx, &props)
  }
}

impl Confirmation {
  /// What uses the resource.
  fn read(&self, ctx: &mut Ctx, props: &Props) -> Option<Result<(UsedBy, Vec<RunLine>), ReadFailed>> {
    let services = Services::of(ctx);
    let state = ctx
      .future(
        (props.root.clone(), props.what, self.retry.get()),
        move |(root, what, _)| {
          let services = services.clone();
          async move {
            let used = read(root.clone(), what, &services)
              .await
              .map_err(|error: UsedByError| ReadFailed {
                not_reached: error.is_not_reached(),
              })?;
            let runs = used.runs.iter().map(|run| run.run).collect();
            let lines = used_by::run_lines(&root, runs)
              .await
              .map_err(|error: UsedByError| ReadFailed {
                not_reached: error.is_not_reached(),
              })?;
            Ok(Used(used, lines))
          }
        },
      )
      .state()
      .get();
    match (&state.data, &state.error) {
      (Some(used), _) if !state.is_pending() => Some(Ok((used.0.clone(), used.1.clone()))),
      (_, Some(failed)) if !state.is_pending() => Some(Err(*failed)),
      _ => None,
    }
  }

  fn confirmation(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let (read, lines) = match self.read(ctx, props) {
      Some(Ok((used, lines))) => (Some(Ok(used)), lines),
      Some(Err(failed)) => (Some(Err(failed)), Vec::new()),
      None => (None, Vec::new()),
    };
    let archiving = self.archive.state().get();
    let (action, person) = (self.archive.clone(), self.person.clone());
    let shown = read
      .as_ref()
      .and_then(|read| read.as_ref().ok())
      .map(|used| used.runs.clone())
      .unwrap_or_default();
    let confirm: Confirm = Arc::new(move || {
      if person.allows("archive a resource and stop the runs that use it") {
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
      &State {
        read,
        lines,
        archiving: archiving.is_pending(),
        failed: archiving.error.clone().filter(|_| !archiving.is_pending()),
        stopped: archiving.data.clone().flatten().filter(|_| !archiving.is_pending()),
      },
      (&self.open, &self.retry, confirm),
    )
  }
}
