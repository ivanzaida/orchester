//! *Browser program* (`Browser program` `ixBG8` in `j3Lre`, `VGqZe` in `NZECi`): the notice of none found over the
//! box (`Not found` `PzLrb`), then *Browser* with the browser agents use or *None found* at its end (`Browser`
//! `z0jJyh`, `J64Qb`), *Program* with its path and *Choose another program…* or *Choose a program…* (`Program`
//! `LToFT`, `N5GaIM`), and *Browser profiles* with their folder and Copy (`Browser profiles` `bVCKX`).

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::{Ctx, FutureState},
    theme::{PaletteColor, SpacingSize},
  },
  clipboard::copy_to_clipboard,
  components::Row,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_exec::browser::NotABrowser;
use orchester_workspace::{
  messages::Message,
  workspace::{AgentsBrowser, BrowserSettings},
};

use super::{Choice, ChooseAction};
use crate::{
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    copied_value::CopiedValue,
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    setting_rows::{HintTone, SettingGroup, SettingRow, mono_value, value},
  },
};

/// What none found says was looked for, where this platform's browsers are looked for (`orchester_exec::browser`).
#[cfg(target_os = "windows")]
const NOT_FOUND: &str = "settings.browser.notFound.windows";
#[cfg(target_os = "macos")]
const NOT_FOUND: &str = "settings.browser.notFound.macos";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const NOT_FOUND: &str = "settings.browser.notFound.other";

/// What the rows' buttons do and how they stand.
pub(super) struct Actions<'a> {
  pub(super) choose: &'a ChooseAction,
  pub(super) chosen: &'a FutureState<Choice, Message>,
  /// Whether what the section shows is being read again.
  pub(super) looking: bool,
  /// Reads it again: *Look again*.
  pub(super) again: Arc<dyn Fn() + Send + Sync>,
}

pub(super) fn program_group(ctx: &mut Ctx, settings: &BrowserSettings, actions: &Actions) -> Element {
  let group = SettingGroup::new("settings-browser-program", ctx.t("settings.browser.program.title"));
  let group = match &settings.browser {
    AgentsBrowser::NotFound(_) => group.notice(not_found(ctx, actions)),
    _ => group,
  };
  group
    .row(browser_row(ctx, &settings.browser))
    .row(program_row(ctx, &settings.browser, actions))
    .row(profiles_row(ctx, settings.profiles.as_deref()))
    .into()
}

/// None found: what was looked for and where, and *Look again*.
fn not_found(ctx: &Ctx, actions: &Actions) -> Element {
  let again = ActionButton::new(
    "settings-browser-look-again",
    ButtonKind::Secondary,
    ctx.t("settings.browser.notFound.lookAgain"),
  )
  .icon(Icon::RotateCw);
  let again = if actions.looking {
    again.loading()
  } else {
    let look = Arc::clone(&actions.again);
    again.on_click(move || look())
  };
  Notice::new(
    "settings-browser-not-found",
    NoticeTone::Warning,
    ctx.t("settings.browser.notFound.title"),
  )
  .body(ctx.t(NOT_FOUND))
  .action(again)
  .into()
}

/// *Browser*: the browser agents use, by its name after a check, or why there is none.
fn browser_row(ctx: &mut Ctx, browser: &AgentsBrowser) -> Element {
  let (hint, state) = match browser {
    AgentsBrowser::Found(program) => ("settings.browser.found.found", usable(program.name())),
    AgentsBrowser::Chosen(program) => ("settings.browser.found.chosen", usable(program.name())),
    AgentsBrowser::ChosenUnusable(_) => (
      "settings.browser.found.unavailable",
      unusable(&ctx.t("settings.browser.found.unusable")),
    ),
    AgentsBrowser::NotFound(_) => (
      "settings.browser.found.unavailable",
      unusable(&ctx.t("settings.browser.found.none")),
    ),
  };
  SettingRow::new("settings-browser-found", ctx.t("settings.browser.found.label"))
    .hint(ctx.t(hint))
    .control(state)
    .render(ctx)
}

/// A browser's name after a `success` check (`Value` `MkbMA`).
fn usable(name: &str) -> Row {
  state(
    Icon::CircleCheck,
    PaletteColor::Success,
    name,
    PaletteColor::TextSecondary,
  )
}

/// Why there is no browser, in `warning` after its mark (`Value` `rrxpf`).
fn unusable(words: &str) -> Row {
  state(Icon::CircleAlert, PaletteColor::Warning, words, PaletteColor::Warning)
}

fn state(glyph: Icon, mark: PaletteColor, words: &str, color: PaletteColor) -> Row {
  Row::new()
    .id("settings-browser-found-value")
    .flex_shrink(0.0)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, mark))
    .child(value("settings-browser-found-text", words).color(color))
}

/// *Program*: its path, which programs it may be, and the button that chooses another; what came of a choice under it.
fn program_row(ctx: &mut Ctx, browser: &AgentsBrowser, actions: &Actions) -> Element {
  let path = match browser {
    AgentsBrowser::Found(program) | AgentsBrowser::Chosen(program) => Some(program.path().to_path_buf()),
    AgentsBrowser::ChosenUnusable(reason) => Some(refused_path(reason).to_path_buf()),
    AgentsBrowser::NotFound(_) => None,
  };
  let label = if path.is_some() {
    "settings.browser.program.chooseAnother"
  } else {
    "settings.browser.program.choose"
  };
  let button = ActionButton::new("settings-browser-choose", ButtonKind::Secondary, ctx.t(label)).icon(Icon::FolderOpen);
  let button = if actions.chosen.is_pending() {
    button.loading()
  } else {
    let choose = actions.choose.clone();
    button.on_click(move || choose.run(()))
  };
  let row = SettingRow::new("settings-browser-program", ctx.t("settings.browser.program.label"))
    .hint(ctx.t("settings.browser.program.hint"))
    .control(button)
    .below_all(program_problems(ctx, browser, actions.chosen));
  let row = match path {
    Some(path) => {
      row.detail(mono_value("settings-browser-program-path", &path.display().to_string()).width(Dimension::Pct(100.0)))
    }
    None => row,
  };
  row.render(ctx)
}

/// Under *Program*: why the chosen program cannot be used, why the last one chosen was not taken, or why it was not
/// saved.
fn program_problems(ctx: &Ctx, browser: &AgentsBrowser, chosen: &FutureState<Choice, Message>) -> Vec<Element> {
  let unusable = match browser {
    AgentsBrowser::ChosenUnusable(reason) => Some(ctx.t_args(
      "settings.browser.found.unusableDetail",
      [("reason", reason_words(ctx, reason))],
    )),
    _ => None,
  };
  let refused = match (&chosen.data, &chosen.error) {
    (_, Some(error)) => Some(ctx.t_args(
      "settings.browser.program.notSaved",
      [("reason", message_text(ctx, error))],
    )),
    (Some(Choice::Refused(reason)), None) => Some(ctx.t_args(
      "settings.browser.program.refused",
      [("reason", reason_words(ctx, reason))],
    )),
    _ => None,
  };
  [
    unusable.map(|words| ("settings-browser-unusable", words)),
    refused.map(|words| ("settings-browser-refused", words)),
  ]
  .into_iter()
  .flatten()
  .map(|(id, words)| field_error(id, &words).into())
  .collect()
}

/// Why `reason`'s program is no Chromium-based browser's, in a sentence.
fn reason_words(ctx: &Ctx, reason: &NotABrowser) -> String {
  let key = match reason {
    NotABrowser::Missing(_) => "settings.browser.reason.missing",
    NotABrowser::Folder(_) => "settings.browser.reason.folder",
    NotABrowser::NotChromium(_) => "settings.browser.reason.notChromium",
    NotABrowser::NotUnicode(_) => "settings.browser.reason.notUnicode",
  };
  let path = refused_path(reason).display().to_string();
  ctx.t_args(key, [("path", path.as_str())]).to_string()
}

fn refused_path(reason: &NotABrowser) -> &Path {
  match reason {
    NotABrowser::Missing(path)
    | NotABrowser::Folder(path)
    | NotABrowser::NotChromium(path)
    | NotABrowser::NotUnicode(path) => path,
  }
}

/// *Browser profiles*: their folder with Copy, or that it is unknown.
fn profiles_row(ctx: &mut Ctx, profiles: Option<&Path>) -> Element {
  let row = SettingRow::new("settings-browser-profiles", ctx.t("settings.browser.profiles.label"))
    .hint(ctx.t("settings.browser.profiles.hint"));
  let row = match profiles {
    Some(folder) => {
      let shown = folder.display().to_string();
      let copied = shown.clone();
      let copy = CopiedValue::new(
        "settings-browser-profiles-path",
        shown,
        ctx.t("common.copy"),
        move || {
          let done = copy_to_clipboard(&copied);
          if !done {
            tracing::warn!("the browser profiles' folder could not be put on the clipboard");
          }
          done
        },
      )
      .render(ctx);
      row.control(copy)
    }
    None => row.toned_hint(ctx.t("settings.browser.profiles.unknown"), HintTone::Danger),
  };
  row.render(ctx)
}
