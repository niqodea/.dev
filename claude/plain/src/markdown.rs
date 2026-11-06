//! Colors Markdown a line at a time, the way an editor highlights a Markdown file: every
//! character stays, so the text copies back as the Markdown it was. A line's colors depend only
//! on the lines before it, so each line is colored once, when it ends.

use crate::code;
use crate::table::Table;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
// For the marks that structure the text, which are not part of what it says
const DIM: &str = "\x1b[2m";
const CODE: &str = "\x1b[36m";

/// Colors one block of Markdown, a line at a time.
pub struct Highlighter {
    code_block: Option<CodeBlock>,
    /// The rows of a table, held until the table ends, since its widest cell sets each column's
    /// width.
    table: Vec<String>,
}

/// A code block, colored in its language when a grammar knows the language.
enum CodeBlock {
    Plain,
    Language(code::Highlighter),
}

impl Highlighter {
    pub fn new() -> Self {
        Highlighter {
            code_block: None,
            table: Vec::new(),
        }
    }

    /// Colors the next line, given without its line ending, and returns the lines it completes:
    /// none while it holds a table's row, and the table's lines once a line ends the table. A
    /// table is laid out only if it fits in `columns`, the terminal's width when it is known.
    pub fn color_line(&mut self, line: &str, columns: Option<usize>) -> Vec<String> {
        if self.code_block.is_none() && line.trim_start().starts_with('|') {
            self.table.push(line.to_string());
            return vec![];
        }
        let mut lines = self.take_table(columns);
        lines.push(self.color_other(line));
        lines
    }

    /// Returns the lines still held, since the block ended, and starts afresh for the next one.
    pub fn finish(&mut self, columns: Option<usize>) -> Vec<String> {
        let lines = self.take_table(columns);
        *self = Highlighter::new();
        lines
    }

    fn color_other(&mut self, line: &str) -> String {
        if line.trim_start().starts_with("```") {
            self.code_block = if self.code_block.is_some() {
                None
            } else {
                let language = line
                    .trim()
                    .trim_start_matches('`')
                    .split_whitespace()
                    .next();
                match language.and_then(code::Highlighter::new) {
                    Some(highlighter) => Some(CodeBlock::Language(highlighter)),
                    None => Some(CodeBlock::Plain),
                }
            };
            // The whole fence line, language name included, is a mark
            return format!("{DIM}{line}{RESET}");
        }
        match &mut self.code_block {
            Some(CodeBlock::Language(highlighter)) => highlighter.color_line(line),
            Some(CodeBlock::Plain) => format!("{CODE}{line}{RESET}"),
            None => color_prose(line),
        }
    }

    /// The table rows held, laid out when they make a table that fits in `columns`, and as they
    /// were written otherwise.
    fn take_table(&mut self, columns: Option<usize>) -> Vec<String> {
        let lines = std::mem::take(&mut self.table);
        match (Table::read(&lines), columns) {
            (Some(table), Some(columns)) if table.width() <= columns => table.render(color_inline),
            _ => lines,
        }
    }
}

/// Colors a line of prose: a heading in bold, the mark that starts a list item or a quote dim,
/// and the inline marks of the rest.
fn color_prose(line: &str) -> String {
    let rest = line.trim_start_matches(' ');
    let indent = line.len() - rest.len();
    let hashes = rest.len() - rest.trim_start_matches('#').len();
    if hashes > 0 && rest[hashes..].starts_with(' ') {
        return format!("{BOLD}{line}{RESET}");
    }
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let mark_length = match rest[digits..].chars().next() {
        Some('-' | '*' | '+' | '>') if digits == 0 => 1,
        Some('.' | ')') if digits > 0 => digits + 1,
        _ => 0,
    };
    // A mark is followed by a space, so `**bold**` or `-1` start no list item
    if mark_length == 0 || !rest[mark_length..].starts_with(' ') {
        return color_inline(line);
    }
    let (mark, text) = line.split_at(indent + mark_length);
    format!("{DIM}{mark}{RESET}{}", color_inline(text))
}

/// Colors the inline marks of text: code spans in the code color, and bold in bold. The marks
/// stay, colored with what they mark.
fn color_inline(text: &str) -> String {
    let mut colored = String::new();
    let mut is_code_span = false;
    let mut is_bold = false;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '`' if is_code_span => {
                colored.push('`');
                is_code_span = false;
                colored.push_str(RESET);
                if is_bold {
                    colored.push_str(BOLD);
                }
            }
            '`' => {
                is_code_span = true;
                colored.push_str(CODE);
                colored.push('`');
            }
            '*' if !is_code_span && characters.peek() == Some(&'*') => {
                characters.next();
                if is_bold {
                    colored.push_str("**");
                    colored.push_str(RESET);
                } else {
                    colored.push_str(BOLD);
                    colored.push_str("**");
                }
                is_bold = !is_bold;
            }
            character => colored.push(character),
        }
    }
    if is_code_span || is_bold {
        colored.push_str(RESET);
    }
    colored
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(lines: &[&str]) -> Vec<String> {
        let mut highlighter = Highlighter::new();
        let mut colored: Vec<String> = lines
            .iter()
            .flat_map(|line| highlighter.color_line(line, Some(80)))
            .collect();
        colored.extend(highlighter.finish(Some(80)));
        colored
    }

    #[test]
    fn plain_text_stays_as_it_is() {
        assert_eq!(color(&["Hello, world."]), ["Hello, world."]);
    }

    #[test]
    fn bold_keeps_its_marks_and_turns_bold() {
        assert_eq!(color(&["a **b** c"]), [format!("a {BOLD}**b**{RESET} c")]);
    }

    #[test]
    fn inline_code_turns_code_colored() {
        assert_eq!(
            color(&["run `ls` now"]),
            [format!("run {CODE}`ls`{RESET} now")]
        );
    }

    #[test]
    fn a_heading_is_bold() {
        assert_eq!(
            color(&["## Title", "Text"]),
            [format!("{BOLD}## Title{RESET}"), "Text".to_string()]
        );
    }

    #[test]
    fn a_list_mark_is_dim() {
        assert_eq!(color(&["- item"]), [format!("{DIM}-{RESET} item")]);
        assert_eq!(color(&["  12. item"]), [format!("{DIM}  12.{RESET} item")]);
    }

    #[test]
    fn a_line_that_starts_with_bold_is_no_list_item() {
        assert_eq!(color(&["**b** c"]), [format!("{BOLD}**b**{RESET} c")]);
    }

    #[test]
    fn a_code_block_without_a_language_is_code_colored_and_ignores_marks() {
        assert_eq!(
            color(&["```", "# not a **heading**", "```"]),
            [
                format!("{DIM}```{RESET}"),
                format!("{CODE}# not a **heading**{RESET}"),
                format!("{DIM}```{RESET}"),
            ]
        );
    }

    #[test]
    fn a_code_block_in_a_known_language_is_colored_by_its_grammar() {
        let mut python = code::Highlighter::new("python").expect("syntect knows Python");
        assert_eq!(
            color(&["```python", "import os", "```"]),
            [
                format!("{DIM}```python{RESET}"),
                python.color_line("import os"),
                format!("{DIM}```{RESET}"),
            ]
        );
    }

    #[test]
    fn a_table_is_laid_out_once_it_ends() {
        let rows = ["| a | b |", "|---|---|", "| long cell | x |"];
        let lines: Vec<String> = rows.iter().map(ToString::to_string).collect();
        let mut expected = vec!["Before".to_string()];
        expected.extend(Table::read(&lines).expect("a table").render(color_inline));
        expected.push("After".to_string());
        assert_eq!(
            color(&["Before", rows[0], rows[1], rows[2], "After"]),
            expected
        );
    }

    #[test]
    fn a_table_that_ends_the_block_is_laid_out_when_the_block_ends() {
        let rows = ["| a |", "|---|", "| x |"];
        let lines: Vec<String> = rows.iter().map(ToString::to_string).collect();
        let laid_out = Table::read(&lines).expect("a table").render(color_inline);
        assert_eq!(color(&rows), laid_out);
    }

    #[test]
    fn rows_that_make_no_table_stay_as_written() {
        assert_eq!(
            color(&["| not a table |", "Text"]),
            ["| not a table |", "Text"]
        );
    }

    #[test]
    fn a_table_wider_than_the_terminal_stays_as_written() {
        let row = format!("| {} |", "x".repeat(100));
        assert_eq!(color(&[&row, "|---|"]), [row.as_str(), "|---|"]);
    }

    #[test]
    fn a_lone_star_stays_plain() {
        assert_eq!(color(&["2 * 3 = 6"]), ["2 * 3 = 6"]);
    }
}
