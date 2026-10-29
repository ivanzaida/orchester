//! Test reports read into one result shape (`orchester_core::test_report`), from real-format fixtures in
//! `tests/fixtures/test_reports/`.

mod broken;
mod capped;
mod formats;

use orchester_core::test_report::{ReportFormat, TestCase, TestReport, read_report};

const CARGO: &str = include_str!("../fixtures/test_reports/cargo.json");
const CARGO_ABORTED: &str = include_str!("../fixtures/test_reports/cargo_aborted.json");
const NODE_JUNIT: &str = include_str!("../fixtures/test_reports/node.junit.xml");
const PYTEST_JUNIT: &str = include_str!("../fixtures/test_reports/pytest.junit.xml");
const VITEST: &str = include_str!("../fixtures/test_reports/vitest.json");
const PLAYWRIGHT: &str = include_str!("../fixtures/test_reports/playwright.json");
const NODE_TAP: &str = include_str!("../fixtures/test_reports/node.tap");

fn read(format: ReportFormat, text: &str) -> TestReport {
  read_report(format, text).unwrap_or_else(|error| panic!("the {format} report reads: {error}"))
}

fn test<'a>(report: &'a TestReport, name: &str) -> &'a TestCase {
  report
    .tests()
    .iter()
    .find(|test| test.name() == name)
    .unwrap_or_else(|| panic!("the report has a test named {name}"))
}
