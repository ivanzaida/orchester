//! *Send error reports* (`Send error reports` `tMyiy` in `01b · General · error reports on` `Z322E`, off in `01c`
//! `mWn0J`): whether the app and its runners send their errors and crashes to Orchester's error tracker
//! (`crate::error_reports`), on until a person turns it off. The switch shows the stored setting, read through its
//! query, which the window read as the session started; a flip takes effect at once for the app's client and the
//! running runners (`error_reports::turn`), then saves off the UI thread, and the header's save state follows it; a
//! failed save puts the stored setting back. *What a report carries* opens `docs/error-reports.md`
//! (`error_reports::WHAT_A_REPORT_CARRIES`).

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, FutureState},
    theme::{SpacingSize, TypographyStyle},
  },
  components::Text,
  core::Signal,
  layout::Alignment,
  node::Element,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
};

use crate::{
  error_reports,
  queries::{self, Session, app},
  screens::settings::save_state::{Outcome, SaveState},
  services::Services,
  theme::{ExtraColor, IconSize},
  ui::{
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    pressable::{Focus, pressable},
    setting_rows::SettingRow,
    skeleton::{self, Skeleton},
    toggle::{Toggle, ToggleKind},
  },
};

const ID: &str = "settings-error-reports";

type SaveAction = FutureAction<bool, bool, Message>;

pub struct ErrorReportsRow {
  /// The setting as the person last chose it; `None` until they do.
  chosen: Signal<Option<bool>>,
  /// *What a report carries* could not be opened.
  unopened: Signal<bool>,
}

impl Component for ErrorReportsRow {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      chosen: ctx.signal(None),
      unopened: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let services = Services::of(ctx);
    // Hooks in the same order on every render: the action, then the read, which each save invalidates.
    let save = save_action(ctx, &services);
    let state = save.state().get();
    let read = queries::state(&ctx.query(app::error_reports(Session(services.clone()))));
    self.restore(&state, read.data, &services);
    // The person's choice took effect at once and shows until the stored value is it; the stored value until they
    // choose, and again once a save failed (which says so under the row).
    queries::settle(&self.chosen, read.data.as_ref(), state.is_pending());
    let shown = self.chosen.get().or(read.data);
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(ID, Outcome::of(&state));
    }
    let control: Element = match shown {
      None if read.is_pending() => Skeleton::new(format!("{ID}-reading"), ctx.t("settings.reading"))
        .inline()
        .show(ctx, skeleton::switch()),
      _ => {
        let label = ctx.t("settings.general.privacy.errorReports.label");
        let switch = Toggle::new(ID, ToggleKind::Switch, shown.unwrap_or(false), label).hide_label();
        // Until the stored value is known, and while it is read or saved, the switch stays as it is.
        if shown.is_some() && !state.is_pending() && !read.is_pending() {
          self.listen(switch, save, services).into()
        } else {
          switch.into()
        }
      }
    };
    let problem = problem(ctx, (&read.error, &state.error));
    let unopened = self.unopened.get().then(|| {
      let words = ctx.t_args(
        "settings.general.privacy.errorReports.notOpened",
        [("url", error_reports::WHAT_A_REPORT_CARRIES)],
      );
      field_error(&format!("{ID}-not-opened"), &words).into()
    });
    SettingRow::new(ID, ctx.t("settings.general.privacy.errorReports.label"))
      .hint(ctx.t("settings.general.privacy.errorReports.hint"))
      .link(self.link(ctx))
      .control(control)
      .below_all(problem.into_iter().chain(unopened))
      .render(ctx)
  }
}

impl ErrorReportsRow {
  /// `switch`, turning the reports as the person chooses at once, then saving the choice.
  fn listen(&self, switch: Toggle, save: SaveAction, services: Services) -> Toggle {
    let chosen = self.chosen.clone();
    switch.on_change(move |on| {
      error_reports::turn(on, &services.runners);
      chosen.set(Some(on));
      save.run(on);
    })
  }

  /// Once a save failed, puts back what is stored: the switch shows it and the reports follow it again.
  fn restore(&self, state: &FutureState<bool, Message>, stored: Option<bool>, services: &Services) {
    if !state.is_rejected() || self.chosen.get_untracked().is_none() {
      return;
    }
    self.chosen.set(None);
    if let Some(stored) = stored {
      error_reports::turn(stored, &services.runners);
    }
  }

  /// *What a report carries ↗* in `accent_text` 12/500 (`Link` `ngNFb`), opening the page in the browser.
  fn link(&self, ctx: &mut Ctx) -> Element {
    let words = ctx.t("settings.general.privacy.errorReports.link");
    let (links, unopened) = (Services::of(ctx).links, self.unopened.clone());
    let open = move || match links.open(error_reports::WHAT_A_REPORT_CARRIES) {
      Ok(()) => unopened.set(false),
      Err(error) => {
        tracing::warn!(%error, "what an error report carries could not be opened");
        unopened.set(true);
      }
    };
    Focus::Link
      .apply(pressable(open))
      .id(format!("{ID}-link"))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .describe("a11y_name", words.to_string())
      .child(
        Text::new(&words)
          .variant(TypographyStyle::Label)
          .color(ExtraColor::AccentText.role())
          .nowrap(),
      )
      .child(icon(Icon::ArrowUpRight, IconSize::Sm, ExtraColor::AccentText.role()))
      .into()
  }
}

/// Why the setting cannot be read or was not saved, under the row.
fn problem(ctx: &Ctx, (read, save): (&Option<Message>, &Option<Message>)) -> Option<Element> {
  let (key, reason) = match (read, save) {
    (Some(reason), _) => ("settings.general.privacy.errorReports.unreadable", reason),
    (None, Some(reason)) => ("settings.general.privacy.errorReports.notSaved", reason),
    (None, None) => return None,
  };
  let reason = message_text(ctx, reason);
  Some(field_error(&format!("{ID}-error"), &ctx.t_args(key, [("reason", &*reason)])).into())
}

/// Saves the person's choice; the workspace counts the change, which reads the setting again (`queries`).
fn save_action(ctx: &mut Ctx, services: &Services) -> SaveAction {
  let workspace = services.workspace.clone();
  ctx.future_action(move |on: bool| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.set_sends_error_reports(on))
        .await?
        .map(|()| on)
        .map_err(|error| {
          tracing::error!(%error, "whether to send error reports was not saved");
          messages::store_error(&error)
        })
    }
  })
}
