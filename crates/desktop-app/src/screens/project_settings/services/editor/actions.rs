//! The pane's last line (`Actions` `doe9q`): *Delete service* at its start, and at its end where the service stands
//! (a draft, which version is published) with *Publish vN* while there is a draft to publish — the person's choice of
//! where Publish goes, as the design draws none. Publish waits, and says why, while the fields hold a change not saved
//! yet or a mistake, while a placeholder cannot be used, and for a draft started from an older version. Under the
//! line: why a save or a publish failed, or that an MCP client's input was not taken.

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::VersionNumber;
use orchester_workspace::{agent_testing::editor::order::Field, agents::data::VersionState};

use super::{Editor, EditorProps, View};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
  },
};

/// The actions line and what it says under it.
pub(super) fn actions(ctx: &mut Ctx, editor: &Editor, props: &EditorProps, view: &View) -> Element {
  let delete = view.stored.entry.as_ref().map(|_| {
    let deleting = editor.deleting.clone();
    Element::from(
      ActionButton::new(
        "settings-service-delete",
        ButtonKind::DangerGhost,
        ctx.t("project_settings.services.editor.delete.button"),
      )
      .icon(Icon::Trash2)
      .on_click(move || deleting.set(true)),
    )
  });
  let state = Text::new(&state_words(ctx, view))
    .id("settings-service-state")
    .variant(ExtraTypography::Note.role());
  let line = Row::new()
    .id("settings-service-actions")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .with_children(delete)
    .child(Spacer::new().flex(1.0))
    .child(state)
    .with_children(publish(ctx, editor, props, view));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(line)
    .with_children(said(ctx, editor, view))
    .into()
}

/// Where the service stands, as stored.
fn state_words(ctx: &Ctx, view: &View) -> std::sync::Arc<str> {
  let Some(entry) = &view.stored.entry else {
    return ctx.t("project_settings.services.editor.state.new");
  };
  let version = |number: VersionNumber| number.get().to_string();
  match entry.state {
    VersionState::Draft { published: None } | VersionState::StaleDraft { published: None, .. } => {
      ctx.t("project_settings.services.editor.state.draftNew")
    }
    VersionState::Draft {
      published: Some(published),
    } => ctx.t_args(
      "project_settings.services.editor.state.draft",
      [("version", version(published))],
    ),
    VersionState::StaleDraft {
      based_on,
      published: Some(published),
    } => ctx.t_args(
      "project_settings.services.editor.state.stale",
      [
        ("based", based_on.map_or_else(String::new, version)),
        ("version", version(published)),
      ],
    ),
    VersionState::Published(published) => ctx.t_args(
      "project_settings.services.editor.state.published",
      [("version", version(published))],
    ),
  }
}

/// *Publish vN* while the service has a draft, or while the fields hold a change that will be one.
fn publish(ctx: &Ctx, editor: &Editor, props: &EditorProps, view: &View) -> Option<Element> {
  let entry = view.stored.entry.as_ref()?;
  if !entry.state.has_draft() && !view.changed {
    return None;
  }
  let next = entry
    .state
    .published()
    .map_or(Some(VersionNumber::FIRST), VersionNumber::next)?;
  let button = ActionButton::new(
    "settings-service-publish",
    ButtonKind::Primary,
    ctx.t_args(
      "project_settings.services.editor.publish",
      [("version", next.get().to_string())],
    ),
  )
  .icon(Icon::Upload);
  let waits = if let VersionState::StaleDraft {
    published: Some(published),
    ..
  } = entry.state
  {
    Some(ctx.t_args(
      "project_settings.services.editor.publishStale",
      [("version", published.get().to_string())],
    ))
  } else if view.built.is_err() {
    Some(ctx.t("project_settings.services.editor.publishWaitsFix"))
  } else if view.changed || editor.save.state().get().is_pending() {
    Some(ctx.t("project_settings.services.editor.publishWaitsSave"))
  } else {
    view.placeholder.as_ref().map(|(_, problem)| message_text(ctx, problem))
  };
  let button = match (waits, &view.built) {
    (Some(reason), _) => button.unavailable(reason),
    (None, _) if editor.publish.state().get().is_pending() => button.loading(),
    (None, Ok(service)) => {
      let (editor, root, id, service) = (
        editor.clone(),
        props.tab.project.open.repo_root.clone(),
        entry.id,
        service.clone(),
      );
      let latest = view.stored.latest.clone();
      button.on_click(move || editor.publish_now(root.clone(), (id, service.clone()), latest.as_ref()))
    }
    (None, Err(_)) => button.disabled(),
  };
  Some(button.into())
}

/// What the line says under it: that an MCP client's input was not taken, why a save or a publish failed, and a
/// placeholder problem in a part the page does not edit.
fn said(ctx: &Ctx, editor: &Editor, view: &View) -> Vec<Element> {
  let mut said = Vec::new();
  if editor.refused.get() {
    said.push(field_error(
      "settings-service-person-only",
      &ctx.t("project_settings.services.editor.personOnly"),
    ));
  }
  for (id, state) in [
    ("settings-service-save-error", editor.save.state().get()),
    ("settings-service-publish-error", editor.publish.state().get()),
  ] {
    if let Some(error) = state.error.as_ref().filter(|_| state.is_rejected()) {
      said.push(field_error(id, &message_text(ctx, error)));
    }
  }
  if let Some((Field::Elsewhere, problem)) = &view.placeholder {
    said.push(field_error(
      "settings-service-placeholder-error",
      &message_text(ctx, problem),
    ));
  }
  said.into_iter().map(Element::from).collect()
}
