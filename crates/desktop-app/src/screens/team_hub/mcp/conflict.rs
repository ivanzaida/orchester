//! A team MCP server the team changed while an admin edited it (`Agents · MCP server · admin · draft` A18 `bk4i9`, A19
//! `DV8OI`, A20 `CAPhQ`, rule 18 of `docs/design/team-configuration.md`): one quiet line over the form says so, with
//! *View changes*; a *Publish* that met the newer version (or a version in the outbox that did) is refused as a
//! conflict, and the page compares the base, the team's version and the draft field by field, with *Rebase my draft*,
//! *Open vN* and *Discard my draft*, which asks in a modal. Nothing is chosen for a field both changed.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::VersionId, time::Timestamp};
use orchester_workspace::{
  agents::{team_mcp_conflict::Newer, team_mcp_edit::McpEdit, team_mcp_merge::Verdict},
  display,
  messages::Message,
};

use super::{
  McpPanel, Props, compare,
  modes::{Chips, Parts},
};
use crate::{
  mcp::input_gate::{Guarded, input_may_decide},
  screens::team_hub::{bar, panel},
  theme::{DialogText, HubSpacing, HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::BadgeTone,
    dialog::{Dialog, Tone},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
  },
};

impl McpPanel {
  /// *olha published v6 at 14:02 · your draft is based on v5* and *View changes* (A18): one quiet line over the form,
  /// which changes nothing in the draft. `view` is the button while the page is not comparing already.
  pub(super) fn newer_line(&self, ctx: &Ctx, newer: &Newer, view: bool) -> Element {
    let version = newer.number.to_string();
    let base = newer.base_number.to_string();
    let said = match &newer.published {
      Some(published) => ctx.t_args(
        "team_hub.newer.line",
        [
          ("who", published.by.clone()),
          ("version", version),
          ("time", display::clock(published.at)),
          ("base", base),
        ],
      ),
      None => ctx.t_args("team_hub.newer.lineUnknown", [("version", version), ("base", base)]),
    };
    let comparing = self.edit.comparing.clone();
    let button = view.then(|| {
      ActionButton::new(
        "team-hub-newer-view",
        ButtonKind::Ghost,
        ctx.t("team_hub.newer.viewChanges"),
      )
      .icon(Icon::GitCompare)
      .on_click(move || comparing.set(true))
    });
    Row::new()
      .id("team-hub-newer")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().left(HubSpacing::CompareSide.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Sm)
      .child(icon(Icon::GitBranch, IconSize::Sm, PaletteColor::Info))
      .child(
        Text::new(&said)
          .id("team-hub-newer-text")
          .variant(HubText::TeamLine.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .with_children(button)
      .into()
  }

  /// The page of a draft the team has passed (A19): the refusal, the comparison and the way out. `outbox` is the
  /// version of the person's own that met the team's, and `refused_at` when a publish was refused as a conflict; a
  /// page that was asked for with *View changes* has neither.
  pub(super) fn conflict(
    &self,
    ctx: &Ctx,
    props: &Props,
    edit: &McpEdit,
    (outbox, refused_at): (Option<VersionId>, Option<Timestamp>),
  ) -> Option<Parts> {
    let newer = edit.newer.as_ref()?;
    let both = newer.comparison.count(Verdict::BothChanged);
    let chip = if refused_at.is_some() && both > 0 {
      (
        ctx.t_args(
          if both == 1 {
            "team_hub.conflict.chip.one"
          } else {
            "team_hub.conflict.chip.other"
          },
          [("count", both.to_string())],
        ),
        BadgeTone::Danger,
      )
    } else {
      (
        ctx.t_args("team_hub.edit.chip.draft", [("version", newer.base_number.to_string())]),
        BadgeTone::Warning,
      )
    };
    let end = match refused_at {
      Some(at) => Some(ctx.t_args("team_hub.conflict.refusedAt", [("time", display::clock(at))])),
      None => panel::used_by(ctx, &props.sheet.used_by),
    };
    let above = match refused_at {
      Some(_) => self.conflict_notice(ctx, newer),
      None => self.newer_line(ctx, newer, false),
    };
    Some(Parts {
      chips: Chips {
        badges: vec![chip],
        end,
      },
      above: vec![above],
      from_team: compare::section(ctx, newer, &self.edit.picks),
      computer: false,
      bar: Some(self.conflict_bar(ctx, newer, outbox)),
      dialogs: vec![self.discard_dialog(ctx, &props.entry.name, &discard_words(ctx, newer), outbox)],
    })
  }

  /// *The team's server refused: v6 was published while you edited* (A19).
  fn conflict_notice(&self, ctx: &Ctx, newer: &Newer) -> Element {
    let who = newer.published.as_ref().map_or_else(
      || ctx.t("team_hub.conflict.theTeam").to_string(),
      |published| published.by.clone(),
    );
    Notice::new(
      "team-hub-conflict-notice",
      NoticeTone::Warning,
      ctx.t_args("team_hub.conflict.title", [("version", newer.number.to_string())]),
    )
    .icon(Icon::GitMerge)
    .title_style(HubText::NoticeTitle.role())
    .body(ctx.t_args("team_hub.conflict.body", [("who", who)]))
    .stacked()
    .flat()
    .into()
  }

  /// *Rebase my draft on v6*, *Open v6* and *Discard my draft* (A19), and at the end what rebasing gives, or what is
  /// left to pick before it can.
  fn conflict_bar(&self, ctx: &Ctx, newer: &Newer, outbox: Option<VersionId>) -> Element {
    let version = newer.number.to_string();
    let open = self.edit.open_new.clone();
    let opening = self.edit.open_new.state().get().is_pending();
    let open_button = ActionButton::new(
      "team-hub-conflict-open",
      ButtonKind::Secondary,
      ctx.t_args("team_hub.conflict.bar.open", [("version", version.clone())]),
    )
    .icon(Icon::Eye)
    .pinned();
    let open_button = if opening {
      open_button.loading()
    } else {
      open_button.on_click(move || open.run(outbox))
    };
    let confirm = self.edit.confirm_discard.clone();
    let discard = ActionButton::new(
      "team-hub-conflict-discard",
      ButtonKind::DangerGhost,
      ctx.t("team_hub.edit.bar.discardMine"),
    )
    .icon(Icon::Trash2)
    .pinned()
    .on_click(move || confirm.set(true));
    let missing = compare::unresolved(newer, &self.edit.picks);
    let hint = if missing.is_empty() {
      (
        ctx
          .t_args("team_hub.conflict.bar.gives", [("version", version)])
          .to_string(),
        PaletteColor::TextMuted,
      )
    } else {
      (
        ctx
          .t_args(
            "team_hub.conflict.bar.pick",
            [("fields", compare::names(ctx, &missing))],
          )
          .to_string(),
        PaletteColor::Warning,
      )
    };
    bar::bar_saying(
      vec![
        self.rebase_button(ctx, newer, outbox, missing.is_empty()).into(),
        open_button.into(),
        discard.into(),
      ],
      Some(hint),
    )
  }

  /// *Rebase my draft on v6*: a person's press (or automation's where they let it change what agents may do) merges.
  fn rebase_button(&self, ctx: &Ctx, newer: &Newer, outbox: Option<VersionId>, resolved: bool) -> ActionButton {
    let button = ActionButton::new(
      "team-hub-conflict-rebase",
      ButtonKind::Primary,
      ctx.t_args("team_hub.conflict.bar.rebase", [("version", newer.number.to_string())]),
    )
    .icon(Icon::GitPullRequestArrow)
    .pinned();
    if self.edit.rebase.state().get().is_pending() {
      return button.loading();
    }
    if !resolved {
      return button.disabled();
    }
    let (rebase, gate, picks, expected) = (
      self.edit.rebase.clone(),
      self.gate.clone(),
      self.edit.picks.clone(),
      newer.number,
    );
    button.on_click(move || {
      let decided = input_may_decide(gate.as_ref(), Guarded::PermissionWidening);
      rebase.run((outbox, expected, picks.get(), decided));
    })
  }

  /// *Discard your draft of figma?* (A20): the one step here that loses work asks in a modal. `words` says what is
  /// lost. Only a person (or automation they let decide) confirms it.
  pub(super) fn discard_dialog(&self, ctx: &Ctx, name: &str, words: &str, outbox: Option<VersionId>) -> Element {
    let (discard, confirm, gate, failure) = (
      self.edit.discard.clone(),
      self.edit.confirm_discard.clone(),
      self.gate.clone(),
      self.edit.failure.clone(),
    );
    let action = ActionButton::new(
      "team-hub-conflict-discard-confirm",
      ButtonKind::Danger,
      ctx.t("team_hub.conflict.discard.confirm"),
    )
    .icon(Icon::Trash2)
    .on_click(move || {
      confirm.set(false);
      if input_may_decide(gate.as_ref(), Guarded::PermissionWidening) {
        discard.run(outbox);
      } else {
        failure.set(Some(Message::new("sharing.errors.personOnly")));
      }
    });
    Dialog::new(
      "team-hub-conflict-discard-dialog",
      Icon::Trash2,
      ctx.t_args("team_hub.conflict.discard.title", [("name", name.to_owned())]),
      ctx.t("team_hub.conflict.discard.keep"),
    )
    .tone(Tone::Danger)
    .plain_footer()
    .part(
      Text::new(words)
        .id("team-hub-conflict-discard-body")
        .variant(DialogText::Detail.role())
        .width(Dimension::Pct(100.0)),
    )
    .action(action)
    .render(ctx, &self.edit.confirm_discard)
  }
}

/// What discarding the draft loses, and what stays the team's (A20): the fields only the person, or both, changed.
fn discard_words(ctx: &Ctx, newer: &Newer) -> String {
  let changed: Vec<_> = newer
    .comparison
    .rows
    .iter()
    .filter(|row| matches!(row.verdict, Verdict::BothChanged | Verdict::KeepsMine))
    .map(|row| row.field)
    .collect();
  let who = newer.published.as_ref().map_or_else(
    || ctx.t("team_hub.conflict.theTeam").to_string(),
    |published| published.by.clone(),
  );
  let version = newer.number.to_string();
  if changed.is_empty() {
    ctx
      .t_args(
        "team_hub.conflict.discard.bodyPlain",
        [("version", version), ("who", who)],
      )
      .to_string()
  } else {
    ctx
      .t_args(
        "team_hub.conflict.discard.body",
        [
          ("fields", compare::names(ctx, &changed)),
          ("version", version),
          ("who", who),
        ],
      )
      .to_string()
  }
}
