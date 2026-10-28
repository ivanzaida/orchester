use orchester_core::workspaces::{DiskUsage, DiskVerdict, GIGABYTE, KeepFree, KeepFreeError};

#[test]
fn disk_guard_keeps_150_gb_free_by_default() {
  assert_eq!(KeepFree::default().gigabytes(), 150);
  assert_eq!(KeepFree::default().bytes(), 150 * GIGABYTE);
}

#[test]
fn disk_guard_is_low_below_what_it_keeps_free() {
  let keep = KeepFree::from_gigabytes(150).expect("150 GB");
  let low = DiskUsage {
    free: 92 * GIGABYTE,
    total: 1800 * GIGABYTE,
  };
  assert_eq!(
    keep.verdict(low),
    DiskVerdict::Low {
      free: 92 * GIGABYTE,
      needed: 150 * GIGABYTE
    }
  );
  let room = DiskUsage {
    free: 150 * GIGABYTE,
    total: 1800 * GIGABYTE,
  };
  assert_eq!(keep.verdict(room), DiskVerdict::Room);
}

#[test]
fn disk_guard_refuses_zero() {
  assert_eq!(KeepFree::from_gigabytes(0), Err(KeepFreeError { gigabytes: 0 }));
}
