//! A team MCP server the team archived while an admin edited it (`Agents · MCP server · admin · draft` A21 `yqYj9`): a
//! publish is refused, the draft is kept here, and the way back is *Restore*, which an admin confirms in a modal and
//! which brings the server back for everyone; then the draft can be published. The page is the draft read-only, with
//! *Discard my draft*, which asks in a modal like any discard of work.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Text,
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;
use orchester_workspace::{agents::team_mcp_edit::McpEdit, display, messages::Message};

use super::{
  McpPanel, Props, edit_view,
  modes::{Chips, Parts, note_text, section},
};
use crate::{
  mcp::input_gate::{Guarded, input_may_decide},
  screens::team_hub::bar,
  theme::{DialogText, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::BadgeTone,
    dialog::{Dialog, Tone},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

impl McpPanel {
  /// The page of the archived server whose draft is kept; `at` is when it was archived. `None` while no draft is kept.
  pub(super) fn archived(&self, ctx: &Ctx, props: &Props, edit: &McpEdit, at: Timestamp) -> Option<Parts> {
    let draft = edit.draft.as_ref()?;
    let name = props.entry.name.as_str();
    let version = draft.based_on.to_string();
    let when = display::clock(at);
    let notice = Notice::new(
      "team-hub-archived-notice",
      NoticeTone::Danger,
      ctx.t_args(
        "team_hub.archived.title",
        [("name", name.to_owned()), ("time", when.to_string())],
      ),
    )
    .icon(Icon::Archive)
    .title_style(HubText::NoticeTitle.role())
    .body(ctx.t_args(
      "team_hub.archived.body",
      [("version", version.clone()), ("name", name.to_owned())],
    ))
    .stacked()
    .flat();
    let end = match self.edit.refused.get() {
      Some((_, refused)) => ctx.t_args("team_hub.conflict.refusedAt", [("time", display::clock(refused))]),
      None => ctx.t_args("team_hub.archived.at", [("time", when)]),
    };
    let rows = edit_view::read_only_rows(ctx, &draft.fields, &edit.published);
    let words = ctx.t_args("team_hub.archived.fromTeam", [("version", version.clone())]);
    Some(Parts {
      chips: Chips {
        badges: vec![
          (ctx.t("team_hub.archived.chip"), BadgeTone::Danger),
          (
            ctx.t_args("team_hub.edit.chip.draft", [("version", version)]),
            BadgeTone::Warning,
          ),
        ],
        end: Some(end),
      },
      above: vec![notice.into()],
      from_team: section(ctx, vec![note_text("team-hub-from-team-note", &words).into()], rows),
      computer: false,
      bar: Some(self.archived_bar(ctx, props, name)),
      dialogs: vec![
        self.restore_dialog(ctx, props, edit),
        self.discard_dialog(
          ctx,
          name,
          &ctx.t_args("team_hub.archived.discard", [("name", name.to_owned())]),
          None,
        ),
      ],
    })
  }

  /// *Restore figma* and *Discard my draft*, and what restoring does (A21). A member has neither *Restore* nor a
  /// disabled one: only an admin restores, and the hint says whom to ask.
  fn archived_bar(&self, ctx: &Ctx, props: &Props, name: &str) -> Element {
    let discard = ActionButton::new(
      "team-hub-archived-discard",
      ButtonKind::DangerGhost,
      ctx.t("team_hub.edit.bar.discardMine"),
    )
    .icon(Icon::Trash2)
    .pinned();
    let confirm = self.edit.confirm_discard.clone();
    let discard = discard.on_click(move || confirm.set(true));
    if !props.team.may_change {
      let admins = crate::screens::hub::team::admins(ctx, &props.team);
      return bar::bar(
        vec![discard.into()],
        Some(
          ctx
            .t_args("team_hub.edit.bar.askAdmin", [("admins", admins)])
            .to_string(),
        ),
      );
    }
    let restore = ActionButton::new(
      "team-hub-archived-restore",
      ButtonKind::Primary,
      ctx.t_args("team_hub.archived.bar.restore", [("name", name.to_owned())]),
    )
    .icon(Icon::ArchiveRestore)
    .pinned();
    let restore = if self.edit.restore.state().get().is_pending() {
      restore.loading()
    } else {
      let confirm = self.edit.confirm_restore.clone();
      restore.on_click(move || confirm.set(true))
    };
    let version = props.entry.number.map_or_else(String::new, |number| number.to_string());
    bar::bar_saying(
      vec![restore.into(), discard.into()],
      Some((
        ctx
          .t_args("team_hub.archived.bar.hint", [("version", version)])
          .to_string(),
        PaletteColor::TextMuted,
      )),
    )
  }

  /// *Restore figma?*: it comes back for everyone, which an admin confirms. Only a person (or automation they let
  /// decide) confirms it.
  fn restore_dialog(&self, ctx: &Ctx, props: &Props, edit: &McpEdit) -> Element {
    let name = props.entry.name.clone();
    let (restore, confirm, gate, failure) = (
      self.edit.restore.clone(),
      self.edit.confirm_restore.clone(),
      self.gate.clone(),
      self.edit.failure.clone(),
    );
    let action = ActionButton::new(
      "team-hub-restore-confirm",
      ButtonKind::Primary,
      ctx.t("team_hub.archived.restore.confirm"),
    )
    .icon(Icon::ArchiveRestore)
    .on_click(move || {
      confirm.set(false);
      if input_may_decide(gate.as_ref(), Guarded::PermissionWidening) {
        restore.run(true);
      } else {
        failure.set(Some(Message::new("sharing.errors.personOnly")));
      }
    });
    let draft = edit
      .draft
      .as_ref()
      .map_or_else(String::new, |draft| draft.based_on.to_string());
    Dialog::new(
      "team-hub-restore-dialog",
      Icon::ArchiveRestore,
      ctx.t_args("team_hub.archived.restore.title", [("name", name.clone())]),
      ctx.t("team_hub.archived.restore.cancel"),
    )
    .tone(Tone::Neutral)
    .plain_footer()
    .part(
      Text::new(&ctx.t_args("team_hub.archived.restore.body", [("name", name), ("draft", draft)]))
        .id("team-hub-restore-body")
        .variant(DialogText::Detail.role())
        .width(Dimension::Pct(100.0)),
    )
    .action(action)
    .render(ctx, &self.edit.confirm_restore)
  }
}
