//! Orchester's lurq theme, from the variables and type roles of `data/orchester2.pen` (Theme board
//! `w3uvVL`), with the `graphite` accent in dark and light mode.
//!
//! UI code styles only through lurq roles (`PaletteColor`, `TypographyStyle`, `SpacingSize`,
//! `RadiusSize`, `BorderSize`, `ShadowStyle`) and the extra roles defined here; design values live only
//! in this module.
//!
//! The roles are the design's complete token set on purpose, also where no screen uses one yet (the
//! selection, scrim, conversation, code and diff colors, `display` type, `radius_full`): the screens
//! that need them come next and must find them here rather than add near-duplicates.

mod accounts;
pub mod agents;
mod archive;
pub mod badge;
mod browser;
mod cards;
mod chat_links;
mod ci;
mod code;
mod composer;
mod connections;
mod context;
mod conversation;
mod date_picker;
mod decisions;
mod delivery;
mod dialog;
mod failure;
mod files;
mod fonts;
mod form;
mod groups;
mod hub;
mod import;
pub mod instructions;
mod lifecycle;
mod local_integration;
mod markdown;
mod memory;
mod merge_attempt;
mod metrics;
mod modules;
mod needs_you;
mod orchester;
mod orchestrator;
mod overview;
mod palette;
mod permissions;
pub mod pipelines;
mod project_settings;
mod projects;
mod review;
mod run_model;
mod secrets;
mod settings;
mod shadow;
mod shell;
mod side_question;
mod sign_in;
pub mod skills;
mod slash;
mod sources;
mod startup;
mod task;
mod task_form;
mod tasks;
mod todo_list;
mod typography;
pub mod usage;
mod watching;

use std::time::Duration;

use lurq::{
  app::{App, theme::Theme},
  layout::scrollbar::ScrollBarStyle,
};

pub use self::{
  accounts::{AccountsSpacing, AccountsText},
  archive::{ARCHIVE_DIALOG_WIDTH, ArchiveText, LOCAL_ARCHIVE_DIALOG_WIDTH},
  browser::{BrowserBorder, BrowserColor, BrowserSpacing, BrowserText},
  cards::{CardsSpacing, CardsText, NOT_CHOSEN_OPACITY, RISK_TRACK_HEIGHT},
  chat_links::ChatLinkLooks,
  ci::CiText,
  code::{CodeText, token_color},
  composer::{ComposerColor, ComposerRadius, mention_chip},
  connections::{ConnectionsSpacing, ConnectionsText},
  context::ContextText,
  conversation::{ConversationRadius, ConversationShadow, ConversationSpacing, ConversationText, TRACK_MARK_OPACITY},
  date_picker::{DatePickerText, UNAVAILABLE_DAY_OPACITY},
  decisions::{DecisionSpacing, DecisionText},
  delivery::{DeliverySpacing, DeliveryText, LOCKED_SWITCH_OPACITY},
  dialog::{DIALOG_TILE, DIALOG_WIDTH, DialogSpacing, DialogText, NARROW_DIALOG_WIDTH},
  failure::{FailureSpacing, FailureText},
  files::{FilesRadius, FilesText},
  fonts::mono_space,
  groups::GroupText,
  hub::{HUB_NEW_MENU_WIDTH, HUB_READING_OPACITY, HubSpacing, HubText},
  import::{ImportSpacing, ImportText},
  lifecycle::{CLOSING_TABS_OPACITY, LifecycleSpacing, LifecycleText},
  local_integration::LocalText,
  markdown::{blank_line_between_blocks, code_lines},
  memory::{MemorySpacing, MemoryText},
  merge_attempt::{AttemptSpacing, AttemptText},
  metrics::{ControlHeight, ExtraRadius, ExtraSpacing},
  modules::ModulesSpacing,
  needs_you::{NeedsYouRadius, NeedsYouShadow, NeedsYouSpacing, NeedsYouText},
  orchester::{OrchesterSpacing, OrchesterText},
  orchestrator::{
    LEADS_HINT_OPACITY, OrchestratorShadow, OrchestratorSpacing, OrchestratorText, beside_chat_lane, chat_scrollbar,
    picker_scrollbar, watch_list_scrollbar,
  },
  overview::{OverviewSpacing, OverviewText},
  palette::{ExtraColor, Mode},
  permissions::{PermissionsSpacing, PermissionsText},
  pipelines::{PipelinesSpacing, PipelinesText},
  project_settings::{ProjectSettingsSpacing, ProjectSettingsText},
  projects::{ProjectsSpacing, ProjectsText},
  review::{REVIEW_TYPING_CYCLE, REVIEW_TYPING_OPACITY, REVIEW_TYPING_STEPS, ReviewSpacing, ReviewText},
  run_model::ModelText,
  secrets::{SECRETS_PENDING_OPACITY, SecretsSpacing, SecretsText},
  settings::{SettingsSpacing, SettingsText},
  shadow::ExtraShadow,
  shell::{ShellSpacing, ShellText},
  side_question::{SideSpacing, SideText},
  sign_in::{SignInSpacing, SignInText},
  slash::SlashText,
  sources::{SourcesSpacing, SourcesText},
  startup::{StartupSpacing, StartupText},
  task::{NOT_READY_ROW_HEIGHT, TaskRadius, TaskSpacing, TaskText},
  task_form::{TaskFormSpacing, TaskFormText, sources_scrollbar},
  tasks::{
    DROP_SCRIM_OPACITY, DROP_VEIL_OPACITY, HELD_CARD_OPACITY, SKELETON_DELAY, SKELETON_DIM, SKELETON_FADE,
    SKELETON_LAST_CARD, SKELETON_PULSE, SKELETON_STEPS, TasksSpacing, list_scrollbar, skeleton_line,
  },
  todo_list::TodoText,
  typography::{ExtraTypography, IconSize},
  usage::{UsageRadius, UsageSpacing, UsageText},
  watching::{WatchingSpacing, WatchingText},
};

/// A control that cannot be used, faded as a whole: a button that is unavailable or disabled (`Create
/// multiple` `Q9aeZ`, `Attach context` `p6caAv`).
pub const DISABLED_OPACITY: f32 = 0.5;
/// The messages sending an edit of an earlier message drops, while it is edited in place (09b `QJb0b`).
pub const DROPPED_OPACITY: f32 = 0.4;
/// The split between a menu button's label and its chevron (`Split` `gsHDM` in `Bm2gp`).
pub const MENU_SPLIT_OPACITY: f32 = 0.25;
/// A shortcut beside a tooltip's label (`n75PTn` in `b6q3OZ`).
pub const SHORTCUT_OPACITY: f32 = 0.6;
/// What a primary full-width button does, in `on_accent` under its label (`dwsGC` in `br5EG`).
pub const CONSEQUENCE_ON_ACCENT_OPACITY: f32 = 0.75;
/// One turn of a loader (`loader-circle`), which turns while it is shown; the design draws it still.
pub const LOADER_TURN: Duration = Duration::from_secs(1);
/// The poses of a loader's turn: 24, 15° apart. lurq draws a frame only when the pose changes, so a window
/// with a turning loader draws 24 frames a second on any display, not one every display refresh.
pub const LOADER_STEPS: u32 = 24;

/// Loads the fonts and fills every role for `mode`.
pub fn install(app: &mut App, mode: Mode) {
  fonts::install(app);
  let theme = app.theme();
  apply(theme, mode);
  metrics::install(theme);
  pipelines::install(theme);
  agents::install(theme);
  instructions::install(theme);
  startup::install(theme);
  settings::install(theme);
  accounts::install(theme);
  orchester::install(theme);
  sign_in::install(theme);
  task::install(theme);
  decisions::install(theme);
  task_form::install(theme);
  tasks::install(theme);
  overview::install(theme);
  connections::install(theme);
  secrets::install(theme);
  projects::install(theme);
  shell::install(theme);
  skills::install(theme);
  lifecycle::install(theme);
  conversation::install(theme);
  cards::install(theme);
  composer::install(theme);
  files::install(theme);
  orchestrator::install(theme);
  needs_you::install(theme);
  usage::install(theme);
  memory::install(theme);
  sources::install(theme);
  delivery::install(theme);
  merge_attempt::install(theme);
  review::install(theme);
  import::install(theme);
  hub::install(theme);
  failure::install(theme);
  modules::install(theme);
  project_settings::install(theme);
  browser::install(theme);
  dialog::install(theme);
  permissions::install(theme);
  side_question::install(theme);
  watching::install(theme);
  theme.set_shadows(shadow::shadows());
  theme.set_form(form::form());
}

/// Switches the colors to `mode`: the palette, and the text styles, scrollbar and Markdown whose colors
/// come from it. Every component that reads the theme follows on its next render.
pub fn apply(theme: &Theme, mode: Mode) {
  let palette = palette::palette(mode);
  theme.set_typography(typography::typography(&palette));
  theme.set_scrollbar(ScrollBarStyle {
    thumb_color: palette.border_strong,
    ..ScrollBarStyle::thin()
  });
  theme.set_markdown(markdown::markdown(&palette));
  theme.set_palette(palette);
}
