//! The panel of a team MCP server (`Agents · MCP servers` `yIsDi` A3; `C4` `SpRvd` no program path, `C5` `h2md5` path
//! set and file missing, `C6` `LiS1F` a secret without a value, `C7` `tV4LG` a remote server with a secret header, `C8`
//! `lSA4x` ready here in `Agents · MCP server states` `acFkz`; `docs/design/team-configuration.md`, "What a computer
//! fills in"): its head and chips, *From the team* (how it runs, the same on every computer), *On this computer* (the
//! program's path, the value of each secret it takes, whether it is on here, and how it stands) and, pinned at the
//! bottom, the fix of the first missing part.
//!
//! The path of the program is the person's choice from a file picker, kept for the whole computer (every project of the
//! team server), and nothing is guessed or found by name. Turning the server on, setting the path and a secret's value
//! widen what agents may do, so they are a person's alone: a press that automation made does nothing and says so.
//! Turning it off is always taken.

mod computer;
mod fix;
mod from_team;
mod words;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, ScrollVertical},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::{ProjectId, ResourceId};
use orchester_workspace::agents::team_mcp::TeamMcpSheet;

use self::fix::{Bar, Fix, Step};
use super::{
  actions::Actions,
  member,
  model::{Entry, Listing},
  panel, secret,
};
use crate::{
  screens::settings::orchester::PersonOnly,
  services::Services,
  theme::HubSpacing,
  ui::{field::field_error, message::message_text, pressable::Press},
};

#[derive(Clone)]
pub(super) struct Props {
  pub repo_root: PathBuf,
  pub project: ProjectId,
  /// The person may change the team's resources: an admin of the team.
  pub may_change: bool,
  /// Who changes the team's resources, which a member reads.
  pub admins: member::Admins,
  /// The hub is read again: what shows is the last answer, and a fix would act on it.
  pub reading: bool,
  /// Reads the hub again, as *Check again* asks.
  pub reread: Signal<u64>,
  pub listing: Listing,
  pub entry: Entry,
  pub sheet: TeamMcpSheet,
}
impl DevtoolsInspectable for Props {}

/// Equal when what is shown is: the signal that reads the hub again stays the screen's own.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.repo_root == other.repo_root
      && self.project == other.project
      && self.may_change == other.may_change
      && self.admins == other.admins
      && self.reading == other.reading
      && self.listing == other.listing
      && self.entry == other.entry
      && self.sheet == other.sheet
  }
}

/// The id the person-only check of a fix is made under.
const FIX: &str = "team-hub-fix";

pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<McpPanel>(&format!("team-hub-mcp-{}", props.entry.id), props)
}

/// The panel of one server: it is keyed by the resource, so an open dialog or a typed value never carries over to
/// another server.
struct McpPanel {
  actions: Actions,
  secret: secret::Entry,
  person: PersonOnly,
}

impl Component for McpPanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let panel = Self {
      actions: Actions::new(ctx, &props.repo_root),
      secret: secret::Entry::new(ctx),
      person: PersonOnly::new(ctx),
    };
    panel.secret.follow_save(ctx, &panel.actions.add_value);
    panel
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let Props {
      listing, entry, sheet, ..
    } = &props;
    let missing = entry.standing.as_ref().map_or(&[][..], |standing| standing.missing());
    let busy = self.actions.busy(props.reading);
    let now = Services::of(ctx).clock.now();
    let press = |step: &Step| self.press(step, &props);
    let view = computer::View {
      listing,
      entry,
      sheet,
      missing,
      busy,
      now,
      choose: press(&Step::Fix(Fix::ChoosePath(entry.id))),
      switch: self.switch(entry.id),
    };
    let chips = panel::chips_saying(
      ctx,
      entry,
      words::chip(ctx, listing, entry, missing),
      panel::used_by(ctx, &sheet.used_by),
    );
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        HubSpacing::Sections.role(),
        HubSpacing::BodyTop.role(),
      ))
      .spacing(HubSpacing::BodyTop.role())
      .child(panel::head_saying(
        ctx,
        entry,
        words::head_line(ctx, now, entry, sheet),
        None,
      ))
      .child(chips)
      .with_children(member::lock_line(ctx, props.may_change, &props.admins))
      .child(from_team::section(ctx, sheet))
      .child(computer::section(ctx, &view))
      .with_children(self.failure(ctx));
    let scroll = ScrollVertical::new(body)
      .id("team-hub-panel-scroll")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0));
    let bar = Bar {
      missing,
      changes_path: sheet.program.is_some().then_some(entry.id),
      busy,
      press: &press,
    }
    .render(ctx);
    let dialog = self.secret.dialog(
      ctx,
      secret::Entry::save_by(&self.actions.add_value, &self.person),
      self.actions.add_value.state().get().is_pending(),
      self.person.reason(ctx, secret::SAVE),
    );
    Column::new()
      .id("team-hub-panel")
      .describe("resource", entry.id.to_string())
      .describe("kind", entry.kind.name())
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Pct(100.0))
      .child(scroll)
      .with_children(bar)
      .with_children(dialog)
  }
}

impl Drop for McpPanel {
  fn drop(&mut self) {
    self.secret.wipe();
  }
}

impl McpPanel {
  /// What a press on `step` does: what widens what agents may do is a person's alone.
  fn press(&self, step: &Step, props: &Props) -> Press {
    let person = self.person.clone();
    let guarded = |action: Arc<dyn Fn() + Send + Sync>| {
      Press::run(move || {
        if person.allows(FIX) {
          action();
        }
      })
    };
    match step {
      Step::Fix(Fix::SwitchOn(server)) => {
        let (action, server) = (self.actions.switch_on.clone(), *server);
        guarded(Arc::new(move || action.run(server)))
      }
      Step::Fix(Fix::ChoosePath(server)) => {
        let (action, server) = (self.actions.set_path.clone(), *server);
        guarded(Arc::new(move || action.run(server)))
      }
      Step::Fix(Fix::AddValue(name)) => {
        let (entry, name) = (self.secret.clone(), name.clone());
        guarded(Arc::new(move || entry.ask(&name)))
      }
      Step::CheckAgain => {
        let reread = props.reread.clone();
        Press::run(move || reread.update(|n| *n += 1))
      }
    }
  }

  /// What flipping the switch does: on is a person's alone; off is always taken.
  fn switch(&self, server: ResourceId) -> Arc<dyn Fn(bool) + Send + Sync> {
    let (on, off, person) = (
      self.actions.switch_on.clone(),
      self.actions.switch_off.clone(),
      self.person.clone(),
    );
    Arc::new(move |now| {
      if !now {
        off.run(server);
      } else if person.allows(FIX) {
        on.run(server);
      }
    })
  }

  /// Why the last fix did nothing: what the workspace refused with, or that automation pressed it. What went wrong
  /// with a secret's value shows in its dialog.
  fn failure(&self, ctx: &Ctx) -> Option<Element> {
    if let Some(reason) = self.actions.refused() {
      return Some(field_error("team-hub-fix-error", &message_text(ctx, &reason)).into());
    }
    self.person.reason(ctx, FIX)
  }
}
