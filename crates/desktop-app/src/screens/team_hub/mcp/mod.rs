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
mod copy_changes;
mod edit;
mod edit_actions;
mod edit_save;
mod edit_view;
mod fix;
mod from_team;
mod modes;
mod refused;
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
use orchester_workspace::{agents::team_mcp::TeamMcpSheet, sharing::team::TeamConfiguration};

use self::{
  edit::Edit,
  fix::{Fix, Step},
  modes::Mode,
};
use super::{
  actions::Actions,
  member,
  model::{Entry, Listing},
  panel, secret,
};
use crate::{
  mcp::input_gate::InputGate,
  screens::settings::orchester::PersonOnly,
  services::Services,
  theme::HubSpacing,
  ui::{field::field_error, message::message_text, pressable::Press},
};

#[derive(Clone)]
pub(super) struct Props {
  pub repo_root: PathBuf,
  pub project: ProjectId,
  /// The team's configuration: an admin may change its resources, and a member reads who the admins are.
  pub team: TeamConfiguration,
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
      && self.team == other.team
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
  /// An admin's edit of the team's part.
  edit: Edit,
  gate: Option<InputGate>,
}

impl Component for McpPanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let panel = Self {
      actions: Actions::new(ctx, &props.repo_root),
      secret: secret::Entry::new(ctx),
      person: PersonOnly::new(ctx),
      edit: Edit::new(
        ctx,
        (props.repo_root.clone(), props.project, props.entry.id),
        props.sheet.edit.as_ref(),
      ),
      gate: ctx.use_context::<InputGate>(),
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
    let mode = self.mode(&props);
    let kept = self.autosave(ctx, &props, mode);
    let parts = self
      .parts(ctx, &props, (mode, &kept))
      .unwrap_or_else(|| self.plain_parts(ctx, &props, (busy, &press)));
    let chips = panel::chips_of(ctx, entry, parts.chips.badges, parts.chips.end);
    let computer = parts.computer.then(|| computer::section(ctx, &view));
    let (above, from_team, bar) = (parts.above, parts.from_team, parts.bar);
    let edit_button = (mode == Mode::Plain).then(|| self.edit_button(ctx, &props)).flatten();
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
        words::head_line(ctx, now, entry, (sheet, &props.team)),
        edit_button,
      ))
      .with_children(above)
      .child(chips)
      .with_children(self.action_failure(ctx))
      .with_children(member::lock_line(ctx, &props.team).filter(|_| mode != Mode::ReadOnly))
      .child(from_team)
      .with_children(computer)
      .with_children(self.failure(ctx));
    let scroll = ScrollVertical::new(body)
      .id("team-hub-panel-scroll")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0));
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
