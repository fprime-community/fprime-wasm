//! Terminal output.
//!
//! [`summary`] is one row per module. [`detail`] expands one module into [`Block`]s
//! joined by blank lines:
//!
//! ```text
//! <path>                                    head
//!   641 bytes: 355 code, 96 data.
//!
//!   Setting          Needed  ...             budgets, incl. PAGE_SIZE
//!   guestMemorySize     941  ...
//!
//!   code fill     299 of 512 16-bit words    figures the budget table cannot hold
//!   heap          37 allocations, ...
//! ```

use super::measure::Verified;
use super::table::{Align, Table};
use fprime_test::interpreter::{Limits, Validated};

/// Column the field values in a [`Block::Fields`] line up at.
const LABEL: usize = 14;

/// One piece of [`detail`]'s output, ending in a newline.
enum Block {
    /// Verbatim lines, indented by the caller.
    Lines(Vec<String>),
    /// `label`, padded to [`LABEL`], then its value.
    Fields(Vec<(&'static str, String)>),
    /// A table, under a caption where it has one.
    Table(Option<String>, Table),
}

impl Block {
    fn render(&self) -> String {
        match self {
            Block::Lines(lines) => lines.iter().map(|line| format!("{line}\n")).collect(),
            Block::Fields(fields) => fields
                .iter()
                .map(|(label, value)| format!("  {label:<width$}{value}\n", width = LABEL))
                .collect(),
            Block::Table(None, table) => table.render(),
            Block::Table(Some(caption), table) => format!("  {caption}\n{}", table.render()),
        }
    }
}

fn joined(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(Block::render)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The configuration every module was loaded under. `stackSize` is here because it is part
/// of the configuration, not because loading measured it.
pub fn limits_line(limits: &Limits, source: &str) -> String {
    format!(
        "Limits ({source}): memory {} B, heap {}, code {}, operand stack {}, page {} B",
        limits.guest_memory,
        plural(u64::from(limits.heap_pages), "page"),
        plural(limits.max_code_pages as u64, "page"),
        plural(limits.stack_size as u64, "word"),
        limits.page_size
    )
}

/// The default report, and the only view that compares modules.
pub fn summary(checked: &[Verified]) -> String {
    let mut table = Table::new(
        &[
            ("Module", Align::Left),
            ("Bytes", Align::Right),
            ("Memory", Align::Right),
            ("Heap", Align::Right),
            ("Code", Align::Right),
            ("Fits", Align::Left),
        ],
        2,
    );

    for one in checked {
        let needed = |setting: &str| {
            one.budgets
                .iter()
                .find(|budget| budget.setting == setting)
                .map_or_else(|| "-".to_string(), |budget| budget.needed.to_string())
        };
        table.row([
            stem(one),
            one.loaded.sizes.total.to_string(),
            needed("guestMemorySize"),
            needed("heapPages"),
            needed("maxCodePages"),
            verdict(one.passed()).to_string(),
        ]);
    }
    table.render()
}

/// One module in full, for `--verbose`.
pub fn detail(checked: &Verified) -> String {
    joined(&[head(checked), budgets(checked), figures(&checked.loaded)])
}

fn head(checked: &Verified) -> Block {
    let sizes = &checked.loaded.sizes;
    Block::Lines(vec![
        checked.path.display().to_string(),
        format!(
            "  {} bytes: {} code, {} data.",
            sizes.total, sizes.code, sizes.data
        ),
    ])
}

/// Every `Config` field loading can size, plus `PAGE_SIZE`.
fn budgets(checked: &Verified) -> Block {
    let mut table = Table::new(
        &[
            ("Setting", Align::Left),
            ("Needed", Align::Right),
            ("Configured", Align::Right),
            ("Unit", Align::Left),
            ("Used", Align::Right),
            ("Fits", Align::Left),
        ],
        2,
    );
    for budget in &checked.budgets {
        table.row([
            budget.setting.to_string(),
            budget.needed.to_string(),
            budget.configured.to_string(),
            budget.unit.to_string(),
            percentage(budget.needed, budget.configured),
            verdict(budget.fits()).to_string(),
        ]);
    }
    let (needed, configured) = (checked.required_page_size, checked.configured_page_size);
    table.row([
        "PAGE_SIZE".to_string(),
        needed.to_string(),
        configured.to_string(),
        "bytes".to_string(),
        percentage(needed as u64, configured as u64),
        verdict(needed <= configured).to_string(),
    ]);
    Block::Table(None, table)
}

fn figures(loaded: &Validated) -> Block {
    let usage = &loaded.usage;
    let mut fields = vec![
        (
            "code fill",
            format!(
                "{} of {} 16-bit words",
                loaded.cost.code_words, loaded.cost.code_capacity
            ),
        ),
        (
            "heap",
            format!(
                "{}, {} B peak, {} B padding, {} B resident",
                plural(usage.allocations, "allocation"),
                usage.peak_live,
                usage.padding,
                usage.resident
            ),
        ),
    ];
    if usage.oversize > 0 {
        fields.push((
            "oversize",
            format!(
                "{} exceeded one page; each is a hard failure on board",
                plural(usage.oversize, "allocation")
            ),
        ));
    }
    Block::Fields(fields)
}

/// Truncated to the file stem.
fn stem(checked: &Verified) -> String {
    checked
        .path
        .file_stem()
        .unwrap_or(checked.path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// `1 page` / `2 pages`; nouns only, not verb phrases.
fn plural(count: u64, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

fn percentage(needed: u64, configured: u64) -> String {
    if configured == 0 {
        return "-".to_string();
    }
    format!("{:.0}%", 100.0 * needed as f64 / configured as f64)
}

fn verdict(fits: bool) -> &'static str {
    if fits { "ok" } else { "OVER" }
}

#[cfg(test)]
mod tests {
    use super::super::fixture;
    use super::*;
    use fprime_test::interpreter::Usage;

    #[test]
    fn blocks_join_with_blank_line() {
        let mut table = Table::new(&[("Id", Align::Right)], 2);
        table.row(["7"]);
        let rendered = joined(&[
            Block::Lines(vec!["head".into()]),
            Block::Fields(vec![("code fill", "1 of 2".into())]),
            Block::Table(Some("1 id".into()), table),
        ]);
        assert_eq!(
            rendered,
            "head\n\
             \n\
             \x20 code fill     1 of 2\n\
             \n\
             \x20 1 id\n\
             \x20 Id\n\
             \x20 --\n\
             \x20  7\n"
        );
    }

    #[test]
    fn figures_report_what_loading_measured() {
        assert_eq!(
            figures(&fixture::validated()).render(),
            "  code fill     299 of 512 16-bit words\n\
             \x20 heap          37 allocations, 9942 B peak, 4 B padding, 9000 B resident\n"
        );
    }

    /// An allocation larger than a page is a hard failure on board, so it is called out
    /// rather than left to the `PAGE_SIZE` row alone.
    #[test]
    fn oversize_allocation_called_out() {
        let loaded = Validated {
            usage: Usage {
                allocations: 1,
                oversize: 1,
                ..fixture::validated().usage
            },
            ..fixture::validated()
        };
        let rendered = figures(&loaded).render();
        assert!(
            rendered.contains("oversize      1 allocation "),
            "{rendered}"
        );
        // Singular, and the noun must not read `1 allocations`.
        assert!(
            rendered.contains("heap          1 allocation,"),
            "{rendered}"
        );
    }

    #[test]
    fn summary_has_one_row_per_module() {
        let rendered = summary(&[fixture::nominal("safing"), fixture::nominal("startup")]);
        let lines: Vec<&str> = rendered.lines().collect();
        // Header, rule, two rows.
        assert_eq!(lines.len(), 4, "{rendered}");
        assert!(lines[0].contains("Module"), "{rendered}");
        assert!(lines[2].contains("safing"), "{rendered}");
        assert!(lines[3].contains("startup"), "{rendered}");
        // No outcome column: nothing ran.
        assert!(!rendered.contains("Status"), "{rendered}");
    }

    /// Limits are stated once by [`limits_line`], so a row that repeated one would be
    /// noise in every report.
    #[test]
    fn summary_omits_limits_stated_once() {
        let rendered = summary(&[fixture::nominal("narrow")]);
        assert!(rendered.contains("941"), "{rendered}");
        for row in rendered.lines().skip(2) {
            assert!(
                !row.contains("8192"),
                "the row restates a limit already in the header:\n{row}"
            );
        }
    }

    #[test]
    fn detail_shows_every_budget_and_the_page_size() {
        let rendered = detail(&fixture::nominal("safing"));
        for setting in ["guestMemorySize", "heapPages", "maxCodePages", "PAGE_SIZE"] {
            assert!(rendered.contains(setting), "{setting} missing:\n{rendered}");
        }
        // Only running measures these; claiming otherwise would be a lie.
        assert!(!rendered.contains("stackSize"), "{rendered}");
        assert!(!rendered.contains("guest stack"), "{rendered}");
    }

    #[test]
    fn a_module_over_budget_is_marked_over() {
        let tight = Limits {
            max_code_pages: 1,
            ..Limits::default()
        };
        let one = fixture::verified("big", fixture::validated(), &tight);
        assert!(!one.passed());
        assert!(summary(&[one]).contains("OVER"));
    }

    #[test]
    fn limits_line_names_its_source() {
        let line = limits_line(&Limits::default(), "sequencer.toml");
        assert!(line.starts_with("Limits (sequencer.toml):"), "{line}");
        assert!(line.contains("memory 8192 B"), "{line}");
    }

    #[test]
    fn percentage_guards_zero_budget() {
        assert_eq!(percentage(941, 8192), "11%");
        assert_eq!(percentage(8192, 8192), "100%");
        assert_eq!(percentage(0, 8192), "0%");
        // Nothing configured: a ratio would divide by zero.
        assert_eq!(percentage(1, 0), "-");
    }

    #[test]
    fn verdict_is_ok_or_over() {
        assert_eq!(verdict(true), "ok");
        assert_eq!(verdict(false), "OVER");
    }
}
