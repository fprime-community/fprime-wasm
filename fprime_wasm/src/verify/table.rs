//! Fixed-width tables, for [`super::render`].
//!
//! Deliberately ASCII: `fprime-wasm` ships a Windows binary, and a legacy console
//! there will mangle box-drawing characters. A rule of `-` renders the same
//! everywhere.
//!
//! Columns size themselves to their contents, and each has an alignment — numbers
//! right, names and prose left — because a column of right-aligned figures is the
//! whole reason to draw a table rather than print labelled lines.

use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

/// A table under construction.
pub struct Table {
    headers: Vec<String>,
    aligns: Vec<Align>,
    rows: Vec<Vec<String>>,
    indent: usize,
}

impl Table {
    /// A table whose columns are `(header, alignment)`, indented by `indent`
    /// spaces so it can sit under a section heading.
    pub fn new(columns: &[(&str, Align)], indent: usize) -> Self {
        Table {
            headers: columns.iter().map(|(name, _)| (*name).to_owned()).collect(),
            aligns: columns.iter().map(|(_, align)| *align).collect(),
            rows: Vec::new(),
            indent,
        }
    }

    /// Add a row. A row with the wrong number of cells is a programming error, so
    /// it is asserted rather than padded — a silently short row would misalign
    /// every column after it.
    pub fn row<I, S>(&mut self, cells: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let cells: Vec<String> = cells.into_iter().map(Into::into).collect();
        assert_eq!(
            cells.len(),
            self.headers.len(),
            "row has {} cells but the table has {} columns",
            cells.len(),
            self.headers.len()
        );
        self.rows.push(cells);
        self
    }

    /// Column widths: the widest of the header and every cell.
    ///
    /// Measured in `char`s rather than bytes so a non-ASCII value — a dictionary
    /// name, a trap message — does not overpad the column by its UTF-8 length.
    /// (Still wrong for double-width characters, which none of these fields hold.)
    fn widths(&self) -> Vec<usize> {
        let mut widths: Vec<usize> = self.headers.iter().map(|h| h.chars().count()).collect();
        for row in &self.rows {
            for (index, cell) in row.iter().enumerate() {
                widths[index] = widths[index].max(cell.chars().count());
            }
        }
        widths
    }

    /// Render to text, with a trailing newline on every line.
    ///
    /// Trailing whitespace is trimmed per line: padding the last column would
    /// leave invisible spaces that show up in diffs and in `git`'s
    /// whitespace warnings for anyone committing captured output.
    pub fn render(&self) -> String {
        if self.rows.is_empty() {
            return String::new();
        }
        let widths = self.widths();
        let pad = " ".repeat(self.indent);
        let mut out = String::new();

        let mut line = |cells: &[String]| {
            let mut text = pad.clone();
            for (index, cell) in cells.iter().enumerate() {
                if index > 0 {
                    text.push_str("  ");
                }
                let width = widths[index];
                let visible = cell.chars().count();
                let padding = " ".repeat(width.saturating_sub(visible));
                match self.aligns[index] {
                    Align::Left => {
                        text.push_str(cell);
                        text.push_str(&padding);
                    }
                    Align::Right => {
                        text.push_str(&padding);
                        text.push_str(cell);
                    }
                }
            }
            let _ = writeln!(out, "{}", text.trim_end());
        };

        line(&self.headers);
        let rule: Vec<String> = widths.iter().map(|width| "-".repeat(*width)).collect();
        line(&rule);
        for row in &self.rows {
            line(row);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        let mut table = Table::new(&[("Name", Align::Left), ("Size", Align::Right)], 2);
        table.row(["baseline", "50"]);
        table.row(["cmd_string_large", "1500"]);
        table
    }

    #[test]
    fn sizes_columns_to_the_widest_cell_and_aligns_them() {
        assert_eq!(
            table().render(),
            "  Name              Size\n\
             \x20 ----------------  ----\n\
             \x20 baseline            50\n\
             \x20 cmd_string_large  1500\n"
        );
    }

    /// A header wider than any cell still has to set the column width, or the rule
    /// and the header disagree.
    #[test]
    fn a_wide_header_sets_the_column_width() {
        let mut table = Table::new(&[("Configured", Align::Right)], 0);
        table.row(["8"]);
        assert_eq!(table.render(), "Configured\n----------\n         8\n");
    }

    /// Captured output gets committed and diffed; invisible padding at the end of
    /// a line is noise in both.
    #[test]
    fn no_line_has_trailing_whitespace() {
        let mut table = Table::new(&[("A", Align::Right), ("Note", Align::Left)], 2);
        table.row(["1", "short"]);
        table.row(["22", ""]);
        for line in table.render().lines() {
            assert_eq!(line, line.trim_end(), "trailing space in {line:?}");
        }
    }

    /// A header and a rule with nothing under them would be worse than silence.
    #[test]
    fn an_empty_table_renders_nothing() {
        assert_eq!(Table::new(&[("A", Align::Left)], 2).render(), "");
    }

    /// Column widths are measured in characters. A byte count would overpad any
    /// cell holding a multi-byte character.
    #[test]
    fn width_counts_characters_not_bytes() {
        let mut table = Table::new(&[("N", Align::Left), ("X", Align::Left)], 0);
        // Four chars, six bytes.
        table.row(["a→b←", "end"]);
        let first = table.render().lines().nth(2).expect("a row").to_string();
        // Width 4, so no padding, then the two-space gutter. Measured by bytes the
        // cell would be 6 wide and this would gain two more spaces.
        assert_eq!(first, "a→b←  end");
    }

    #[test]
    #[should_panic(expected = "row has 1 cells but the table has 2 columns")]
    fn a_row_with_the_wrong_cell_count_is_a_bug() {
        Table::new(&[("A", Align::Left), ("B", Align::Left)], 0).row(["only one"]);
    }
}
