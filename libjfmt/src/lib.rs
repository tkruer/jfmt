use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tree_sitter::{Language, Node, Parser};

#[derive(Debug, Error)]
pub enum LintError {
    #[error("failed to initialize Java language")] 
    Language,
    #[error("failed to parse source")]
    Parse,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid toml: {0}")]
    Toml(#[from] toml::de::Error),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IndentStyle {
    Tabs,
    Spaces,
}

impl Default for IndentStyle {
    fn default() -> Self { IndentStyle::Spaces }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub indent_style: IndentStyle, // tabs or spaces
    #[serde(default = "default_indent_width")]
    pub indent_width: u16,         // used when spaces
    #[serde(default = "default_max_line_length")]
    pub max_line_length: u16,      // line length budget
}

fn default_indent_width() -> u16 { 4 }
fn default_max_line_length() -> u16 { 100 }

impl Default for Config {
    fn default() -> Self {
        Self {
            indent_style: IndentStyle::Spaces,
            indent_width: 4,
            max_line_length: 100,
        }
    }
}

/// Find and load configuration by walking up from `start_dir` to root.
pub fn load_config_from(start_dir: impl AsRef<Path>) -> Result<Config, ConfigError> {
    let start = start_dir.as_ref();
    if let Some(p) = find_config_path(start) {
        let text = fs::read_to_string(&p)?;
        let cfg: Config = toml::from_str(&text)?;
        Ok(cfg)
    } else {
        Ok(Config::default())
    }
}

/// Load configuration starting at current directory.
pub fn load_config() -> Result<Config, ConfigError> {
    let cwd = std::env::current_dir()?;
    load_config_from(cwd)
}

fn find_config_path(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = Some(start_dir);
    while let Some(d) = dir {
        let candidate = d.join("jfmt.toml");
        if candidate.is_file() { return Some(candidate); }
        dir = d.parent();
    }
    None
}

#[derive(Debug, Clone)]
pub struct LintIssue {
    pub rule_id: &'static str,
    pub message: String,
    pub line: usize,   // 1-based
    pub column: usize, // 1-based
    pub fix: Option<Fix>,
}

#[derive(Debug, Clone)]
pub struct Fix {
    pub start_byte: usize,
    pub end_byte: usize,
    pub replacement: String,
}

fn java_language() -> Result<Language, LintError> {
    // SAFETY: Provided by tree-sitter-java crate
    let lang = tree_sitter_java::language();
    if lang.node_kind_count() == 0 {
        return Err(LintError::Language);
    }
    Ok(lang)
}

pub fn lint_java_source(source: &str, config: &Config) -> Result<Vec<LintIssue>, LintError> {
    let mut parser = Parser::new();
    parser.set_language(&java_language()?).map_err(|_| LintError::Language)?;

    let tree = parser.parse(source, None).ok_or(LintError::Parse)?;
    let root = tree.root_node();

    let mut issues = Vec::new();
    // Rule: no wildcard imports (import x.y.*;)
    collect_no_wildcard_imports(source, root, &mut issues);
    // Rule: no stray empty statements (;)
    collect_no_empty_statements(root, &mut issues);
    // Config-driven rules
    collect_line_length(source, config.max_line_length, &mut issues);
    collect_indent_style(source, config.indent_style, config.indent_width, &mut issues);

    Ok(issues)
}

fn issue_at(node: Node, rule_id: &'static str, message: impl Into<String>) -> LintIssue {
    let start = node.start_position();
    LintIssue {
        rule_id,
        message: message.into(),
        line: start.row + 1,
        column: start.column + 1,
        fix: None,
    }
}

fn collect_no_wildcard_imports(source: &str, root: Node, out: &mut Vec<LintIssue>) {
    let mut cursor = root.walk();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "import_declaration" {
            // Heuristic: check the import text for ".*;"
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                if text.contains(".*") {
                    out.push(issue_at(
                        node,
                        "no-wildcard-imports",
                        "Avoid wildcard imports (use explicit classes)",
                    ));
                }
            }
        }

        if node.child_count() > 0 {
            cursor.reset(node);
            for i in (0..node.child_count()).rev() {
                if let Some(child) = node.child(i) {
                    stack.push(child);
                }
            }
        }
    }
}

fn collect_no_empty_statements(root: Node, out: &mut Vec<LintIssue>) {
    let mut cursor = root.walk();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        // Java grammar exposes empty statements as "empty_statement".
        // Do not flag generic ';' tokens inside valid statements (e.g., package/import).
        let kind = node.kind();
        if kind == "empty_statement" {
            let mut issue = issue_at(
                node,
                "no-empty-statement",
                "Remove unnecessary empty statement",
            );
            issue.fix = Some(Fix {
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                replacement: String::new(),
            });
            out.push(issue);
        }
        if node.child_count() > 0 {
            cursor.reset(node);
            for i in (0..node.child_count()).rev() {
                if let Some(child) = node.child(i) {
                    stack.push(child);
                }
            }
        }
    }
}

fn collect_line_length(source: &str, max_len: u16, out: &mut Vec<LintIssue>) {
    let max_len = max_len as usize;
    for (idx, line) in source.lines().enumerate() {
        let visual_len = line.chars().count();
        if visual_len > max_len {
            out.push(LintIssue {
                rule_id: "max-line-length",
                message: format!("Line exceeds {} characters (was {})", max_len, visual_len),
                line: idx + 1,
                column: max_len + 1,
                fix: None,
            });
        }
    }
}

fn collect_indent_style(source: &str, style: IndentStyle, indent_width: u16, out: &mut Vec<LintIssue>) {
    let mut byte_pos = 0usize;
    for (idx, line_inc) in source.split_inclusive('\n').enumerate() {
        let line = line_inc.strip_suffix('\n').unwrap_or(line_inc);
        let start_byte = byte_pos;
        byte_pos += line_inc.as_bytes().len();

        let trimmed_end = line.trim_end();
        if trimmed_end.is_empty() { continue; }
        let leading_ws_len = trimmed_end
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        if leading_ws_len == 0 { continue; }
        let leading: String = trimmed_end.chars().take(leading_ws_len).collect();
        let has_space = leading.chars().any(|c| c == ' ');
        let has_tab = leading.chars().any(|c| c == '\t');

        match style {
            IndentStyle::Tabs => {
                if has_space {
                    // Suggest fix only if spaces count aligns to indent width and no tabs mixed.
                    let spaces_count = leading.chars().take_while(|c| *c == ' ').count();
                    let only_spaces = !has_tab && spaces_count == leading_ws_len;
                    let fix = if only_spaces && spaces_count % indent_width as usize == 0 {
                        let tabs = spaces_count / indent_width as usize;
                        Some(Fix {
                            start_byte,
                            end_byte: start_byte + leading_ws_len,
                            replacement: "\t".repeat(tabs),
                        })
                    } else {
                        None
                    };

                    let issue = LintIssue {
                        rule_id: "indent-style",
                        message: "Use tabs for indentation".to_string(),
                        line: idx + 1,
                        column: 1,
                        fix,
                    };
                    out.push(issue);
                }
            }
            IndentStyle::Spaces => {
                if has_tab {
                    // Replace each leading tab with indent_width spaces; keep spaces as-is.
                    let replacement: String = leading
                        .chars()
                        .map(|c| if c == '\t' { " ".repeat(indent_width as usize) } else { c.to_string() })
                        .collect();
                    let issue = LintIssue {
                        rule_id: "indent-style",
                        message: "Use spaces for indentation".to_string(),
                        line: idx + 1,
                        column: 1,
                        fix: Some(Fix {
                            start_byte,
                            end_byte: start_byte + leading_ws_len,
                            replacement,
                        }),
                    };
                    out.push(issue);
                }
            }
        }
    }
}

/// Apply a set of non-overlapping fixes to the source. If fixes overlap, later ones win by sorting by range.
pub fn apply_fixes(source: &str, fixes: &[Fix]) -> String {
    if fixes.is_empty() { return source.to_string(); }
    let mut fixes = fixes.to_vec();
    fixes.sort_by_key(|f| f.start_byte);
    let mut out = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for f in fixes {
        if f.start_byte > cursor {
            out.push_str(&source[cursor..f.start_byte]);
        }
        out.push_str(&f.replacement);
        cursor = f.end_byte.min(source.len());
    }
    if cursor < source.len() {
        out.push_str(&source[cursor..]);
    }
    out
}

/// Lint and return a fixed version of the source, applying safe autofixes.
pub fn fix_java_source(source: &str, config: &Config) -> Result<(String, Vec<LintIssue>), LintError> {
    let issues = lint_java_source(source, config)?;
    let fixes: Vec<Fix> = issues
        .iter()
        .filter_map(|i| i.fix.clone())
        .collect();
    let fixed = apply_fixes(source, &fixes);
    Ok((fixed, issues))
}
