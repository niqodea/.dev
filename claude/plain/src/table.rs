//! A Markdown table, laid out once all its rows arrived, with each column as wide as its widest
//! cell. The layout is still Markdown for the same table, so the table copies back as one.

use unicode_width::UnicodeWidthStr;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
// For the marks that structure the table, which are not part of what it says
const DIM: &str = "\x1b[2m";
// The narrowest divider cell Markdown reads as one
const MIN_COLUMN_WIDTH: usize = 3;

pub struct Table {
    header: Vec<String>,
    alignments: Vec<Alignment>,
    rows: Vec<Vec<String>>,
    widths: Vec<usize>,
}

#[derive(Clone, Copy)]
enum Alignment {
    Left,
    Center,
    Right,
}

impl Table {
    /// Reads `lines` as a header row, a divider row and body rows that each have as many cells as
    /// the header. None for anything else, which is then shown as it was written.
    pub fn read(lines: &[String]) -> Option<Table> {
        let [header, divider, body @ ..] = lines else {
            return None;
        };
        let header = split_row(header)?;
        let alignments = split_row(divider)?
            .iter()
            .map(|cell| read_alignment(cell))
            .collect::<Option<Vec<_>>>()?;
        let rows = body
            .iter()
            .map(|line| split_row(line))
            .collect::<Option<Vec<_>>>()?;
        if alignments.len() != header.len() || rows.iter().any(|row| row.len() != header.len()) {
            return None;
        }
        let widths = (0..header.len())
            .map(|column| {
                std::iter::once(&header)
                    .chain(&rows)
                    .map(|row| row[column].width())
                    .fold(MIN_COLUMN_WIDTH, usize::max)
            })
            .collect();
        Some(Table {
            header,
            alignments,
            rows,
            widths,
        })
    }

    /// How many columns of the terminal the laid-out table takes.
    pub fn width(&self) -> usize {
        // Each cell has a space on both sides and a pipe after it, and the row starts with one
        self.widths.iter().map(|width| width + 3).sum::<usize>() + 1
    }

    /// The lines of the laid-out table, header in bold and the body's cells as `color_cell`
    /// colors them.
    pub fn render(&self, color_cell: impl Fn(&str) -> String) -> Vec<String> {
        let pipe = format!("{DIM}|{RESET}");
        let mut header = pipe.clone();
        for (column, cell) in self.header.iter().enumerate() {
            header.push_str(&self.pad(column, cell, &format!("{BOLD}{cell}{RESET}")));
            header.push_str(&pipe);
        }
        let mut divider = format!("{DIM}|");
        for (alignment, width) in self.alignments.iter().zip(&self.widths) {
            let dashes = match alignment {
                Alignment::Left => "-".repeat(*width),
                Alignment::Center => format!(":{}:", "-".repeat(width - 2)),
                Alignment::Right => format!("{}:", "-".repeat(width - 1)),
            };
            divider.push(' ');
            divider.push_str(&dashes);
            divider.push_str(" |");
        }
        divider.push_str(RESET);
        let mut lines = vec![header, divider];
        for row in &self.rows {
            let mut line = pipe.clone();
            for (column, cell) in row.iter().enumerate() {
                line.push_str(&self.pad(column, cell, &color_cell(cell)));
                line.push_str(&pipe);
            }
            lines.push(line);
        }
        lines
    }

    /// The cell's `text`, colored, with the spaces that align `cell` in its column around it.
    fn pad(&self, column: usize, cell: &str, text: &str) -> String {
        let space = self.widths[column] - cell.width();
        let (before, after) = match self.alignments[column] {
            Alignment::Left => (0, space),
            Alignment::Center => (space / 2, space - space / 2),
            Alignment::Right => (space, 0),
        };
        format!(" {}{text}{} ", " ".repeat(before), " ".repeat(after))
    }
}

/// The cells of a row, which starts with a pipe and may end with one. A pipe escaped with a
/// backslash, or inside a code span, is text rather than the end of a cell.
fn split_row(line: &str) -> Option<Vec<String>> {
    let inside = line.trim().strip_prefix('|')?;
    let mut cells = vec![String::new()];
    let mut is_code_span = false;
    let mut characters = inside.chars().peekable();
    while let Some(character) = characters.next() {
        let cell = cells.last_mut().expect("the row starts with a cell");
        match character {
            '\\' if characters.peek() == Some(&'|') => {
                cell.push('\\');
                cell.push(characters.next().expect("the pipe was peeked"));
            }
            '`' => {
                is_code_span = !is_code_span;
                cell.push('`');
            }
            '|' if !is_code_span => cells.push(String::new()),
            character => cell.push(character),
        }
    }
    // The pipe that ends a row leaves nothing after it
    if cells.last().is_some_and(String::is_empty) {
        cells.pop();
    }
    Some(cells.iter().map(|cell| cell.trim().to_string()).collect())
}

/// The alignment a divider cell like `---`, `:--:` or `--:` sets for its column.
fn read_alignment(cell: &str) -> Option<Alignment> {
    let dashes = cell.trim_start_matches(':').trim_end_matches(':');
    if dashes.is_empty() || !dashes.chars().all(|character| character == '-') {
        return None;
    }
    match (cell.starts_with(':'), cell.ends_with(':')) {
        (true, true) => Some(Alignment::Center),
        (false, true) => Some(Alignment::Right),
        (_, false) => Some(Alignment::Left),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(lines: &[&str]) -> Option<Table> {
        let lines: Vec<String> = lines.iter().map(ToString::to_string).collect();
        Table::read(&lines)
    }

    fn render_plain(table: &Table) -> String {
        let colored = table.render(ToString::to_string).join("\n");
        colored
            .replace(RESET, "")
            .replace(BOLD, "")
            .replace(DIM, "")
    }

    #[test]
    fn columns_take_the_width_of_their_widest_cell() {
        let table = read(&[
            "| Name | Age |",
            "|---|---|",
            "| Ada | 36 |",
            "| Grace | 85 |",
        ])
        .expect("a table");
        assert_eq!(
            render_plain(&table),
            "| Name  | Age |\n| ----- | --- |\n| Ada   | 36  |\n| Grace | 85  |"
        );
    }

    #[test]
    fn a_divider_aligns_its_column() {
        let table = read(&["| a | b |", "|:-:|--:|", "| x | 10000 |"]).expect("a table");
        assert_eq!(
            render_plain(&table),
            "|  a  |     b |\n| :-: | ----: |\n|  x  | 10000 |"
        );
    }

    #[test]
    fn a_pipe_in_code_or_escaped_stays_in_its_cell() {
        let table = read(&["| a | b |", "|---|---|", "| `x | y` | 1 \\| 2 |"]).expect("a table");
        assert_eq!(table.rows, vec![vec!["`x | y`", "1 \\| 2"]]);
    }

    #[test]
    fn a_wide_character_counts_as_two_columns() {
        let table = read(&["| a |", "|---|", "| 日本 |"]).expect("a table");
        assert_eq!(render_plain(&table), "| a    |\n| ---- |\n| 日本 |");
    }

    #[test]
    fn a_row_with_another_number_of_cells_is_no_table() {
        assert!(read(&["| a | b |", "|---|---|", "| x |"]).is_none());
    }

    #[test]
    fn rows_without_a_divider_are_no_table() {
        assert!(read(&["| a | b |", "| x | y |"]).is_none());
    }

    #[test]
    fn a_lone_row_is_no_table() {
        assert!(read(&["| a | b |"]).is_none());
    }
}
