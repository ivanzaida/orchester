//! Every screen's skeleton while its data is read (`Loading states · work screens` `xed4a`, `Loading states · configure
//! screens` `psFv5`), on the QA seed's project with its reads held (`hold_reads`): the skeleton is there from the first
//! frame but shows no bone before `SKELETON_DELAY`, then its bones; real content takes their place once the reads are
//! let go, in the same room, and a section that reads inside a shown page leaves the rest of the page as it is.

mod areas;
mod harness;
mod in_place;
#[allow(
  dead_code,
  reason = "the seed's report and its database path are the QA tool's; these tests read its project"
)]
#[path = "../../examples/seed/kontur/mod.rs"]
mod kontur;
#[path = "../support/mod.rs"]
mod support;
