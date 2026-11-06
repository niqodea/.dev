//! Colors the lines of a code block in its language. A line's colors depend only on the lines
//! before it, so each line is colored once, when it ends.

use std::sync::OnceLock;

use syntect::easy::ScopeRegionIterator;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxSet};

const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const MAGENTA: &str = "\x1b[35m";
const GREY: &str = "\x1b[90m";

/// The color of each kind of token, in the terminal's own colors. A token takes the color of
/// the first kind in the list that it is.
const COLORS: [(&str, &str); 13] = [
    ("comment", GREY),
    ("markup.inserted", GREEN),
    ("markup.deleted", RED),
    // Before string, since a key is a string too
    ("meta.structure.dictionary.key", BLUE),
    ("entity.name.tag", BLUE),
    ("string", GREEN),
    ("constant", YELLOW),
    ("keyword", MAGENTA),
    ("storage", MAGENTA),
    ("entity.name.function", BLUE),
    ("support.function", BLUE),
    ("entity.name", YELLOW),
    ("support.type", YELLOW),
];

/// The grammars of the languages Sublime Text knows, which take a moment to load, so they load
/// once, when a code block first names a language.
static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();

/// Colors one code block of a known language, a line at a time.
pub struct Highlighter {
    parse_state: ParseState,
    scopes: ScopeStack,
    colors: Vec<(Scope, &'static str)>,
}

impl Highlighter {
    /// A highlighter for `language`, the name a code block's fence gives it, like `python` or
    /// `py`. None when no grammar knows the name.
    pub fn new(language: &str) -> Option<Self> {
        let syntax = load_syntaxes().find_syntax_by_token(language)?;
        let colors = COLORS
            .iter()
            .map(|(kind, color)| {
                let scope = Scope::new(kind).expect("the kinds are valid scope names");
                (scope, *color)
            })
            .collect();
        Some(Highlighter {
            parse_state: ParseState::new(syntax),
            scopes: ScopeStack::new(),
            colors,
        })
    }

    /// Colors the next line of the block, given without its line ending. A grammar that fails
    /// on a line leaves it uncolored, which loses nothing of the line.
    pub fn color_line(&mut self, line: &str) -> String {
        // The grammars read lines with their line ending, which some of their rules match
        let ended = format!("{line}\n");
        let Ok(operations) = self.parse_state.parse_line(&ended, load_syntaxes()) else {
            return line.to_string();
        };
        let mut colored = String::new();
        let mut current = "";
        for (text, operation) in ScopeRegionIterator::new(&operations, &ended) {
            if self.scopes.apply(operation).is_err() {
                return line.to_string();
            }
            let text = text.trim_end_matches('\n');
            if text.is_empty() {
                continue;
            }
            let color = self
                .colors
                .iter()
                .find(|(kind, _)| {
                    self.scopes
                        .as_slice()
                        .iter()
                        .any(|scope| kind.is_prefix_of(*scope))
                })
                .map_or("", |(_, color)| *color);
            if color != current {
                if !current.is_empty() {
                    colored.push_str(RESET);
                }
                colored.push_str(color);
                current = color;
            }
            colored.push_str(text);
        }
        if !current.is_empty() {
            colored.push_str(RESET);
        }
        colored
    }
}

fn load_syntaxes() -> &'static SyntaxSet {
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_python_line_colors_its_keyword_and_string() {
        let mut highlighter = Highlighter::new("python").expect("syntect knows Python");
        assert_eq!(
            highlighter.color_line("import os  # load"),
            format!("{MAGENTA}import{RESET} os  {GREY}# load{RESET}")
        );
    }

    #[test]
    fn a_diff_colors_added_and_removed_lines() {
        let mut highlighter = Highlighter::new("diff").expect("syntect knows diff");
        assert_eq!(highlighter.color_line("-old"), format!("{RED}-old{RESET}"));
        assert_eq!(
            highlighter.color_line("+new"),
            format!("{GREEN}+new{RESET}")
        );
    }

    #[test]
    fn a_json_key_is_colored_apart_from_its_string_value() {
        let mut highlighter = Highlighter::new("json").expect("syntect knows JSON");
        highlighter.color_line("{");
        assert_eq!(
            highlighter.color_line("  \"name\": \"Ada\""),
            format!("  {BLUE}\"name\"{RESET}: {GREEN}\"Ada\"{RESET}")
        );
    }

    #[test]
    fn a_yaml_key_is_colored_apart_from_its_string_value() {
        let mut highlighter = Highlighter::new("yaml").expect("syntect knows YAML");
        assert_eq!(
            highlighter.color_line("name: \"Ada\""),
            format!("{BLUE}name{RESET}: {GREEN}\"Ada\"{RESET}")
        );
    }

    #[test]
    fn an_unknown_language_has_no_highlighter() {
        assert!(Highlighter::new("no-such-language").is_none());
    }
}
