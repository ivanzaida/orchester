//! Saving and publishing what the Orchestrator runs on, as agent profiles are (`Save state` `WcBtl` in `b8gGj`,
//! `Orchestrator settings · states` `qAq0z`): where the choices stand (*Published v2*, *Unsaved changes · Published
//! v2*, *Draft saved · Published v2*, *Not published*), *Save draft* while there are unsaved changes, *Publish vN*,
//! and under them why publishing waits. A project's first version is published straight from the form; after that
//! a change is saved as a draft first.

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_core::orchestrator::OrchestratorSettings;
use orchester_workspace::{
  messages::Message,
  orchestrator::settings::{Keep, OrchestratorSetup, Saving},
};

use crate::{
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
  },
};

/// Saving or publishing what the Orchestrator runs on.
pub(super) type Save = FutureAction<Saving, (), Message>;

/// Where the form's choices stand against what is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stand {
  /// Nothing is published or drafted: the first version is published from the form.
  First,
  /// The form differs from the draft, or from the published version when there is none.
  Unsaved,
  /// The draft is saved and not published.
  Drafted,
  /// The form shows the published version.
  Published,
}

/// What the bar is about: the project at `root`, what is kept, and the form's complete choice, if it is one the
/// Orchestrator can run on.
pub(super) struct Publishing<'a> {
  pub(super) root: &'a std::path::Path,
  pub(super) setup: &'a OrchestratorSetup,
  pub(super) chosen: Option<OrchestratorSettings>,
  /// What the form lacks while it has no choice.
  pub(super) incomplete: Option<Incomplete>,
}

/// What the form lacks for a choice that can be saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Incomplete {
  /// No connection the Orchestrator can run on.
  Connection,
  /// The model id is missing or is none.
  Model,
}

impl Publishing<'_> {
  fn stand(&self) -> Stand {
    let kept = self.setup.current();
    if kept.is_none() {
      return Stand::First;
    }
    if self.chosen.as_ref() != kept {
      Stand::Unsaved
    } else if self.setup.draft.is_some() {
      Stand::Drafted
    } else {
      Stand::Published
    }
  }
}

/// The status, the buttons and the note under them.
pub(super) fn bar(ctx: &mut Ctx, save: &Save, publishing: &Publishing<'_>) -> Element {
  let state = save.state().get();
  let stand = publishing.stand();
  let pending = state.is_pending();
  let (publish, save_draft) = buttons(ctx, save, publishing, (stand, pending));
  let under = match &state.error {
    Some(error) => {
      let why = message_text(ctx, error);
      Some(field_error(
        "orchestrator-settings-save-error",
        &ctx.t_args("orchestrator.settings.saveFailed", [("reason", &*why)]),
      ))
    }
    None => note(ctx, publishing, stand).map(|note| {
      Row::new().child(
        Text::new(&note)
          .id("orchestrator-settings-publish-note")
          .variant(TypographyStyle::Caption)
          .nowrap(),
      )
    }),
  };
  Column::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::End)
    .child(
      Row::new()
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(status(ctx, stand, publishing.setup.version))
        .child(publish)
        .with_children(save_draft),
    )
    .with_children(under)
    .into()
}

/// Where the choices stand, against the published version `version`.
fn status(ctx: &Ctx, stand: Stand, version: Option<u32>) -> Row {
  let published = version.map_or_else(
    || ctx.t("orchestrator.settings.publish.notPublished"),
    |version| {
      ctx.t_args(
        "orchestrator.settings.publish.published",
        [("version", version.to_string())],
      )
    },
  );
  let (glyph, color, words) = match stand {
    Stand::First => (Icon::CircleDashed, PaletteColor::TextMuted, published),
    Stand::Unsaved => (
      Icon::CircleDot,
      PaletteColor::TextMuted,
      ctx.t_args("orchestrator.settings.publish.unsaved", [("published", &*published)]),
    ),
    Stand::Drafted => (
      Icon::Check,
      PaletteColor::TextMuted,
      ctx.t_args("orchestrator.settings.publish.drafted", [("published", &*published)]),
    ),
    Stand::Published => (Icon::CircleCheck, PaletteColor::Success, published),
  };
  Row::new()
    .id("orchestrator-settings-saved")
    .describe("stand", format!("{stand:?}").to_lowercase())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color))
    .child(
      Text::new(&words)
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
}

/// Why publishing waits, or what it does; `None` when there is nothing to say.
fn note(ctx: &Ctx, publishing: &Publishing<'_>, stand: Stand) -> Option<std::sync::Arc<str>> {
  if publishing.chosen.is_none() {
    return Some(match publishing.incomplete {
      Some(Incomplete::Model) => ctx.t("orchestrator.settings.publish.needsModel"),
      Some(Incomplete::Connection) | None => ctx.t("orchestrator.settings.publish.needsConnection"),
    });
  }
  match stand {
    Stand::First => Some(ctx.t("orchestrator.settings.publish.firstNote")),
    Stand::Unsaved => Some(ctx.t("orchestrator.settings.publish.saveFirst")),
    Stand::Drafted => None,
    Stand::Published => Some(ctx.t_args(
      "orchestrator.settings.publish.noChanges",
      [("version", publishing.setup.version.unwrap_or_default().to_string())],
    )),
  }
}

/// *Publish vN*, and *Save draft* while there are unsaved changes; neither takes a press while one is saved.
fn buttons(
  ctx: &Ctx,
  save: &Save,
  publishing: &Publishing<'_>,
  (stand, pending): (Stand, bool),
) -> (ActionButton, Option<ActionButton>) {
  let complete = publishing.chosen.is_some();
  let next = publishing.setup.version.map_or(1, |version| version + 1).to_string();
  let run = |keep: Keep| {
    let (save, chosen, root) = (save.clone(), publishing.chosen.clone(), publishing.root.to_path_buf());
    move || {
      if let Some(settings) = chosen.clone() {
        save.run(Saving {
          root: root.clone(),
          settings,
          keep,
        });
      }
    }
  };
  // While changes wait to be saved, Publish is the outlined button (`EPzaF`).
  let kind = if stand == Stand::Unsaved {
    ButtonKind::Secondary
  } else {
    ButtonKind::Primary
  };
  let publish = ActionButton::new(
    "orchestrator-settings-publish",
    kind,
    ctx.t_args("orchestrator.settings.publish.publishAs", [("version", next.as_str())]),
  )
  .icon(Icon::Upload);
  let publish = if complete && !pending && matches!(stand, Stand::First | Stand::Drafted) {
    publish.on_click(run(Keep::Publish))
  } else {
    publish.disabled()
  };
  let save_draft = (stand == Stand::Unsaved).then(|| {
    let button = ActionButton::new(
      "orchestrator-settings-save-draft",
      ButtonKind::Primary,
      ctx.t("orchestrator.settings.publish.saveDraft"),
    )
    .icon(Icon::Save);
    if complete && !pending {
      button.on_click(run(Keep::Draft))
    } else {
      button.disabled()
    }
  });
  (publish, save_draft)
}
