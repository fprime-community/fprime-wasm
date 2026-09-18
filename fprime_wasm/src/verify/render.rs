//! Terminal output.
//!
//! [`summary`] is one row per module. [`detail`] expands one module into [`Block`]s
//! joined by blank lines:
//!
//! ```text
//! <path>                                    head
//!   641 bytes: 355 code, ... returned.
//!
//!   Setting          Needed  ...             budgets, incl. PAGE_SIZE
//!   guestMemorySize     941  ...
//!
//!   guest stack   none declared; ...         figures the budget table cannot hold
//!   code fill     299 of 512 16-bit words
//!
//!   19 commands                              commands
//!   Opcode      Command      ...
//!
//!   1 telemetry channels read                ids, once per kind
//!         Id  Name           ...
//!
//!   sleeps 1500 us (0.002 s) ...             sleeps
//!
//!   20 host calls                            calls, with --trace only
//!    #  Host call
//! ```
//!
//! [`issues`] is separate: it groups one line per distinct message across all
//! modules, since a warning usually fires on many of them at once.

use super::describe::{
    channel_name, command_name, describe_call, describe_outcome, outcome_word, parameter_name,
};
use super::measure::Verified;
use super::table::{Align, Table};
use crate::harness::{Kind, Limits, Report};
use fprime_dictionary::Dictionary;

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

pub fn limits_line(limits: &Limits) -> String {
    format!(
        "Limits: memory {} B, heap {}, code {}, operand stack {}, page {} B",
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
            ("Stack", Align::Right),
            ("Heap", Align::Right),
            ("Code", Align::Right),
            ("Operand", Align::Right),
            ("Status", Align::Left),
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
            one.sizes.total.to_string(),
            needed("guestMemorySize"),
            match one.report.guest_stack {
                Some(stack) => stack.used.to_string(),
                // Not `0`: declaring no stack pointer is a different fact from having
                // one and not touching it.
                None => "-".into(),
            },
            needed("heapPages"),
            needed("maxCodePages"),
            needed("stackSize"),
            status(one).to_string(),
        ]);
    }
    table.render()
}

/// One module in full, for `--verbose`. Only what [`summary`] had no room for:
/// restating it as prose is what made the old report unreadable.
pub fn detail(checked: &Verified, dictionary: Option<&Dictionary>, trace: bool) -> String {
    let report = &checked.report;
    let recording = &report.recording;

    let mut blocks = vec![head(checked), budgets(checked), figures(report)];
    blocks.extend(commands(report, dictionary));
    blocks.extend(ids(
        "telemetry channels read",
        &recording.telemetry_read(),
        |id| channel_name(dictionary, id),
    ));
    blocks.extend(ids("parameters read", &recording.parameters_read(), |id| {
        parameter_name(dictionary, id)
    }));
    blocks.extend(sleeps(report));
    blocks.extend(trace.then(|| calls(report, dictionary)));
    joined(&blocks)
}

fn head(checked: &Verified) -> Block {
    let (sizes, report) = (&checked.sizes, &checked.report);
    Block::Lines(vec![
        checked.path.display().to_string(),
        format!(
            "  {} bytes: {} code, {} data. {}, {}.",
            sizes.total,
            sizes.code,
            sizes.data,
            plural(report.instructions, "instruction"),
            describe_outcome(&report.outcome)
        ),
    ])
}

/// Every `Config` field, then `PAGE_SIZE` — not one of them, but the same kind of
/// limit: a page bounds the largest single allocation, so exceeding it fails however
/// many pages are configured.
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

fn figures(report: &Report) -> Block {
    let usage = &report.usage;
    let mut fields = vec![(
        "guest stack",
        match report.guest_stack {
            Some(stack) => format!(
                "{} of {} bytes used, {} spare",
                stack.used,
                stack.reserved,
                stack.headroom()
            ),
            None => "none declared; the module never spills to linear memory".to_string(),
        },
    )];
    if report.guest_memory != report.declared_memory {
        fields.push((
            "guest memory",
            format!(
                "grew from {} to {} bytes",
                report.declared_memory, report.guest_memory
            ),
        ));
    }
    if report.refused_grows > 0 {
        fields.push((
            "memory.grow",
            format!(
                "{} refused for want of pool space; the guest saw -1 and continued",
                report.refused_grows
            ),
        ));
    }
    fields.push((
        "code fill",
        format!(
            "{} of {} 16-bit words",
            report.code_words, report.code_capacity
        ),
    ));
    fields.push((
        "heap",
        format!(
            "{}, {} B peak, {} B padding, {} B resident",
            plural(usage.allocations, "allocation"),
            usage.peak_live,
            usage.padding,
            usage.resident
        ),
    ));
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

fn commands(report: &Report, dictionary: Option<&Dictionary>) -> Option<Block> {
    let commands: Vec<_> = report.recording.commands().collect();
    if commands.is_empty() {
        return None;
    }
    let mut table = Table::new(
        &[
            ("Opcode", Align::Left),
            ("Command", Align::Left),
            ("Payload", Align::Right),
        ],
        2,
    );
    for (opcode, payload) in &commands {
        table.row([
            format!("{opcode:#010x}"),
            command_name(dictionary, *opcode).unwrap_or_else(|| "-".into()),
            payload.len().to_string(),
        ]);
    }
    let caption = plural(commands.len() as u64, "command");
    Some(Block::Table(Some(caption), table))
}

fn ids(caption: &str, ids: &[i64], name: impl Fn(i64) -> Option<String>) -> Option<Block> {
    if ids.is_empty() {
        return None;
    }
    let mut table = Table::new(&[("Id", Align::Right), ("Name", Align::Left)], 2);
    for id in ids {
        table.row([id.to_string(), name(*id).unwrap_or_else(|| "-".into())]);
    }
    Some(Block::Table(
        Some(format!("{} {caption}", ids.len())),
        table,
    ))
}

fn sleeps(report: &Report) -> Option<Block> {
    let us = report.recording.relative_sleep_us();
    (us > 0).then(|| {
        Block::Lines(vec![format!(
            "  sleeps {us} us ({:.3} s) of relative delay",
            us as f64 / 1e6
        )])
    })
}

fn calls(report: &Report, dictionary: Option<&Dictionary>) -> Block {
    let calls = &report.recording.calls;
    let mut table = Table::new(&[("#", Align::Right), ("Host call", Align::Left)], 2);
    for (index, call) in calls.iter().enumerate() {
        table.row([index.to_string(), describe_call(call, dictionary)]);
    }
    let caption = plural(calls.len() as u64, "host call");
    Block::Table(Some(caption), table)
}

/// `include_info` adds the zero-fill notes, which fire on nearly every run and would
/// otherwise be most of the output; [`info_count`] stands in for them.
pub fn issues(checked: &[Verified], include_info: bool) -> String {
    // Insertion-ordered, so the output reads in the order the run met them.
    let mut grouped: Vec<(&str, Vec<String>)> = Vec::new();
    for one in checked {
        let module = stem(one);
        for issue in &one.report.recording.issues {
            if issue.kind == Kind::Info && !include_info {
                continue;
            }
            match grouped
                .iter_mut()
                .find(|(message, _)| *message == issue.message)
            {
                Some((_, modules)) if modules.contains(&module) => {}
                Some((_, modules)) => modules.push(module.clone()),
                None => grouped.push((&issue.message, vec![module.clone()])),
            }
        }
    }
    grouped
        .iter()
        .map(|(message, modules)| format!("  {message}\n    in {}\n", listed(modules)))
        .collect()
}

/// Distinct informational notes, for the one line that stands in for them.
pub fn info_count(checked: &[Verified]) -> usize {
    let mut seen: Vec<&str> = Vec::new();
    for one in checked {
        for issue in one.report.recording.issues_of(Kind::Info) {
            if !seen.contains(&issue.message.as_str()) {
                seen.push(&issue.message);
            }
        }
    }
    seen.len()
}

/// Four names, then a count: a longer list wrapped over several lines reads worse.
fn listed(modules: &[String]) -> String {
    if modules.len() > 4 {
        format!("{} and {} more", modules[..4].join(", "), modules.len() - 4)
    } else {
        modules.join(", ")
    }
}

/// A full path down every row would crowd out the figures.
fn stem(checked: &Verified) -> String {
    checked
        .path
        .file_stem()
        .unwrap_or(checked.path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// `1 page` / `2 pages`. Simple nouns only: it cannot make a verb agree, and on
/// "channel or parameter" it would yield "channel or parameters".
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

/// How a module ended wins over whether it fits: a trapped sequence was not measured
/// to the end, so `ok` would mislead.
fn status(checked: &Verified) -> &'static str {
    if !checked.report.outcome.is_nominal() {
        return outcome_word(&checked.report.outcome);
    }
    if checked.passed() { "ok" } else { "OVER" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::{GuestStack, Outcome, Recording, Usage};

    /// A report that trips every conditional figure; fields are overridden per test.
    fn report() -> Report {
        Report {
            outcome: Outcome::Returned,
            usage: Usage {
                largest: 4096,
                peak_live: 9942,
                resident: 9000,
                allocations: 37,
                pages: 2,
                padding: 4,
                page_bytes: 8192,
                peak_pages: 2,
                oversize: 1,
            },
            code_pages: 2,
            code_words: 299,
            code_capacity: 512,
            instructions: 225,
            peak_operand_stack: 15,
            guest_memory: 1004,
            declared_memory: 941,
            refused_grows: 3,
            guest_stack: Some(GuestStack {
                reserved: 512,
                used: 328,
            }),
            recording: Recording::default(),
        }
    }

    /// Blocks are blank-line separated, fields line up at [`LABEL`], and a caption
    /// sits above its table at the same indent as the rows.
    #[test]
    fn blocks_join_with_a_blank_line_between() {
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

    /// The four conditional fields fire only on a module that grew, was refused, or
    /// allocated past a page — none of which the reference sequences do, so the shape
    /// of these lines is pinned here rather than by a run.
    #[test]
    fn every_figure_appears_in_order_when_it_applies() {
        assert_eq!(
            figures(&report()).render(),
            "  guest stack   328 of 512 bytes used, 184 spare\n\
             \x20 guest memory  grew from 941 to 1004 bytes\n\
             \x20 memory.grow   3 refused for want of pool space; the guest saw -1 and continued\n\
             \x20 code fill     299 of 512 16-bit words\n\
             \x20 heap          37 allocations, 9942 B peak, 4 B padding, 9000 B resident\n\
             \x20 oversize      1 allocation exceeded one page; each is a hard failure on board\n"
        );
    }

    /// A module that grew nothing and stayed inside its pages shows only the three
    /// figures that always apply.
    #[test]
    fn a_nominal_module_shows_only_the_unconditional_figures() {
        let report = Report {
            declared_memory: 1004,
            refused_grows: 0,
            guest_stack: None,
            usage: Usage {
                allocations: 1,
                oversize: 0,
                ..report().usage
            },
            ..report()
        };
        let rendered = figures(&report).render();
        assert_eq!(rendered.lines().count(), 3, "{rendered}");
        assert!(
            rendered.contains("guest stack   none declared"),
            "{rendered}"
        );
        // Singular, and the noun must not read `1 allocations`.
        assert!(
            rendered.contains("heap          1 allocation,"),
            "{rendered}"
        );
    }

    #[test]
    fn a_percentage_is_whole_and_guards_a_zero_budget() {
        assert_eq!(percentage(941, 8192), "11%");
        assert_eq!(percentage(8192, 8192), "100%");
        assert_eq!(percentage(0, 8192), "0%");
        // Nothing configured: a ratio would divide by zero.
        assert_eq!(percentage(1, 0), "-");
    }

    #[test]
    fn a_verdict_is_one_of_two_words() {
        assert_eq!(verdict(true), "ok");
        assert_eq!(verdict(false), "OVER");
    }

    #[test]
    fn a_long_module_list_is_summarised() {
        let names: Vec<String> = (0..6).map(|i| format!("m{i}")).collect();
        assert_eq!(listed(&names[..2]), "m0, m1");
        assert_eq!(listed(&names), "m0, m1, m2, m3 and 2 more");
    }
}
