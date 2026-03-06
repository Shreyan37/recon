//! Classify novel diff positions as behavioral or cosmetic, and build
//! per-position semantic maps for --semantic-colors.

use crate::diff::changes::{ChangeKind, ChangeMap};
use crate::hash::DftHashMap;
use crate::parse::syntax::{AtomKind, MatchKind, MatchedPos, Syntax, TokenKind};

// ─────────────────────────────────────────────────────────────────────────────
// ChangeSpan
// ─────────────────────────────────────────────────────────────────────────────

/// A change that may span multiple consecutive source lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSpan {
    pub start_line: u32,
    pub end_line: u32,
    pub start_col: u32,
    pub end_col: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// SemanticType + SemanticMap
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticType {
    /// Change affects program semantics. Shown in red on both sides.
    Behavioral,
    /// Change does not affect semantics (comments, whitespace, imports,
    /// formatting). Shown in blue.
    Cosmetic,
}

/// Maps `(line, start_col, end_col)` → `SemanticType` for every novel
/// `MatchedPos` on one side of a diff.
pub type SemanticMap = DftHashMap<(u32, u32, u32), SemanticType>;

// ─────────────────────────────────────────────────────────────────────────────
// MatchedPosExt trait
// ─────────────────────────────────────────────────────────────────────────────

pub trait MatchedPosExt {
    fn overlaps(&self, other: &Self) -> bool;
}

impl MatchedPosExt for MatchedPos {
    fn overlaps(&self, other: &MatchedPos) -> bool {
        self.pos.line == other.pos.line
            && self.pos.start_col < other.pos.end_col
            && other.pos.start_col < self.pos.end_col
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Classification helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns true if `pos` is (or overlaps) a comment token.
pub fn is_comment_change(pos: &MatchedPos, comments: &[MatchedPos]) -> bool {
    let highlight = match &pos.kind {
        MatchKind::Novel { highlight, .. } => Some(highlight),
        MatchKind::NovelWord { highlight } => Some(highlight),
        MatchKind::UnchangedPartOfNovelItem { highlight, .. } => Some(highlight),
        MatchKind::UnchangedToken { highlight, .. } => Some(highlight),
        MatchKind::Ignored { highlight } => Some(highlight),
    };
    if matches!(highlight, Some(TokenKind::Atom(AtomKind::Comment))) {
        return true;
    }
    for comment in comments {
        if comment.pos.line == pos.pos.line
            && comment.pos.start_col <= pos.pos.start_col
            && comment.pos.end_col >= pos.pos.end_col
        {
            return true;
        }
    }
    false
}

/// Returns true if the source text covered by `pos` is pure ASCII whitespace.
pub fn is_whitespace_only_change(pos: &MatchedPos, src: &str) -> bool {
    let line = match src.lines().nth(pos.pos.line.0 as usize) {
        Some(l) => l,
        None => return false,
    };
    let start = pos.pos.start_col as usize;
    let end = pos.pos.end_col as usize;

    if start >= end || end > line.as_bytes().len() {
        return false;
    }

    line.as_bytes()[start..end]
        .iter()
        .all(|&b| b == b' ' || b == b'\t')
}

/// Returns true if `pos` lives on a line that is purely an import/use
/// statement.
///
/// # Why line-level rather than token-level
///
/// A previous version checked whether the token itself was the `use`/`import`
/// keyword. That only marked one token as cosmetic — every path component
/// (`std`, `collections`, `HashMap`) remained behavioral. The correct approach
/// is to inspect the **whole source line**: if the line is an import statement,
/// every novel token on it is cosmetic.
///
/// # Language coverage
/// Rust (`use`), C/C++ (`#include`), Python/JS/TS/Go/Java/Kotlin/Haskell
/// (`import`, `from … import`), C#/Dart (`using`).
pub fn is_import_change(pos: &MatchedPos, src: &str) -> bool {
    let line = match src.lines().nth(pos.pos.line.0 as usize) {
        Some(l) => l,
        None => return false,
    };
    let t = line.trim_start();
    t.starts_with("use ")
        || t.starts_with("use\t")
        || t.starts_with("#include")
        || t.starts_with("import ")
        || t.starts_with("import\t")
        || t.starts_with("from ")
        || t.starts_with("using ")
}

/// Returns true if `pos` is a delimiter token whose change is purely
/// cosmetic formatting.
///
/// Currently only trailing commas qualify. Semicolons and structural
/// brackets (`{}`, `()`, `[]`) are left as behavioral because they alter
/// program structure.
pub fn is_formatting_change(pos: &MatchedPos, src: &str) -> bool {
    let is_delimiter = matches!(
        &pos.kind,
        MatchKind::Novel {
            highlight: TokenKind::Delimiter,
            ..
        } | MatchKind::NovelWord {
            highlight: TokenKind::Delimiter,
        }
    );
    if !is_delimiter {
        return false;
    }

    let line = match src.lines().nth(pos.pos.line.0 as usize) {
        Some(l) => l,
        None => return false,
    };
    let start = pos.pos.start_col as usize;
    let end = pos.pos.end_col as usize;
    if start >= end || end > line.len() {
        return false;
    }

    line[start..end].trim() == ","
}

/// Returns true if the diff engine matched this node as a replaced comment,
/// or as a replaced string literal with high similarity (≥ 70 %).
///
/// A `ReplacedComment` is always cosmetic — it is a comment whose wording
/// changed but its position in the code did not.
///
/// A `ReplacedString` with high similarity catches common patterns like
/// renaming a log message or updating a version string where the content is
/// very close to the original. Low-similarity string replacements remain
/// behavioral because the actual data changed meaningfully.
///
/// When `syntax_nodes` is `None` (e.g. for plain-text files or early in the
/// pipeline before AST nodes are available) the function returns `false`
/// without panicking.
pub fn is_replaced_comment_or_similar_string<'a>(
    pos: &MatchedPos,
    change_map: &ChangeMap<'a>,
    syntax_nodes: &[&'a Syntax<'a>],
) -> bool {
    let node = match find_syntax_node_for_position(pos, syntax_nodes) {
        Some(n) => n,
        None => return false,
    };
    match change_map.get(node) {
        Some(ChangeKind::ReplacedComment(_, _, _)) => true,
        Some(ChangeKind::ReplacedString(_, _, pct)) => pct >= 70,
        _ => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Syntax-node lookup helpers
// ─────────────────────────────────────────────────────────────────────────────

fn find_syntax_node_for_position<'a>(
    pos: &MatchedPos,
    nodes: &[&'a Syntax<'a>],
) -> Option<&'a Syntax<'a>> {
    for node in nodes {
        if let Some(found) = find_in_subtree(node, pos) {
            return Some(found);
        }
    }
    None
}

/// Recursively search `node` and its descendants for a node whose position
/// covers `pos`.
///
/// The original flat search only checked root-level nodes, so `ReplacedComment`
/// entries inside function bodies (nested several levels deep) were never
/// found.  This recursive version fixes that.
fn find_in_subtree<'a>(node: &'a Syntax<'a>, pos: &MatchedPos) -> Option<&'a Syntax<'a>> {
    if node_contains_position(node, pos) {
        return Some(node);
    }
    if let Syntax::List { children, .. } = node {
        for child in children {
            if let Some(found) = find_in_subtree(child, pos) {
                return Some(found);
            }
        }
    }
    None
}

fn node_contains_position(node: &Syntax, pos: &MatchedPos) -> bool {
    match node {
        Syntax::Atom { position, .. } => position.iter().any(|p| {
            p.line == pos.pos.line
                && p.start_col <= pos.pos.start_col
                && p.end_col >= pos.pos.end_col
        }),
        Syntax::List {
            open_position,
            close_position,
            ..
        } => open_position
            .iter()
            .chain(close_position.iter())
            .any(|p| {
                p.line == pos.pos.line
                    && p.start_col <= pos.pos.start_col
                    && p.end_col >= pos.pos.end_col
            }),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Merge utilities
// ─────────────────────────────────────────────────────────────────────────────

/// Merge adjacent `MatchedPos` spans on the same line.
pub fn merge_positions(mut positions: Vec<MatchedPos>) -> Vec<MatchedPos> {
    if positions.is_empty() {
        return vec![];
    }
    positions.sort_by(|a, b| {
        a.pos
            .line
            .cmp(&b.pos.line)
            .then(a.pos.start_col.cmp(&b.pos.start_col))
    });

    let mut merged: Vec<MatchedPos> = Vec::with_capacity(positions.len());
    let mut current = positions.remove(0);

    for pos in positions {
        if pos.pos.line == current.pos.line && pos.pos.start_col == current.pos.end_col {
            current.pos.end_col = pos.pos.end_col;
        } else {
            merged.push(current);
            current = pos;
        }
    }
    merged.push(current);
    merged
}

/// Convert `Vec<MatchedPos>` → `Vec<ChangeSpan>`, collapsing consecutive lines.
pub fn merge_consecutive_lines(positions: Vec<MatchedPos>) -> Vec<ChangeSpan> {
    if positions.is_empty() {
        return vec![];
    }
    let first = &positions[0];
    let mut current = ChangeSpan {
        start_line: first.pos.line.0,
        end_line: first.pos.line.0,
        start_col: first.pos.start_col,
        end_col: first.pos.end_col,
    };
    let mut spans: Vec<ChangeSpan> = Vec::with_capacity(positions.len());

    for pos in positions.iter().skip(1) {
        let line = pos.pos.line.0;
        if line <= current.end_line + 1 {
            if line > current.end_line {
                current.end_line = line;
            }
        } else {
            spans.push(current);
            current = ChangeSpan {
                start_line: line,
                end_line: line,
                start_col: pos.pos.start_col,
                end_col: pos.pos.end_col,
            };
        }
    }
    spans.push(current);
    spans
}

/// Collapse `ChangeSpan`s that share the same `start_line` into one.
pub fn merge_spans_by_line(spans: Vec<ChangeSpan>) -> Vec<ChangeSpan> {
    if spans.is_empty() {
        return vec![];
    }
    let mut merged: Vec<ChangeSpan> = Vec::with_capacity(spans.len());
    let mut current = spans[0].clone();

    for span in spans.into_iter().skip(1) {
        if span.start_line == current.start_line {
            current.end_line = current.end_line.max(span.end_line);
            current.end_col = current.end_col.max(span.end_col);
        } else {
            merged.push(current);
            current = span;
        }
    }
    merged.push(current);
    merged
}

// ─────────────────────────────────────────────────────────────────────────────
// SemanticMap builder
// ─────────────────────────────────────────────────────────────────────────────

/// Build a `SemanticMap` for one side of the diff.
///
/// `change_map` and `syntax_nodes` are optional. When provided,
/// `ReplacedComment` and high-similarity `ReplacedString` nodes are also
/// classified as cosmetic. When absent, those checks are skipped gracefully —
/// the function never panics.
pub fn build_semantic_map_for_side<'a>(
    positions: &[MatchedPos],
    src: &str,
    comments: &[MatchedPos],
    change_map: Option<&ChangeMap<'a>>,
    syntax_nodes: Option<&[&'a Syntax<'a>]>,
) -> SemanticMap {
    let mut map = SemanticMap::default();

    for pos in positions.iter().filter(|p| p.kind.is_novel()) {
        let replaced_cosmetic = match (change_map, syntax_nodes) {
            (Some(cm), Some(nodes)) => is_replaced_comment_or_similar_string(pos, cm, nodes),
            _ => false,
        };

        let stype = if is_comment_change(pos, comments)
            || is_whitespace_only_change(pos, src)
            || is_import_change(pos, src)
            || is_formatting_change(pos, src)
            || replaced_cosmetic
        {
            SemanticType::Cosmetic
        } else {
            SemanticType::Behavioral
        };

        let key = (pos.pos.line.0, pos.pos.start_col, pos.pos.end_col);

        // Behavioral wins on key collision.
        match map.get(&key) {
            Some(SemanticType::Behavioral) => {}
            _ => {
                map.insert(key, stype);
            }
        }
    }

    map
}