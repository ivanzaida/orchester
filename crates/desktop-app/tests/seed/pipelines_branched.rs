//! The `pipelines-branched` scenario, which reaches `Pipelines · v2` P16 `T9KJFP` and P16b `P1853`: Feature delivery
//! whose step 06 waits for 04 only. `cargo test -p orchester-app --test seed pipelines_branched`.
use orchester_core::{
  resource::{ResourceKind, StepKey},
  store::ResourceStore,
};
use orchester_workspace::pipelines::{
  data::OpenedPipeline,
  flow::{Named, PipelineFlow, Waits},
};

use super::Seed;

#[test]
fn seed_pipelines_branched_draws_feature_delivery_with_a_step_that_waits_for_part_of_the_group() {
  let seed = Seed::of("pipelines-branched");
  let db = seed.db();
  let rows = db.resources(ResourceKind::Pipeline).unwrap();
  let id = rows
    .iter()
    .find(|row| row.name().as_str() == "Feature delivery")
    .unwrap()
    .id();
  let opened = OpenedPipeline::load_in(seed.repo(), id).unwrap();
  let flow = PipelineFlow::of(&opened.body, &opened.assignables).expect("the branched flow is drawn");
  let sizes: Vec<usize> = flow.stages.iter().map(|stage| stage.cards.len()).collect();
  assert_eq!(sizes, [1, 1, 1, 2, 1], "06 stands under the parallel group");
  let last = &flow.stages[4].cards[0];
  assert_eq!(last.key, StepKey::new("create-pull-request").unwrap());
  assert_eq!(last.number, 6);
  assert_eq!(
    last.waits,
    Some(Waits {
      on: vec![Named {
        number: 4,
        name: "Run checks".into()
      }],
      not_on: vec![Named {
        number: 5,
        name: "Review".into()
      }],
    })
  );
  assert!(flow.deliver.is_some(), "07 Deliver waits for everything");
}
