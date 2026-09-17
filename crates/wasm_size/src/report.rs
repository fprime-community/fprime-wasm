//! The size report and the comparison between two of them.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;

use crate::wasm::Sections;

/// One measured wasm module.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Binary {
    pub name: String,
    pub total: usize,
    pub code: usize,
    pub data: usize,
    /// The bin's source, relative to the workspace root, for linking the table.
    ///
    /// Defaulted rather than required because CI compares against a measurement
    /// taken by the *base* revision's own copy of this tool, which predates the
    /// field. A missing source costs a link, not the comparison.
    #[serde(default)]
    pub source: Option<String>,
}

/// A whole run's measurements, ordered by name so the JSON is diffable.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Report {
    pub binaries: Vec<Binary>,
    /// Whether these sizes were measured after `wasm-opt`, where known.
    ///
    /// `None` for a measurement written by a tool old enough not to record it,
    /// which is what CI's base side can be — and is deliberately not the same as
    /// `Some(false)`. Unlike [`Binary::source`] this changes what the numbers
    /// *mean* rather than how they look, so a comparison across a real mismatch
    /// says so rather than letting the reader credit the optimiser's win to the
    /// change. Guessing `false` here would produce that warning on comparisons
    /// that are in fact like for like.
    #[serde(default)]
    pub optimized: Option<bool>,
}

impl Report {
    pub fn new(mut binaries: Vec<Binary>, optimized: bool) -> Self {
        binaries.sort_by(|a, b| a.name.cmp(&b.name));

        Self {
            binaries,
            optimized: Some(optimized),
        }
    }

    /// The measured module named `name`, if it was measured.
    fn get(&self, name: &str) -> Option<&Binary> {
        self.binaries.iter().find(|binary| binary.name == name)
    }

    /// Absolute sizes, smallest first, so the fixed cost is the top row and the
    /// table reads as a ladder.
    pub fn markdown(&self) -> String {
        let mut rows = self.binaries.clone();
        rows.sort_by_key(|binary| binary.total);

        let mut out = String::from("| Binary | Total | Code | Data |\n|---|--:|--:|--:|\n");
        for binary in &rows {
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} |",
                binary.name, binary.total, binary.code, binary.data
            );
        }

        out
    }
}

/// The three numbers measured for one module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sizes {
    pub total: usize,
    pub code: usize,
    pub data: usize,
}

/// One row of a comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub name: String,
    /// `None` when the binary is new or was removed.
    pub base: Option<Sizes>,
    pub head: Option<Sizes>,
    /// Preferred from the head measurement: a bin that moved between crates
    /// should link to where it lives now, not where it used to.
    pub source: Option<String>,
}

impl Change {
    /// Positive means the binary grew.
    pub fn delta(&self) -> i64 {
        self.head.map_or(0, |sizes| sizes.total as i64)
            - self.base.map_or(0, |sizes| sizes.total as i64)
    }

    /// Whether anything about this binary moved.
    ///
    /// Every section is compared, not just the total: const-encoding a command
    /// moves its payload out of code and into data, which leaves the total
    /// identical and is exactly the shift this report exists to show. Folding
    /// such a row away as "unchanged" would hide the finding.
    pub fn moved(&self) -> bool {
        self.base != self.head
    }
}

/// Compare two reports, joined on binary name.
///
/// Sorted by delta descending, so a regression is the first thing read. Binaries
/// present in only one side are kept: a benchmark that stopped building is
/// exactly the thing a size report should not hide.
pub fn compare(base: &Report, head: &Report) -> Vec<Change> {
    let mut names: BTreeMap<&str, ()> = BTreeMap::new();
    for binary in base.binaries.iter().chain(&head.binaries) {
        names.insert(binary.name.as_str(), ());
    }

    let mut changes: Vec<Change> = names
        .into_keys()
        .map(|name| Change {
            name: name.to_string(),
            base: base.get(name).map(Binary::sizes),
            head: head.get(name).map(Binary::sizes),
            source: head
                .get(name)
                .or_else(|| base.get(name))
                .and_then(|binary| binary.source.clone()),
        })
        .collect();

    // Descending by delta, then by name so equal rows are stable.
    changes.sort_by(|a, b| b.delta().cmp(&a.delta()).then(a.name.cmp(&b.name)));
    changes
}

fn signed(delta: i64) -> String {
    match delta {
        0 => "0".to_string(),
        delta => format!("{delta:+}"),
    }
}

/// How to render a comparison.
#[derive(Debug, Clone, Copy, Default)]
pub struct Style<'a> {
    /// Names the revision the base measurement came from, so a moved cell reads
    /// `main → HEAD` rather than something anonymous.
    pub base_label: &'a str,
    /// Prefix that turns a [`Binary::source`] path into a URL, such as
    /// `https://github.com/owner/repo/blob/<sha>`. Absolute rather than relative
    /// because the table is posted as a pull request comment, and it should keep
    /// pointing at the revision it describes once the branch has moved on.
    /// Without it, names render as plain code spans.
    pub source_base: Option<&'a str>,
}

/// One section's cell: the plain size when it held still, `base → head (Δ)` when
/// it moved, so a reader scanning the table sees only the numbers that did.
fn cell(base: Option<usize>, head: Option<usize>) -> String {
    match (base, head) {
        (Some(base), Some(head)) if base == head => head.to_string(),
        (Some(base), Some(head)) => {
            format!("{base} → {head} ({})", signed(head as i64 - base as i64))
        }
        (None, Some(head)) => format!("— → {head}"),
        (Some(base), None) => format!("{base} → —"),
        (None, None) => "—".to_string(),
    }
}

/// The total carries the percentage as well, since that is the number a size
/// regression gets judged on.
fn total_cell(change: &Change) -> String {
    let base = change.base.map(|sizes| sizes.total);
    let head = change.head.map(|sizes| sizes.total);

    match (base, head) {
        (None, Some(head)) => format!("— → {head} (new)"),
        (Some(base), None) => format!("{base} → — (removed)"),
        (Some(base), Some(head)) if base != head && base > 0 => format!(
            "{base} → {head} ({}, {:+.1}%)",
            signed(change.delta()),
            100.0 * change.delta() as f64 / base as f64
        ),
        _ => cell(base, head),
    }
}

fn table(changes: &[&Change], style: &Style) -> String {
    let mut out = String::from("| Binary | Total | Code | Data |\n|---|--:|--:|--:|\n");

    for change in changes {
        let name = match (style.source_base, &change.source) {
            (Some(base), Some(source)) => format!("[`{}`]({base}/{source})", change.name),
            _ => format!("`{}`", change.name),
        };

        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            name,
            total_cell(change),
            cell(
                change.base.map(|sizes| sizes.code),
                change.head.map(|sizes| sizes.code)
            ),
            cell(
                change.base.map(|sizes| sizes.data),
                change.head.map(|sizes| sizes.data)
            ),
        );
    }

    out
}

/// Render a comparison as markdown.
///
/// Two tables: what moved, then everything else behind a `<details>`. Every
/// measured binary appears in one of them, because the report is read as the
/// current absolute size of each benchmark as much as it is read as a diff — but
/// forty untouched rows should not be what a reviewer has to scroll past first.
pub fn markdown(changes: &[Change], style: &Style) -> String {
    let (moved, held): (Vec<&Change>, Vec<&Change>) =
        changes.iter().partition(|change| change.moved());

    let base_total: usize = changes.iter().filter_map(|c| c.base).map(|s| s.total).sum();
    let head_total: usize = changes.iter().filter_map(|c| c.head).map(|s| s.total).sum();
    let total_delta = head_total as i64 - base_total as i64;

    let mut out = String::new();

    let _ = writeln!(
        out,
        "**Total** {} → {} bytes ({}){}\n",
        base_total,
        head_total,
        signed(total_delta),
        match total_delta {
            0 => ", no change".to_string(),
            _ => format!(", {:+.2}%", 100.0 * total_delta as f64 / base_total as f64),
        }
    );

    match moved.is_empty() {
        true => {
            let _ = writeln!(out, "No binary changed size.");
        }
        false => {
            let _ = writeln!(
                out,
                "A cell that moved reads `{}` → `HEAD`.\n",
                style.base_label
            );
            out.push_str(&table(&moved, style));
        }
    }

    if !held.is_empty() {
        let _ = writeln!(
            out,
            "\n<details>\n<summary>{} unchanged</summary>\n\n{}\n</details>",
            held.len(),
            table(&held, style)
        );
    }

    out
}

impl Binary {
    pub fn from_sections(
        name: impl Into<String>,
        sections: Sections,
        source: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            total: sections.total,
            code: sections.code,
            data: sections.data,
            source,
        }
    }

    fn sizes(&self) -> Sizes {
        Sizes {
            total: self.total,
            code: self.code,
            data: self.data,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    fn binary(name: &str, total: usize) -> Binary {
        Binary {
            name: name.to_string(),
            total,
            code: total / 2,
            data: total / 4,
            source: Some(format!("crates/bench/src/bin/{name}.rs")),
        }
    }

    /// A binary whose sections are set independently of its total, for the cases
    /// where the point is that the sections and the total disagree.
    fn sectioned(name: &str, total: usize, code: usize, data: usize) -> Binary {
        Binary {
            name: name.to_string(),
            total,
            code,
            data,
            source: None,
        }
    }

    fn style() -> Style<'static> {
        Style {
            base_label: "BASE",
            source_base: None,
        }
    }

    #[test]
    fn orders_a_report_by_name() {
        let report = Report::new(vec![binary("b", 2), binary("a", 1)], true);

        assert_eq!(
            report
                .binaries
                .iter()
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
    }

    #[test]
    fn puts_the_worst_regression_first() {
        let base = Report::new(vec![binary("small", 100), binary("big", 100)], true);
        let head = Report::new(vec![binary("small", 105), binary("big", 200)], true);

        let changes = compare(&base, &head);

        assert_eq!(changes[0].name, "big");
        assert_eq!(changes[0].delta(), 100);
        assert_eq!(changes[1].delta(), 5);
    }

    #[test]
    fn reports_a_shrink_as_negative() {
        let base = Report::new(vec![binary("a", 200)], true);
        let head = Report::new(vec![binary("a", 150)], true);

        assert_eq!(compare(&base, &head)[0].delta(), -50);
    }

    /// A benchmark that stopped building must not silently vanish from the table.
    #[test]
    fn keeps_added_and_removed_binaries() {
        let base = Report::new(vec![binary("gone", 100)], true);
        let head = Report::new(vec![binary("fresh", 100)], true);

        let changes = compare(&base, &head);
        let table = markdown(&changes, &style());

        assert!(table.contains("`gone`"), "{table}");
        assert!(table.contains("removed"), "{table}");
        assert!(table.contains("`fresh`"), "{table}");
        assert!(table.contains("new"), "{table}");
    }

    /// A row that moved shows every section as `base → head`, and the total
    /// carries the percentage.
    #[test]
    fn spells_out_each_section_of_a_changed_row() {
        let base = Report::new(vec![sectioned("moved", 100, 60, 20)], true);
        let head = Report::new(vec![sectioned("moved", 120, 75, 25)], true);

        let table = markdown(&compare(&base, &head), &style());

        assert!(
            table.contains("| `moved` | 100 → 120 (+20, +20.0%) | 60 → 75 (+15) | 20 → 25 (+5) |"),
            "{table}"
        );
    }

    /// The table is the absolute size of every benchmark as much as it is a
    /// diff, so a row that did not move still has to be there — folded away.
    #[test]
    fn folds_unchanged_rows_into_details() {
        let base = Report::new(vec![binary("same", 100), binary("moved", 100)], true);
        let head = Report::new(vec![binary("same", 100), binary("moved", 120)], true);

        let table = markdown(&compare(&base, &head), &style());
        let (top, details) = table
            .split_once("<details>")
            .expect("the unchanged row needs somewhere to hide");

        assert!(top.contains("`moved`"), "{table}");
        assert!(!top.contains("`same`"), "{table}");
        assert!(
            details.contains("<summary>1 unchanged</summary>"),
            "{table}"
        );
        assert!(details.contains("| `same` | 100 | 50 | 25 |"), "{table}");
    }

    /// Const-encoding a command moves bytes from code into data and leaves the
    /// total alone. That is a finding, not an unchanged row.
    #[test]
    fn treats_a_section_shift_as_a_change() {
        let base = Report::new(vec![sectioned("shifted", 100, 80, 0)], true);
        let head = Report::new(vec![sectioned("shifted", 100, 60, 20)], true);

        let table = markdown(&compare(&base, &head), &style());

        assert!(!table.contains("<details>"), "{table}");
        assert!(
            table.contains("| `shifted` | 100 | 80 → 60 (-20) | 0 → 20 (+20) |"),
            "{table}"
        );
    }

    #[test]
    fn says_so_when_nothing_moved() {
        let report = Report::new(vec![binary("a", 100), binary("b", 200)], true);
        let table = markdown(&compare(&report, &report), &style());

        assert!(table.contains("no change"), "{table}");
        assert!(table.contains("No binary changed size."), "{table}");
        assert!(table.contains("<summary>2 unchanged</summary>"), "{table}");
        assert!(table.contains("| `a` | 100 | 50 | 25 |"), "{table}");
    }

    #[test]
    fn labels_the_base_side_of_a_moved_cell() {
        let base = Report::new(vec![binary("a", 100)], true);
        let head = Report::new(vec![binary("a", 120)], true);
        let style = Style {
            base_label: "main",
            source_base: None,
        };

        assert!(
            markdown(&compare(&base, &head), &style)
                .contains("A cell that moved reads `main` → `HEAD`."),
            "the label should reach the reader"
        );
    }

    #[test]
    fn links_a_name_to_its_source() {
        let report = Report::new(vec![binary("mixed_max", 100)], true);
        let style = Style {
            base_label: "BASE",
            source_base: Some("https://github.com/o/r/blob/abc"),
        };

        let table = markdown(&compare(&report, &report), &style);

        assert!(
            table.contains(
                "[`mixed_max`](https://github.com/o/r/blob/abc/crates/bench/src/bin/mixed_max.rs)"
            ),
            "{table}"
        );
    }

    /// Without somewhere to point, the name is still a name.
    #[test]
    fn leaves_a_name_plain_with_no_source_base() {
        let report = Report::new(vec![binary("a", 100)], true);
        let table = markdown(&compare(&report, &report), &style());

        assert!(table.contains("| `a` |"), "{table}");
        assert!(!table.contains("]("), "{table}");
    }

    #[test]
    fn round_trips_through_json() {
        let report = Report::new(vec![binary("a", 100), binary("b", 200)], true);
        let json = serde_json::to_string(&report).expect("should serialize");

        assert_eq!(
            serde_json::from_str::<Report>(&json).expect("should deserialize"),
            report
        );
    }

    /// CI compares against a measurement written by the base revision's own copy
    /// of this tool, which has neither `source` nor `optimized`. That must still
    /// parse, and must not claim to have been optimised.
    #[test]
    fn reads_a_measurement_from_an_older_revision() {
        let json = r#"{"binaries":[{"name":"a","total":100,"code":50,"data":25}]}"#;
        let report: Report = serde_json::from_str(json).expect("should deserialize");

        assert_eq!(report.binaries[0].source, None);
        assert_eq!(report.optimized, None, "unknown, not known-unoptimised");
    }
}
