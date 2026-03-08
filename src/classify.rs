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
    /// Change affects program behavior.
    Behavioral,
    /// Change is purely cosmetic (whitespace, comments, formatting, imports).
    Cosmetic,
}

pub type SemanticMap = DftHashMap<(u32, u32, u32), SemanticType>;

// ─────────────────────────────────────────────────────────────────────────────
// Line index
// ─────────────────────────────────────────────────────────────────────────────

/// A pre-built random-access index over the lines of a source file.
///
/// # Why this exists
///
/// Every classifier helper (`is_whitespace_only_change`, `is_import_change`,
/// `is_formatting_change`) previously called `get_line_content(src, line_num)`
/// which rescans `src` from byte 0 via `str::lines().nth(n)`.  For a file with
/// L lines and N novel positions, that is O(L × N) string scanning work.
///
/// `LineIndex` builds the index once in O(L) and then each lookup is O(1).
struct LineIndex<'src> {
    lines: Vec<&'src str>,
}

impl<'src> LineIndex<'src> {
    fn new(src: &'src str) -> Self {
        Self {
            lines: src.lines().collect(),
        }
    }

    #[inline]
    fn get(&self, line_num: u32) -> Option<&'src str> {
        self.lines.get(line_num as usize).copied()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Merging helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Convert `Vec<MatchedPos>` into `Vec<ChangeSpan>`, merging adjacent spans.
pub fn merge_positions(positions: Vec<MatchedPos>) -> Vec<ChangeSpan> {
    if positions.is_empty() {
        return vec![];
    }

    let mut positions = positions;
    positions.sort_by(|a, b| {
        a.pos.line
            .cmp(&b.pos.line)
            .then_with(|| a.pos.start_col.cmp(&b.pos.start_col))
    });

    let mut spans: Vec<ChangeSpan> = Vec::with_capacity(positions.len());
    let mut current = ChangeSpan {
        start_line: positions[0].pos.line.0,
        end_line:   positions[0].pos.line.0,
        start_col:  positions[0].pos.start_col,
        end_col:    positions[0].pos.end_col,
    };

    for mp in positions.into_iter().skip(1) {
        let line      = mp.pos.line.0;
        let start_col = mp.pos.start_col;
        let end_col   = mp.pos.end_col;

        if line == current.end_line && start_col <= current.end_col + 1 {
            current.end_col = current.end_col.max(end_col);
        } else {
            spans.push(current);
            current = ChangeSpan { start_line: line, end_line: line, start_col, end_col };
        }
    }
    spans.push(current);
    spans
}

/// Collapse `ChangeSpan`s that share the same `start_line` into one.
pub fn merge_consecutive_lines(spans: Vec<ChangeSpan>) -> Vec<ChangeSpan> {
    if spans.is_empty() {
        return vec![];
    }
    let mut merged: Vec<ChangeSpan> = Vec::with_capacity(spans.len());
    let mut current = spans[0].clone();

    for span in spans.into_iter().skip(1) {
        if span.start_line == current.start_line {
            current.end_line = current.end_line.max(span.end_line);
            current.end_col  = current.end_col.max(span.end_col);
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
/// # Performance improvements
///
/// A `LineIndex` is constructed once from `src` and shared across all
/// classifier calls.  Previously each call to `is_whitespace_only_change`,
/// `is_import_change`, and `is_formatting_change` independently rescanned
/// `src` with `str::lines().nth(n)` — O(L) per lookup.  With the index,
/// every line access is O(1).
///
/// The classifier short-circuits as soon as a position is confirmed cosmetic:
/// we call the cheapest checks first and skip the remaining ones once any
/// returns `true`.  Conversely, `Behavioral` is the default; we only write
/// `Cosmetic` when at least one classifier fires.
///
/// # Note on import detection performance
///
/// The AST-based `is_import_change` may traverse the syntax tree for each
/// position, which can be expensive for large diffs.  In practice the tree
/// depth is limited, and positions are few, so this is acceptable.  If
/// performance becomes an issue, we could precompute a spatial index of
/// import nodes (e.g., an interval tree) and query in O(log n) per position.
pub fn build_semantic_map_for_side<'a>(
    positions: &[MatchedPos],
    src: &str,
    comments: &[MatchedPos],
    change_map: Option<&ChangeMap<'a>>,
    syntax_nodes: Option<&[&'a Syntax<'a>]>,
) -> SemanticMap {
    let mut map = SemanticMap::default();
    let line_idx = LineIndex::new(src);

    for pos in positions.iter().filter(|p| p.kind.is_novel()) {
        let key = (pos.pos.line.0, pos.pos.start_col, pos.pos.end_col);

        // Behavioral wins on collision: once confirmed behavioral we can skip
        // the rest of the classifiers for this position entirely.
        if matches!(map.get(&key), Some(SemanticType::Behavioral)) {
            continue;
        }

        // Run classifiers cheapest-first; short-circuit as soon as one fires.
        // The ordering is:
        //   1. whitespace-only  — trivial string scan, no AST
        //   2. comment          — linear scan of comment list
        //   3. formatting       — single-line content check
        //   4. import           — AST walk or text scan
        //   5. replaced comment/string — recursive AST walk (most expensive)
        let is_cosmetic = is_whitespace_only_change_indexed(pos, &line_idx)
            || is_comment_change(pos, comments)
            || is_formatting_change_indexed(pos, &line_idx)
            || is_import_change_indexed(pos, &line_idx, syntax_nodes)
            || match (change_map, syntax_nodes) {
                (Some(cm), Some(nodes)) => {
                    is_replaced_comment_or_similar_string(pos, cm, nodes)
                }
                _ => false,
            };

        let stype = if is_cosmetic {
            SemanticType::Cosmetic
        } else {
            SemanticType::Behavioral
        };

        map.insert(key, stype);
    }
    map
}

// ─────────────────────────────────────────────────────────────────────────────
// Classification Helpers (public for main.rs)
// ─────────────────────────────────────────────────────────────────────────────

/// True if `pos` falls *within* a comment span.
///
/// # Bug fix: was `c.pos.line == pos.pos.line`
///
/// Line-equality alone causes false negatives on mixed lines such as:
///
/// ```text
/// x += 1;  // counter
/// ```
///
/// If the `+=` is changed, the old check would classify it as cosmetic merely
/// because a comment exists on the same line. We now require column containment:
/// `pos` must actually overlap the comment span, not just share its line.
pub fn is_comment_change(pos: &MatchedPos, comments: &[MatchedPos]) -> bool {
    comments.iter().any(|c| {
        c.pos.line == pos.pos.line
            && c.pos.start_col <= pos.pos.start_col
            && pos.pos.end_col <= c.pos.end_col
    })
}

/// True if `pos` refers to a whitespace-only token or an entirely blank line.
///
/// Uses a pre-built `LineIndex` to avoid rescanning `src` from the beginning
/// on every call.  The public `src: &str` variant below wraps this for callers
/// that don't have an index.
pub fn is_whitespace_only_change(pos: &MatchedPos, src: &str) -> bool {
    let line_idx = LineIndex::new(src);
    is_whitespace_only_change_indexed(pos, &line_idx)
}

fn is_whitespace_only_change_indexed(pos: &MatchedPos, line_idx: &LineIndex<'_>) -> bool {
    if let Some(line) = line_idx.get(pos.pos.line.0) {
        let start = pos.pos.start_col as usize;
        let end   = pos.pos.end_col   as usize;
        if start <= end && end <= line.len() {
            if let Some(content) = line.get(start..end) {
                if content.trim().is_empty() {
                    return true;
                }
            }
        }
        if line.trim().is_empty() {
            return true;
        }
    }
    false
}

/// True if `pos` is part of an import/include/use declaration.
///
/// # Strategy
///
/// 1. **AST-primary**: match against known import-related `ts_node_kind`s when
///    `syntax_nodes` are available. This is exact and has zero false positives.
///
/// 2. **Text-fallback**: check that the changed position is at or near the start
///    of the line *and* that its actual token content is a known import keyword.
///    The fallback no longer lowercases the entire line (case-sensitive languages)
///    and now guards against tokens like `use_count` on a `use_count += 1` line.
pub fn is_import_change(pos: &MatchedPos, src: &str, syntax_nodes: Option<&[&Syntax]>) -> bool {
    let line_idx = LineIndex::new(src);
    is_import_change_indexed(pos, &line_idx, syntax_nodes)
}

fn is_import_change_indexed(
    pos: &MatchedPos,
    line_idx: &LineIndex<'_>,
    syntax_nodes: Option<&[&Syntax]>,
) -> bool {
    // ── 1. AST check (preferred) ──────────────────────────────────────────────
    if let Some(nodes) = syntax_nodes {
        for node in nodes {
            if node_covers_position(node, pos) {
                if is_import_node_kind(crate::parse::semantic_normalizer::node_kind(node)) {
                    return true;
                }
            }
        }
    }

    // ── 2. Text fallback ──────────────────────────────────────────────────────
    let line = match line_idx.get(pos.pos.line.0) {
        Some(l) => l,
        None => return false,
    };

    let trimmed    = line.trim_start();
    let indent_len = (line.len() - trimmed.len()) as u32;

    // Only fire when pos starts at or immediately after the first non-whitespace
    // column. This prevents `use_count += 1` from matching because `use_count`
    // starts at the same column as the `use` keyword on a `use some::path;` line.
    if pos.pos.start_col > indent_len + 1 {
        return false;
    }

    // Check the actual changed token, not the full lowercased line.
    let token = get_content_at_pos_indexed(line_idx, pos).unwrap_or("");

    // `#include` — C/C++
    if trimmed.starts_with("#include") {
        return true;
    }

    // `import` — Python, JS, Java, …
    if trimmed.starts_with("import ") && (token == "import" || token.starts_with("import ")) {
        return true;
    }

    // `from … import` — Python
    if trimmed.starts_with("from ") && token == "from" {
        return true;
    }

    // `use` — Rust: only when the changed token is exactly "use" to avoid
    // matching `use_count`, `user`, etc.
    if trimmed.starts_with("use ") && token == "use" {
        return true;
    }

    // `extern crate` — Rust
    if trimmed.starts_with("extern crate ") && (token == "extern" || token == "crate") {
        return true;
    }

    // `require(` — CommonJS / Ruby (only at line start)
    if trimmed.starts_with("require(") && token.starts_with("require") {
        return true;
    }

    false
}

/// True if `pos` is a purely cosmetic formatting token.
///
/// # Improvements over previous version
///
/// - Whitespace-only token content → cosmetic (was not checked here before).
/// - Trailing `,`/`;` now also handles a trailing line comment (`// …`, `/* … */`).
/// - Lone `{` or `}` on their own line (brace-style change) → cosmetic.
/// - Safe column indexing via `.get()` to avoid panicking on multibyte chars.
pub fn is_formatting_change(pos: &MatchedPos, src: &str) -> bool {
    let line_idx = LineIndex::new(src);
    is_formatting_change_indexed(pos, &line_idx)
}

fn is_formatting_change_indexed(pos: &MatchedPos, line_idx: &LineIndex<'_>) -> bool {
    let content = match get_content_at_pos_indexed(line_idx, pos) {
        Some(c) => c,
        None => return false,
    };

    // Whitespace-only tokens are always cosmetic.
    if content.trim().is_empty() {
        return true;
    }

    // Trailing separator: `,` or `;` followed only by whitespace, a closing
    // delimiter, or a line comment — never followed by meaningful code.
    if content == "," || content == ";" {
        if let Some(line) = line_idx.get(pos.pos.line.0) {
            let end    = pos.pos.end_col as usize;
            let suffix = line.get(end..).unwrap_or("").trim();
            if suffix.is_empty()
                || suffix.starts_with(')')
                || suffix.starts_with(']')
                || suffix.starts_with('}')
                || suffix.starts_with("//")
                || suffix.starts_with("/*")
                || suffix.starts_with('#')
            {
                return true;
            }
        }
    }

    // Brace-on-its-own-line: `{` or `}` when that is all (modulo whitespace)
    // on the line. This is a brace-style change (K&R vs. Allman), which is
    // cosmetic formatting, not a behavioral change.
    if content == "{" || content == "}" {
        if let Some(line) = line_idx.get(pos.pos.line.0) {
            let trimmed = line.trim();
            if trimmed == "{" || trimmed == "}" {
                return true;
            }
        }
    }

    false
}

// ─────────────────────────────────────────────────────────────────────────────
// Private: ReplacedComment / ReplacedString check
// ─────────────────────────────────────────────────────────────────────────────

/// True if `pos` falls within a syntax node that the differ recorded as a
/// high-similarity replaced comment or string (≥ 80 % similarity).
///
/// We recurse into child nodes so that inner atoms — not just the top-level
/// list wrappers passed in as `syntax_nodes` — can be matched. This prevents
/// false positives where a large container (e.g. a function body) is a
/// `ReplacedString` match and incorrectly marks every inner change as cosmetic.
fn is_replaced_comment_or_similar_string<'a>(
    pos: &MatchedPos,
    change_map: &ChangeMap<'a>,
    syntax_nodes: &[&'a Syntax<'a>],
) -> bool {
    syntax_nodes
        .iter()
        .any(|node| check_node_replaced(node, pos, change_map))
}

/// Recursively check `node` and its descendants for a ReplacedComment /
/// ReplacedString entry in `change_map` that covers `pos`.
///
/// Recursion is bounded by the depth of the syntax tree, which difftastic
/// already limits via `DFT_PARSE_ERROR_LIMIT`. In practice the tree is shallow
/// for the regions we care about (comment atoms are leaves).
fn check_node_replaced<'a>(
    node: &'a Syntax<'a>,
    pos: &MatchedPos,
    change_map: &ChangeMap<'a>,
) -> bool {
    match node {
        Syntax::Atom { position, .. } => {
            // Atoms are matched precisely: the pos must fall within this atom's
            // own span, not merely on the same line.
            let covers = position.iter().any(|p| {
                p.line == pos.pos.line
                    && p.start_col <= pos.pos.start_col
                    && pos.pos.end_col <= p.end_col
            });
            if !covers {
                return false;
            }
            if let Some(ck) = change_map.get(node) {
                return is_high_similarity_replace(&ck);
            }
            false
        }
        Syntax::List { children, .. } => {
            // Only descend when node_covers_position confirms this list spans pos,
            // so we avoid walking irrelevant subtrees.
            if !node_covers_position(node, pos) {
                return false;
            }
            children
                .iter()
                .any(|child| check_node_replaced(child, pos, change_map))
        }
    }
}

/// True when a `ChangeKind` represents a high-similarity replaced comment or
/// string (our threshold for treating it as cosmetic is ≥ 80 %).
///
/// Extracted to a named function so the magic constant is documented once.
#[inline]
fn is_high_similarity_replace(ck: &ChangeKind) -> bool {
    match ck {
        ChangeKind::ReplacedComment(..) => true,
        // `pct` is the similarity percentage (0–100).  We treat ≥ 80 % as
        // cosmetic: the strings are so similar that the change is likely a
        // minor wording tweak rather than a meaningful content change.
        ChangeKind::ReplacedString(_, _, pct) => *pct >= 80,
        _ => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Private: import node-kind predicate
// ─────────────────────────────────────────────────────────────────────────────

/// True for any tree-sitter node kind that represents an import/include/use
/// declaration across the languages difftastic supports.
#[inline]
fn is_import_node_kind(kind: &str) -> bool {
    matches!(
        kind,
        "import_statement"
            | "import_declaration"
            | "import_from_statement"   // Python `from x import y`
            | "use_declaration"          // Rust
            | "use_tree"                 // Rust nested `use { … }`
            | "include_directive"
            | "preproc_include"          // tree-sitter-c / tree-sitter-cpp
            | "preproc_import"
            | "require_call"
            | "extern_declaration"
    ) || kind.ends_with("import")
}

// ─────────────────────────────────────────────────────────────────────────────
// Private: low-level position helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Return the source text at `pos` using a pre-built `LineIndex`.
///
/// Uses `.get(start..end)` instead of indexing to avoid panicking on invalid
/// UTF-8 byte boundaries (tree-sitter positions are byte offsets, but defensive
/// slicing prevents UB on malformed input).
fn get_content_at_pos_indexed<'src>(
    line_idx: &LineIndex<'src>,
    pos: &MatchedPos,
) -> Option<&'src str> {
    let line  = line_idx.get(pos.pos.line.0)?;
    let start = pos.pos.start_col as usize;
    let end   = pos.pos.end_col   as usize;
    if start <= end && end <= line.len() {
        line.get(start..end)
    } else {
        None
    }
}

/// True if `node` spatially covers `pos`.
///
/// # Fix: column-bounded containment for List nodes
///
/// The previous implementation used a bare line-range check for `List` nodes:
///
/// ```text
/// (Some(s), Some(e)) if s <= target_line && target_line <= e => return true,
/// ```
///
/// This meant every position inside a 100-line function body was considered
/// "covered" by the function node — a huge source of false positives in both
/// `is_import_change` and `is_replaced_comment_or_similar_string`.
///
/// The fix: for `List`, we check that `pos` lies within the column-bounded
/// extent from the first open-delimiter token to the last close-delimiter token.
/// For `Atom`, we require that `pos`'s column range falls *within* the atom's
/// span (containment), not merely that start columns match.
fn node_covers_position(node: &Syntax, pos: &MatchedPos) -> bool {
    match node {
        Syntax::List { open_position, close_position, .. } => {
            let open_start  = open_position.first();
            let close_end   = close_position.last().or_else(|| open_position.last());

            if let (Some(s), Some(e)) = (open_start, close_end) {
                let tl = pos.pos.line.0;
                let tc = pos.pos.start_col;
                let te = pos.pos.end_col;

                // `pos` must start after (or at) the opening delimiter …
                let after_open = tl > s.line.0
                    || (tl == s.line.0 && tc >= s.start_col);

                // … and end before (or at) the closing delimiter.
                let before_close = tl < e.line.0
                    || (tl == e.line.0 && te <= e.end_col);

                after_open && before_close
            } else {
                // Degenerate node: fall back to exact token matching.
                open_position.iter().any(|p| {
                    p.line == pos.pos.line
                        && p.start_col <= pos.pos.start_col
                        && pos.pos.end_col <= p.end_col
                }) || close_position.iter().any(|p| {
                    p.line == pos.pos.line
                        && p.start_col <= pos.pos.start_col
                        && pos.pos.end_col <= p.end_col
                })
            }
        }
        Syntax::Atom { position, .. } => {
            // Containment check: pos must lie within the atom's column span.
            // The old check only compared start_col, which failed for partial
            // overlaps and multi-token atoms.
            position.iter().any(|p| {
                p.line == pos.pos.line
                    && p.start_col <= pos.pos.start_col
                    && pos.pos.end_col <= p.end_col
            })
        }
    }
}