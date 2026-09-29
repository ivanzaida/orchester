use std::path::PathBuf;

use orchester_core::run::{BuildTargetState, PreflightProblem, build_space_problem};

const GIB: u64 = 1024 * 1024 * 1024;

#[test]
fn build_space_empty_target_requires_ten_gib_with_exact_boundary() {
  let target = PathBuf::from("/build/target");
  assert_eq!(
    build_space_problem(target.clone(), 10 * GIB - 1, BuildTargetState::Empty),
    Some(PreflightProblem::BuildSpaceLow {
      target: target.clone(),
      available: 10 * GIB - 1,
      required: 10 * GIB,
    })
  );
  assert_eq!(build_space_problem(target, 10 * GIB, BuildTargetState::Empty), None);
}

#[test]
fn build_space_populated_target_uses_one_gib_emergency_floor() {
  let target = PathBuf::from("/build/target");
  assert_eq!(
    build_space_problem(target.clone(), GIB - 1, BuildTargetState::Populated),
    Some(PreflightProblem::BuildSpaceLow {
      target: target.clone(),
      available: GIB - 1,
      required: GIB,
    })
  );
  assert_eq!(
    build_space_problem(target.clone(), GIB, BuildTargetState::Populated),
    None
  );
  assert_eq!(build_space_problem(target, 5 * GIB, BuildTargetState::Populated), None);
}
