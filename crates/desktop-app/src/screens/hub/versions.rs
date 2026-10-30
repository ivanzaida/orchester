//! Versions of one of the team's resources side by side (`Shared configuration · redesign` `UC5xk`): a body's fields
//! by their names; *Compare with vN* on the conflict bar, which holds a newer version and the one published here
//! field by field (02c `JZFlA`); and *Versions* in a member's editor (`Versions` `EGuXw` in 03 `pjEjf`), each version
//! the server numbered with who published it, what it changed and which one runs here use. Both open in a popover.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row, ScrollVertical, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, VersionNumber},
};
use orchester_workspace::{
  background, messages,
  sharing::compare::{self, FieldChange, HistoryEntry},
};

use super::{
  parts::{Tone, status},
  team::when,
};
use crate::{
  theme::{ExtraSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
    popover::{PopoverSide, popover, popover_panel, room_under},
  },
};

/// *Compare with vN*'s popover (`Compare with v5` `oeExg` opens it; the design draws no frame of its own).
const COMPARE_WIDTH: f32 = 560.0;
/// *Versions*' popover.
const VERSIONS_WIDTH: f32 = 380.0;

/// A field of a resource's body by its name there, in the person's words: `instructions` is *role instructions*. A
/// field this build does not know keeps its name.
pub(crate) fn field_name(ctx: &Ctx, field: &str) -> Arc<str> {
  let key = match field {
    "role" => "sharing.config.field.role",
    "responsibilities" => "sharing.config.field.responsibilities",
    "instructions" => "sharing.config.field.instructions",
    "connection" => "sharing.config.field.connection",
    "model" => "sharing.config.field.model",
    "settings" => "sharing.config.field.settings",
    "permissions" => "sharing.config.field.permissions",
    "mcp_servers" => "sharing.config.field.mcpServers",
    "text" => "sharing.config.field.text",
    "targets" => "sharing.config.field.targets",
    "events" => "sharing.config.field.events",
    "scope" => "sharing.config.field.scope",
    "description" => "sharing.config.field.description",
    "body" => "sharing.config.field.body",
    "files" => "sharing.config.field.files",
    "action" => "sharing.config.field.action",
    "access" => "sharing.config.field.access",
    "sources" => "sharing.config.field.sources",
    "steps" => "sharing.config.field.steps",
    "deliver" => "sharing.config.field.deliver",
    "delivers" => "sharing.config.field.delivers",
    _ => return Arc::from(field),
  };
  ctx.t(key)
}

/// `role instructions, model`: `fields` in the person's words.
pub(crate) fn field_names(ctx: &Ctx, fields: &[Arc<str>]) -> String {
  fields
    .iter()
    .map(|field| field_name(ctx, field).to_string())
    .collect::<Vec<_>>()
    .join(&ctx.t("sharing.config.separator"))
}

/// *Compare with vN*'s panel: each field the version published here holds differently from `latest`, the two values
/// side by side.
pub(crate) fn compare_panel(ctx: &Ctx, id: &str, latest: VersionNumber, changes: &[FieldChange]) -> Column {
  let latest = latest.get().to_string();
  let mut fields = Column::new().width(Dimension::Pct(100.0)).spacing(SpacingSize::Md);
  if changes.is_empty() {
    fields = fields.child(
      Text::new(&ctx.t_args("sharing.config.compare.same", [("latest", latest.clone())]))
        .id(format!("{id}-same"))
        .variant(HubText::Hint.role())
        .width(Dimension::Pct(100.0)),
    );
  }
  for change in changes {
    let side = |label: Arc<str>, value: &str| {
      let value = if value.is_empty() {
        ctx.t("sharing.config.compare.none")
      } else {
        Arc::from(value)
      };
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Hair.role())
        .child(Text::new(&label).variant(HubText::Hint.role()))
        .child(
          Text::new(&value)
            .variant(HubText::Value.role())
            .width(Dimension::Pct(100.0)),
        )
    };
    fields = fields.child(
      Column::new()
        .id(format!("{id}-field-{}", change.field))
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(&field_name(ctx, &change.field)).variant(HubText::Kind.role()))
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .spacing(SpacingSize::Md)
            .align_items(Alignment::Start)
            .child(side(
              ctx.t_args("sharing.config.compare.latest", [("latest", latest.clone())]),
              &change.from,
            ))
            .child(side(ctx.t("sharing.config.compare.mine"), &change.to)),
        ),
    );
  }
  popover_panel(id, COMPARE_WIDTH)
    .child(
      Text::new(&ctx.t_args("sharing.config.compare.title", [("latest", latest)]))
        .id(format!("{id}-title"))
        .variant(HubText::GroupTitle.role()),
    )
    .child(scrolled(id, fields))
}

/// `content` scrolling where the popover is too short for it, clear of its scrollbar at its end.
fn scrolled(id: &str, content: Column) -> ScrollVertical {
  ScrollVertical::new(content.padding(Padding::new().right(SpacingSize::Md)))
    .id(format!("{id}-scroll"))
    .width(Dimension::Pct(100.0))
    .flex_shrink(1.0)
}

/// `panel` under `anchor` while `open` holds, as tall as the room under it lets it be.
pub(crate) fn opened(ctx: &Ctx, anchor: &ElementRef, open: &Signal<bool>, panel: Option<Column>) -> Element {
  // Declared while closed too, so that a press on its button switches to it from another open dropdown.
  let panel: Element = match (panel, room_under(ctx, anchor, 0.0)) {
    (Some(panel), Some(room)) => panel.max_height(Dimension::Px(room)).into(),
    (Some(panel), None) => panel.into(),
    (None, _) => Element::new(),
  };
  popover(anchor.clone(), open.clone(), PopoverSide::BelowEnd, panel).into()
}

#[derive(Clone, PartialEq)]
pub(crate) struct VersionsProps {
  pub id: Arc<str>,
  pub root: PathBuf,
  pub resource: (ResourceKind, ResourceId),
}

impl DevtoolsInspectable for VersionsProps {}

/// *Versions* in a member's editor: the button and, while it is open, the versions the server numbered.
pub(crate) struct VersionsButton {
  open: Signal<bool>,
  anchor: ElementRef,
}

/// *Versions*, mounted for a resource of the team in a member's editor head.
pub(crate) fn versions_button(ctx: &mut Ctx, props: VersionsProps) -> Element {
  let key = format!("{}-versions", props.id);
  ctx.mount_keyed::<VersionsButton>(&key, props)
}

impl Component for VersionsButton {
  type Props = VersionsProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<VersionsProps>().clone();
    let id = format!("{}-versions", props.id);
    let open = self.open.clone();
    let button = ActionButton::new(id.clone(), ButtonKind::Ghost, ctx.t("sharing.config.versions.button"))
      .icon(Icon::History)
      .held(self.open.get())
      .element_ref(self.anchor.clone())
      .on_click(move || open.update(|open| *open = !*open));
    let panel = self.open.get().then(|| self.panel(ctx, &props, &id));
    Row::new()
      .child(button)
      .child(opened(ctx, &self.anchor, &self.open, panel))
  }
}

impl VersionsButton {
  fn panel(&self, ctx: &mut Ctx, props: &VersionsProps, id: &str) -> Column {
    let (root, (kind, resource)) = (props.root.clone(), props.resource);
    let read = ctx.future((root, kind, resource), |(root, kind, resource)| async move {
      background::run(move || compare::history(&root, kind, resource))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the versions of a resource of the team");
          messages::store_error(&error)
        })
    });
    let state = read.state().get();
    let panel = popover_panel(&format!("{id}-panel"), VERSIONS_WIDTH).child(
      Text::new(&ctx.t("sharing.config.versions.title"))
        .id(format!("{id}-title"))
        .variant(HubText::GroupTitle.role()),
    );
    match (&state.error, &state.data) {
      (Some(error), _) if state.is_rejected() => {
        panel.child(field_error(&format!("{id}-error"), &message_text(ctx, error)))
      }
      (_, Some(history)) if history.is_empty() => panel.child(
        Text::new(&ctx.t("sharing.config.versions.none"))
          .id(format!("{id}-none"))
          .variant(HubText::Hint.role()),
      ),
      (_, Some(history)) => {
        let rows = history.iter().fold(
          Column::new().width(Dimension::Pct(100.0)).spacing(SpacingSize::Md),
          |rows, entry| rows.child(version_row(ctx, id, entry)),
        );
        panel.child(scrolled(id, rows))
      }
      _ => panel,
    }
  }
}

/// `v4 · dana-k · Oct 27`, *Used here* on the one runs here use, and what it changed under it.
fn version_row(ctx: &Ctx, id: &str, entry: &HistoryEntry) -> Column {
  let number = entry.number.get();
  let line = ctx.t_args(
    "sharing.config.versions.line",
    [
      ("version", number.to_string()),
      (
        "name",
        entry
          .publisher
          .clone()
          .unwrap_or_else(|| ctx.t("sharing.config.here").to_string()),
      ),
      ("when", when(ctx, entry.published_at)),
    ],
  );
  let changed = if number == 1 {
    ctx.t("sharing.config.versions.first")
  } else {
    ctx.t_args(
      "sharing.config.versions.changed",
      [("fields", field_names(ctx, &entry.changed))],
    )
  };
  let row = format!("{id}-v{number}");
  Column::new()
    .id(row.clone())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Hair.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Text::new(&line).variant(HubText::Value.role()).flex(1.0))
        .with_children(entry.used_here.then(|| {
          status(
            &format!("{row}-used"),
            Tone::Success,
            &ctx.t("sharing.config.head.used"),
          )
        })),
    )
    .child(
      Text::new(&changed)
        .variant(HubText::Hint.role())
        .width(Dimension::Pct(100.0)),
    )
}
