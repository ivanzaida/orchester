//! The project's daily report (ORC-47, `orchester_workspace::orchestrator::report`): while the project is open, when
//! it opens and once a minute after, reads when the person wants the report (Settings › Notifications; off
//! until they turn it on) and delivers the report of each due day the project's conversations do not hold yet, also
//! one whose time passed while the app was closed. A day that had its report, or had no activity, is not read again
//! while the app runs. A delivered report that starts a turn by itself gets it from the waits' follower
//! (`super::waits`), which the conversations' change wakes. Mounted with the project's feed. Renders nothing.

use std::{
  collections::{HashMap, HashSet},
  path::PathBuf,
  sync::LazyLock,
  time::Duration,
};

use chrono::NaiveDate;
use lurq::app::{
  ctx::{Ctx, StreamEmitter},
  i18n::I18n,
};
use orchester_core::time::Timestamp;
use orchester_store::StoreError;
use orchester_workspace::{
  background,
  messages::{self, Message},
  orchestrator::report::{self, Delivery},
  workspace::{OpenProject, OrchestratorConsent},
};

use crate::services::Services;

/// How often an open project looks whether a report is due.
const EVERY: Duration = Duration::from_secs(60);

/// The days of each project, by its folder, that had their report or had no activity, as this run of the app found.
static SETTLED: LazyLock<parking_lot::Mutex<HashMap<PathBuf, HashSet<NaiveDate>>>> = LazyLock::new(Default::default);

/// Delivers `open`'s due reports while it is open.
pub(super) fn deliver(ctx: &mut Ctx, open: &OpenProject) {
  let services = Services::of(ctx);
  let i18n = ctx.i18n().clone();
  let open = open.clone();
  let _delivering = ctx.stream(open.repo_root.clone(), move |_, emitter: StreamEmitter<u64, ()>| {
    let (services, i18n, open) = (services.clone(), i18n.clone(), open.clone());
    async move {
      let mut delivered = 0;
      loop {
        match deliver_due(&services, &i18n, &open).await {
          Ok(count) => delivered += count,
          Err(error) => tracing::error!(?error, "cannot deliver the project's daily report"),
        }
        if !emitter.emit(delivered) {
          return;
        }
        tokio::time::sleep(EVERY).await;
      }
    }
  });
}

/// Delivers each report due now that this run of the app did not settle; how many it kept. The conversations are told
/// when it kept one.
async fn deliver_due(services: &Services, i18n: &I18n, open: &OpenProject) -> Result<u64, Message> {
  let (workspace, root) = (services.workspace.clone(), open.repo_root.clone());
  let read = background::run(move || {
    Ok::<_, StoreError>((
      workspace.daily_report_schedule()?,
      workspace.orchestrator_allows(&root, OrchestratorConsent::AnswerAgents)?,
    ))
  })
  .await?
  .map_err(|error| messages::store_error(&error))?;
  let (Some(schedule), answers) = read else {
    return Ok(0);
  };
  let now = Timestamp::now();
  let settled = SETTLED.lock().get(&open.repo_root).cloned().unwrap_or_default();
  let mut kept = 0;
  for day in report::due_days(schedule, now) {
    if settled.contains(&day) {
      continue;
    }
    let title = i18n
      .t_args(
        "orchestrator.report.conversation",
        [("day", day.format("%Y-%m-%d").to_string())],
      )
      .to_string();
    let (root, programs) = (open.repo_root.clone(), services.programs.clone());
    let delivery = {
      let _keeping = super::waits::KEEPING.lock().await;
      background::run(move || report::deliver_in((&root, &programs), answers, (day, now), &title)).await??
    };
    if delivery == Delivery::Kept {
      kept += 1;
    }
    SETTLED.lock().entry(open.repo_root.clone()).or_default().insert(day);
  }
  if kept > 0 {
    services.runners.chat_changed(&open.repo_root);
  }
  Ok(kept)
}
