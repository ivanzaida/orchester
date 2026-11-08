//! Which of its states the panel of a team MCP server is in, and what each puts in the panel (`Agents · MCP server ·
//! admin · draft` A13 `WMtxm`, A14 `gTLx6`, A16 `x3ywL`, A17 `rJbPm`): a form over the team's part with a draft note
//! and *Publish for the team* (editing), the same part read-only with *Cancel sending* (a version waits for the team
//! server), the waiting bar of a version the server did not take or that met another (stuck, with its way out), or
//! read-only with *Discard my draft* (the person is not an admin any more). No version published here is edited on top
//! of: while one is in the outbox the page offers no *Edit*.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row, Text},
  node::Element,
};
use orchester_core::{id::VersionId, time::Timestamp};
use orchester_store::PublishState;
use orchester_workspace::{
  agents::team_mcp_edit::{EditError, McpEdit, Saved},
  app_mcp::setup::NewTeamMcpServer,
  sharing::team_edit::{EditRefused, Refusal},
  tasks::outside::Outside,
};

use super::{
  McpPanel, Props,
  edit_view::{self, EditBar, Kept},
  fix::{Bar, Step},
  from_team, words,
};
use crate::{
  mcp::input_gate::{Guarded, input_may_decide},
  screens::{
    hub::team::waiting_bar_in,
    team_hub::{actions::Busy, create::EditView, panel, rows},
  },
  theme::HubText,
  ui::{
    button::{ActionButton, ButtonKind},
    chip::BadgeTone,
    field::field_error,
    icon::Icon,
    message::message_text,
    pressable::Press,
  },
};

/// What the panel is doing with the team's part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
  /// Nothing is edited: the part is values, and an admin has *Edit*.
  Plain,
  /// An admin edits it as a form.
  Editing,
  /// A version published here waits in the outbox for the team server, which could not be reached (`queued` at); a
  /// `created` resource has no version of the team's yet, and cancelling takes it back whole.
  Waiting {
    version: VersionId,
    queued: Timestamp,
    created: bool,
  },
  /// The first version in the outbox was not taken: the server refused it as a new resource's first, this computer
  /// found the team cannot take it, or it met a version someone published from the same base. It waits for the
  /// person's decision.
  Stuck,
  /// The team server refused (at `at`) the version `version` of an edit, which waits in the outbox: the form shows it
  /// with the reason and *Publish for the team*, which takes it back as the draft and publishes what is typed (A17).
  Refused { version: VersionId, at: Timestamp },
  /// A draft is kept, and the person may not publish it: the team server refused as they are no longer an admin.
  ReadOnly,
  /// The team published since the version the draft was edited from, and the page compares them (A19): a publish was
  /// refused as a conflict (`refused_at`), a version of the person's own in the outbox met the team's (`outbox`), or
  /// *View changes* asked for it.
  Conflict {
    outbox: Option<VersionId>,
    refused_at: Option<Timestamp>,
  },
  /// The team archived the server while the draft is kept (A21).
  Archived { at: Timestamp },
}

/// The chips after *Team · vN*: their words and what stands at their end.
pub(super) struct Chips {
  pub badges: Vec<(Arc<str>, BadgeTone)>,
  pub end: Option<Arc<str>>,
}

/// What a state puts in the panel besides its head.
pub(super) struct Parts {
  pub chips: Chips,
  /// What stands over *From the team*: the draft's note and what the server answered.
  pub above: Vec<Element>,
  pub from_team: Column,
  /// *On this computer* follows *From the team*.
  pub computer: bool,
  pub bar: Option<Element>,
  /// The modals the state may open, over the page.
  pub dialogs: Vec<Element>,
}

impl McpPanel {
  pub(super) fn mode(&self, props: &Props) -> Mode {
    if let Some(at) = props.entry.archived {
      return Mode::Archived { at };
    }
    let standing = props.team.standing(props.entry.id);
    if let Some(unsent) = standing.and_then(|standing| standing.waiting.first().map(|first| (standing, first))) {
      let (standing, first) = unsent;
      return match first.state {
        PublishState::Conflict { .. }
          if props.team.may_change
            && props
              .sheet
              .edit
              .as_ref()
              .and_then(|edit| edit.newer.as_ref())
              .is_some_and(|newer| newer.from_outbox == Some(first.version)) =>
        {
          Mode::Conflict {
            outbox: Some(first.version),
            refused_at: Some(first.published_at),
          }
        }
        PublishState::Waiting => Mode::Waiting {
          version: first.version,
          queued: first.published_at,
          created: standing.confirmed.is_none(),
        },
        PublishState::Refused { .. } if first.based_on.is_some() && props.team.may_change => Mode::Refused {
          version: first.version,
          at: first.published_at,
        },
        PublishState::Conflict { .. } | PublishState::Refused { .. } => Mode::Stuck,
      };
    }
    let Some(edit) = &props.sheet.edit else {
      return Mode::Plain;
    };
    let not_admin = matches!(
      self.edit.refused.get(),
      Some((EditRefused::Refused(Refusal::NotAdmin | Refusal::NotAdminHere), _))
    );
    let conflict = edit.newer.is_some() && props.team.may_change && !not_admin;
    let refused = self.edit.refused.get();
    if edit.draft.is_some() && (!props.team.may_change || not_admin) {
      Mode::ReadOnly
    } else if conflict && (self.edit.comparing.get() || matches!(&refused, Some((EditRefused::Changed { .. }, _)))) {
      Mode::Conflict {
        outbox: None,
        refused_at: refused.map(|(_, at)| at),
      }
    } else if props.team.may_change && self.edit.editing.get() {
      Mode::Editing
    } else {
      Mode::Plain
    }
  }

  /// The input the draft keeps while the form is open (an edit, or the refused version taken up again); none in any
  /// other state, nor while a publish is taken or the draft is being discarded, which a save meanwhile would bring
  /// back.
  pub(super) fn to_keep(&self, mode: Mode) -> Option<NewTeamMcpServer> {
    let busy = self.edit.publish.state().get().is_pending() || self.edit.discard.state().get().is_pending();
    let open = matches!(mode, Mode::Editing | Mode::Refused { .. }) && !busy;
    self.edit.fields.as_ref().filter(|_| open).map(|fields| fields.input())
  }

  /// Keeps what the form holds as the draft ([`Edit::autosave`]); how it ended.
  pub(super) fn autosave(&self, ctx: &mut Ctx, props: &Props, mode: Mode) -> Result<Option<Saved>, EditError> {
    let edit = props.sheet.edit.as_ref();
    self
      .edit
      .autosave(ctx, edit.is_some_and(|edit| edit.draft.is_some()), self.to_keep(mode))
  }

  /// What the state adds to the panel; `None` for the plain panel. `kept` is how the autosave ended.
  pub(super) fn parts(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    (mode, kept): (Mode, &Result<Option<Saved>, EditError>),
  ) -> Option<Parts> {
    if mode == Mode::Stuck {
      return Some(self.stuck(ctx, props));
    }
    let edit = props.sheet.edit.as_ref()?;
    match mode {
      Mode::Plain | Mode::Stuck => None,
      Mode::Conflict { outbox, refused_at } => self.conflict(ctx, props, edit, (outbox, refused_at)),
      Mode::Archived { at } => self.archived(ctx, props, edit, at),
      Mode::Refused { version, at } => self.refused(ctx, props, edit, (version, at)),
      Mode::Editing => self.editing(ctx, props, edit, kept),
      Mode::Waiting {
        version,
        queued,
        created,
      } => Some(self.waiting(ctx, props, edit, (version, queued, created))),
      Mode::ReadOnly => self.read_only(ctx, props, edit),
    }
  }

  fn editing(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    edit: &McpEdit,
    saved: &Result<Option<Saved>, EditError>,
  ) -> Option<Parts> {
    let fields = self.edit.fields.as_ref()?;
    let based = self.edit.base.lock().clone()?;
    let pending = self.edit.publish.state().get().is_pending();
    let form = fields.edit(
      ctx,
      &EditView {
        listing: &props.listing,
        // What is marked *changed in your draft* is what differs from the version the form was opened on, not from a
        // newer one the team published since.
        published: &based.fields,
        locked: pending,
      },
    );
    let input = fields.input();
    let kept = match saved {
      Ok(Some(Saved::Draft(at))) => Kept::At(*at),
      Ok(Some(Saved::Unchanged)) => Kept::Nothing,
      Ok(None) => edit
        .draft
        .as_ref()
        .map_or(Kept::Nothing, |draft| Kept::At(draft.saved_at)),
      Err(_) => Kept::Not,
    };
    let refused = self.edit.refused.get();
    let mut above: Vec<Element> = edit
      .newer
      .iter()
      .map(|newer| self.newer_line(ctx, newer, true))
      .collect();
    above.push(edit_view::draft_note(ctx, based.number, (kept, &form.changed)));
    // Fields that do not make a server yet are said by their rows; a draft the disk or the role would not keep is said
    // here, with why.
    if let Err(EditError::Store(why)) = saved {
      above.push(field_error("team-hub-edit-save-error", &message_text(ctx, why)).into());
    }
    above.extend(
      refused
        .as_ref()
        .map(|(refused, _)| edit_view::refusal(ctx, refused, &props.entry)),
    );
    let publishable = form.valid && !form.changed.is_empty() && !pending;
    let bar = EditBar {
      publish: self.publish_button(ctx, (input, None), publishable, pending),
      discard: self.discard_button(ctx, "team-hub-edit-discard", "team_hub.edit.bar.discard", None),
      refused_at: refused.as_ref().map(|(_, at)| *at),
      reverts: None,
      team: &props.team,
    }
    .render(ctx);
    let note = ctx.t("team_hub.edit.fromTeamEditing");
    let mut chips = vec![(
      ctx.t_args("team_hub.edit.chip.draft", [("version", based.number.to_string())]),
      BadgeTone::Warning,
    )];
    chips.push(readiness_chip(ctx, props));
    Some(Parts {
      chips: Chips {
        badges: chips,
        end: panel::used_by(ctx, &props.sheet.used_by),
      },
      above,
      from_team: section(ctx, vec![note_text("team-hub-from-team-note", &note).into()], form.rows),
      computer: true,
      bar: Some(bar),
      dialogs: Vec::new(),
    })
  }

  fn waiting(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    edit: &McpEdit,
    (version, queued, created): (VersionId, Timestamp, bool),
  ) -> Parts {
    let taking = self.edit.cancel.state().get().is_pending() || self.edit.cancel_created.state().get().is_pending();
    let cancel = |id: &str| {
      let (action, created_action) = (self.edit.cancel.clone(), self.edit.cancel_created.clone());
      let run: Arc<dyn Fn() + Send + Sync> = if created {
        Arc::new(move || created_action.run(()))
      } else {
        Arc::new(move || action.run(version))
      };
      edit_view::cancel_sending(ctx, id, (Some(run), taking))
    };
    let words = if created {
      ctx.t("team_hub.edit.fromTeamWaitingCreated")
    } else {
      ctx.t_args("team_hub.edit.fromTeamWaiting", [("version", edit.number.to_string())])
    };
    let rows = edit_view::read_only_rows(ctx, &edit.published, edit.team.as_ref().unwrap_or(&edit.published));
    Parts {
      chips: Chips {
        badges: vec![(ctx.t("team_hub.edit.chip.waiting"), BadgeTone::Info)],
        end: Some(edit_view::queued(ctx, queued)),
      },
      above: vec![edit_view::waiting(
        ctx,
        (props.entry.number, created.then_some(props.entry.name.as_str())),
        cancel("team-hub-edit-cancel-notice"),
      )],
      from_team: section(ctx, vec![note_text("team-hub-from-team-note", &words).into()], rows),
      computer: false,
      bar: Some(edit_view::waiting_bar(ctx, cancel("team-hub-edit-cancel").pinned())),
      dialogs: Vec::new(),
    }
  }

  /// The first version in the outbox was not taken (refused, or met another version): the waiting bar says why and
  /// offers the way out, which an edit on top of it would only hide; the team's part shows as it is.
  fn stuck(&self, ctx: &mut Ctx, props: &Props) -> Parts {
    let editing = self.edit.editing.clone();
    // A draft taken back is the form's again.
    let reread: Arc<dyn Fn() + Send + Sync> = Arc::new(move || editing.set(true));
    let bar = props.team.standing(props.entry.id).and_then(|standing| {
      waiting_bar_in(
        ctx,
        (&props.repo_root, props.project),
        (&props.team, standing),
        ("team-hub-edit", Outside::Connections),
        reread,
      )
    });
    Parts {
      chips: Chips {
        badges: vec![
          (ctx.t("team_hub.edit.chip.notPublished"), BadgeTone::Warning),
          readiness_chip(ctx, props),
        ],
        end: panel::used_by(ctx, &props.sheet.used_by),
      },
      above: bar.into_iter().collect(),
      from_team: from_team::section(ctx, &props.sheet),
      computer: true,
      bar: None,
      dialogs: Vec::new(),
    }
  }

  fn read_only(&self, ctx: &mut Ctx, props: &Props, edit: &McpEdit) -> Option<Parts> {
    let draft = edit.draft.as_ref()?;
    let copy = super::copy_changes::button(ctx, edit_view::changes_text(ctx, &draft.fields));
    let by_server = matches!(
      self.edit.refused.get(),
      Some((EditRefused::Refused(Refusal::NotAdmin), _))
    );
    let words = ctx.t_args("team_hub.edit.fromTeamReadOnly", [("version", edit.number.to_string())]);
    let rows = edit_view::read_only_rows(ctx, &draft.fields, &edit.published);
    Some(Parts {
      chips: Chips {
        badges: vec![(ctx.t("team_hub.edit.chip.draftNotPublished"), BadgeTone::Warning)],
        end: panel::used_by(ctx, &props.sheet.used_by),
      },
      above: vec![edit_view::not_admin(
        ctx,
        (&props.entry.name, &props.team),
        (by_server, copy),
      )],
      from_team: section(ctx, vec![note_text("team-hub-from-team-note", &words).into()], rows),
      computer: false,
      bar: Some(edit_view::read_only_bar(
        ctx,
        self.discard_button(ctx, "team-hub-edit-discard", "team_hub.edit.bar.discardMine", None),
        &props.team,
      )),
      dialogs: Vec::new(),
    })
  }

  /// The panel that edits nothing: the readiness chip, the team's part as values and the fix of the first missing part
  /// pinned at the bottom.
  pub(super) fn plain_parts(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    (busy, press): (Busy, &dyn Fn(&Step) -> Press),
  ) -> Parts {
    let Props {
      listing, entry, sheet, ..
    } = props;
    let missing = entry.standing.as_ref().map_or(&[][..], |standing| standing.missing());
    Parts {
      chips: Chips {
        badges: vec![words::chip(ctx, listing, entry, missing)],
        end: panel::used_by(ctx, &sheet.used_by),
      },
      above: Vec::new(),
      from_team: from_team::section(ctx, sheet),
      computer: true,
      bar: Bar {
        missing,
        changes_path: sheet.program.is_some().then_some(entry.id),
        busy,
        press,
      }
      .render(ctx),
      dialogs: Vec::new(),
    }
  }

  /// *Publish for the team*: a person's press (or automation's where they let it change what agents may do) sends the
  /// fields as typed; the refused version in the outbox, when there is one, goes back to being the draft first.
  pub(super) fn publish_button(
    &self,
    ctx: &Ctx,
    (input, refused): (NewTeamMcpServer, Option<VersionId>),
    publishable: bool,
    pending: bool,
  ) -> ActionButton {
    let button = ActionButton::new(
      "team-hub-edit-publish",
      ButtonKind::Primary,
      ctx.t("team_hub.edit.bar.publish"),
    )
    .icon(Icon::Upload)
    .pinned();
    if pending {
      return button.loading();
    }
    if !publishable {
      return button.disabled();
    }
    let (publish, gate, base) = (self.edit.publish.clone(), self.gate.clone(), self.edit.base.clone());
    button.on_click(move || {
      let decided = input_may_decide(gate.as_ref(), Guarded::PermissionWidening);
      // A form with no base has nothing to save over: nothing is sent.
      if let Some(based) = base.lock().clone() {
        publish.run((input.clone(), based.number, decided, refused));
      }
    })
  }

  /// *Discard draft*: the draft goes and the form shows the version again; the refused version in the outbox, when
  /// there is one, goes back to being the draft first, and the form shows the team's version.
  pub(super) fn discard_button(&self, ctx: &Ctx, id: &str, key: &str, refused: Option<VersionId>) -> ActionButton {
    let button = edit_view::discard_button(ctx, id, key);
    if self.edit.discard.state().get().is_pending() {
      return button.loading();
    }
    let discard = self.edit.discard.clone();
    button.on_click(move || discard.run(refused))
  }

  /// Why the last *Discard draft* or *Cancel sending* did nothing, under the notices; `None` before one failed.
  pub(super) fn action_failure(&self, ctx: &Ctx) -> Option<Element> {
    let why = self.edit.failure.get()?;
    Some(field_error("team-hub-edit-action-error", &message_text(ctx, &why)).into())
  }

  /// *Edit* in the head of an admin's panel.
  pub(super) fn edit_button(&self, ctx: &Ctx, props: &Props) -> Option<Element> {
    props.team.may_change.then_some(())?;
    props.sheet.edit.as_ref()?;
    let editing = self.edit.editing.clone();
    Some(
      ActionButton::new("team-hub-edit", ButtonKind::Ghost, ctx.t("team_hub.panel.edit"))
        .icon(Icon::Pencil)
        .on_click(move || editing.set(true))
        .into(),
    )
  }
}

/// The panel's readiness chip, as it says it besides the draft's.
pub(super) fn readiness_chip(ctx: &Ctx, props: &Props) -> (Arc<str>, BadgeTone) {
  let missing = props
    .entry
    .standing
    .as_ref()
    .map_or(&[][..], |standing| standing.missing());
  super::words::chip(ctx, &props.listing, &props.entry, missing)
}

pub(super) fn note_text(id: &str, words: &str) -> Text {
  Text::new(words)
    .id(id.to_owned())
    .variant(HubText::PanelHint.role())
    .nowrap()
}

/// *From the team* over `rows`, with `after` on its line.
pub(super) fn section(ctx: &Ctx, after: Vec<Element>, rows: Vec<Row>) -> Column {
  rows::section("team-hub-from-team", &ctx.t("team_hub.profile.fromTeam"), after, rows)
}
