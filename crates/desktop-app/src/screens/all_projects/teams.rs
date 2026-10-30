//! What All projects reads of the person's teams (`docs/design/startup.md`, decision 8; 09 `i5G19A`, 09b `dLOCJ`):
//! where each listed project is, the team servers joined, and the one team project with no checkout here that it
//! offers above the list until *Not now* puts it off. A read that fails is logged and shows nothing: a dash in the
//! *Where* column, no offer.

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
};
use orchester_core::id::{ProjectId, ServerId};
use orchester_workspace::{
  background,
  sharing::{OfferedProject, Places, Servers, SharingError},
  workspace::ProjectEntry,
};
use parking_lot::Mutex;

/// Where each project is and the servers joined, as last read.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Joined {
  pub places: Places,
  /// The signed-in account's servers in use, in the order they were added.
  pub servers: Vec<ServerId>,
}

impl DevtoolsInspectable for Joined {}

/// The projects put off on this screen, hidden at once while *Not now* is kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PutOff(Vec<(ServerId, ProjectId)>);

impl DevtoolsInspectable for PutOff {}

/// The project *Not now* puts off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NotNow {
  server: ServerId,
  project: ProjectId,
}

impl DevtoolsInspectable for NotNow {}

/// What the strip does: *Not now* for the project it offers.
#[derive(Clone)]
pub(super) struct Answer {
  put_off: Signal<PutOff>,
  action: FutureAction<NotNow, (), ()>,
}

impl Answer {
  /// *Not now* for `offer`: it goes at once, and is kept so it is not offered again.
  pub fn not_now(&self, offer: &OfferedProject) {
    let put = NotNow {
      server: offer.server,
      project: offer.project.id,
    };
    self.put_off.update(|put_off| put_off.0.push((put.server, put.project)));
    self.action.run(put);
  }
}

/// The screen's state of the teams' reads: what was put off, and how often *Not now* was kept.
pub(super) struct Teams {
  put_off: Signal<PutOff>,
  /// Counts the answers kept, so the next project is offered after one.
  kept: Signal<u64>,
  /// Whether the answers' ends are followed already; once per screen.
  followed: Mutex<bool>,
}

/// What [`Teams::read`] found.
pub(super) struct Read {
  /// `None` until it is read.
  pub joined: Option<Joined>,
  /// The project offered, unless it was put off on this screen.
  pub offer: Option<OfferedProject>,
  pub answer: Answer,
}

impl Teams {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      put_off: ctx.signal(PutOff::default()),
      kept: ctx.signal(0),
      followed: Mutex::new(false),
    }
  }

  /// The teams' reads for `entries`, held while they are `None`: until the list is read, and on the welcome. The
  /// screen asks for them in every render, in the same place: lurq keeps a component's futures by the order it asks
  /// for them.
  pub fn read(&self, ctx: &mut Ctx, servers: &Servers, entries: Option<&[ProjectEntry]>) -> Read {
    let entries = entries.map(<[ProjectEntry]>::to_vec);
    let joined = joined(ctx, servers, entries.clone());
    let offer = offer(ctx, servers, (entries, self.kept.get()));
    let action = not_now_action(ctx, servers);
    self.follow(ctx, &action);
    let put_off = self.put_off.get();
    Read {
      joined,
      offer: offer.filter(|offer| !put_off.0.contains(&(offer.server, offer.project.id))),
      answer: Answer {
        put_off: self.put_off.clone(),
        action,
      },
    }
  }

  /// A kept *Not now* reads the offer again, for the next project. One that was not kept is logged; its project stays
  /// hidden on this screen and comes back at the next.
  fn follow(&self, ctx: &mut Ctx, action: &FutureAction<NotNow, (), ()>) {
    let mut followed = self.followed.lock();
    if *followed {
      return;
    }
    *followed = true;
    let kept = self.kept.clone();
    ctx.watch(&action.state(), move |state| {
      if state.is_fulfilled() && !state.is_pending() {
        kept.update(|kept| *kept += 1);
      }
    });
  }
}

/// Where each of `entries` is and the servers joined, read off the UI thread whenever the list changes; nothing
/// while the entries are held.
fn joined(ctx: &mut Ctx, servers: &Servers, entries: Option<Vec<ProjectEntry>>) -> Option<Joined> {
  ctx
    .future((servers.clone(), entries), |(servers, entries)| async move {
      let Some(entries) = entries else {
        return Ok(None);
      };
      let read = background::run(move || -> Result<Joined, SharingError> {
        Ok(Joined {
          places: servers.places(&entries)?,
          servers: servers
            .entries()?
            .iter()
            .filter(|entry| entry.in_use())
            .map(|entry| entry.stored.id)
            .collect(),
        })
      })
      .await;
      match read {
        Ok(Ok(joined)) => Ok(Some(joined)),
        Ok(Err(error)) => {
          tracing::warn!(%error, "cannot read where the listed projects are");
          Err(())
        }
        // The work did not finish; `background::run` logged why.
        Err(_) => Err(()),
      }
    })
    .state()
    .get()
    .data
    .flatten()
}

/// The project offered, read when the list changes and after a kept *Not now*; nothing while the entries are held.
/// Not when a server's connection changes: reading the offer asks the servers, and each answer changes their
/// connection.
fn offer(
  ctx: &mut Ctx,
  servers: &Servers,
  (entries, kept): (Option<Vec<ProjectEntry>>, u64),
) -> Option<OfferedProject> {
  ctx
    .future((servers.clone(), entries, kept), |(servers, entries, _)| async move {
      if entries.is_none() {
        return Ok(None);
      }
      servers.offered().await.map_err(|error| {
        tracing::warn!(%error, "cannot read which team project to offer");
      })
    })
    .state()
    .get()
    .data
    .flatten()
}

fn not_now_action(ctx: &mut Ctx, servers: &Servers) -> FutureAction<NotNow, (), ()> {
  let servers = servers.clone();
  ctx.future_action(move |NotNow { server, project }| {
    let servers = servers.clone();
    async move {
      let kept = background::run(move || servers.not_now(server, project)).await;
      match kept {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
          tracing::warn!(%error, %server, %project, "cannot keep a team project put off");
          Err(())
        }
        // The work did not finish; `background::run` logged why.
        Err(_) => Err(()),
      }
    }
  })
}
