//! Classify novel diff positions as behavioral or cosmetic, and build
//! per-position semantic maps for --semantic-colors.

use crate::hash::DftHashMap;
use crate::parse::syntax::{AtomKind, MatchKind, MatchedPos, TokenKind};

// ─────────────────────────────────────────────────────────────────────────────
// ChangeSpan
// ─────────────────────────────────────────────────────────────────────────────

/// A change that may span multiple consecutive source lines.
///
/// Unlike `SingleLineSpan` (which `MatchedPos` uses internally and which is
/// always confined to one line), this type records a range from `start_line`
/// through `end_line`. Both values are **0-indexed**; callers that display
/// them to the user should add 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSpan {
    /// First line of the change, 0-indexed.
    pub start_line: u32,
    /// Last line of the change, 0-indexed. Equal to `start_line` for
    /// single-line changes.
    pub end_line: u32,
    /// Start column on `start_line`, in bytes.
    pub start_col: u32,
    /// End column on `start_line`, in bytes.
    pub end_col: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// SemanticType + SemanticMap
// ─────────────────────────────────────────────────────────────────────────────

/// Whether a novel change is likely to affect program behaviour.
///
/// Used by `--semantic-colors` to choose the highlight colour for each
/// individual novel token rather than showing all changes in a single
/// left=red / right=green scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticType {
    /// Change affects program semantics (logic, variable names, function
    /// calls, structural delimiters). Shown in red.
    ///
    /// Note: novel delimiters such as `{` or `}` are always classified as
    /// Behavioral because there is no way to determine purely from token
    /// positions whether a brace addition is purely structural formatting
    /// or introduces a new scope. This is a known limitation.
    Behavioral,

    /// Change does not affect semantics (comments, whitespace, formatting).
    /// Shown in blue.
    ///
    /// When `--ignore-comments` is active, comment nodes are already excluded
    /// from the AST before diffing, so no novel comment positions exist.
    /// Passing empty comment slices is correct: `is_comment_change` always
    /// returns false, and every surviving novel position is real code
    /// (correctly classified Behavioral).
    Cosmetic,
}

/// Maps `(line, start_col, end_col)` → `SemanticType` for every novel
/// `MatchedPos` on one side of a diff.
///
/// The key uses the **original** position triple as it appears in the
/// `MatchedPos` slice. `color_positions` in `style.rs` iterates the same
/// un-merged positions, so lookups always hit.
///
/// When two `MatchedPos` entries share the same key (possible when
/// `split_atom_words` produces sub-spans of a multi-line atom),
/// `Behavioral` wins over `Cosmetic`: a change is considered semantic if
/// *any* reading of the position is semantic.
pub type SemanticMap = DftHashMap<(u32, u32, u32), SemanticType>;

// ─────────────────────────────────────────────────────────────────────────────
// MatchedPosExt trait
// ─────────────────────────────────────────────────────────────────────────────

/// Extension methods on `MatchedPos` used internally by this module.
///
/// Imported in `main.rs` as `use crate::classify::MatchedPosExt as _` so
/// that the trait methods are in scope for callers that delegate to helpers
/// defined here.
pub trait MatchedPosExt {
    /// Returns true if `self` and `other` occupy overlapping byte ranges on
    /// the same source line.
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
///
/// Two checks are performed:
/// 1. The token's own `highlight` field is `AtomKind::Comment`.
/// 2. The token's span is contained within one of the spans in the
///    `comments` slice (which comes from `tsp::comment_positions`).
///
/// Either check is sufficient. The second handles cases where a novel
/// position is a sub-span of a larger comment atom.
pub fn is_comment_change(pos: &MatchedPos, comments: &[MatchedPos]) -> bool {
    // Check own highlight field first — cheap and covers most cases.
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

    // Fallback: check whether pos is covered by a comment span from
    // tsp::comment_positions. This catches cases where the highlight field
    // was set to Normal because tree-sitter returned a generic node, but the
    // actual token is inside a comment region.
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

/// Returns true if the source text covered by `pos` consists entirely of
/// ASCII whitespace (spaces and tabs).
///
/// `src` **must** be the file that `pos` belongs to — `lhs_src` for lhs
/// positions, `rhs_src` for rhs positions. Passing the wrong side causes
/// incorrect results because column offsets are file-specific.
///
/// Out-of-bounds or zero-length spans return false rather than panicking.
/// An out-of-bounds span is not a whitespace-only change.
pub fn is_whitespace_only_change(pos: &MatchedPos, src: &str) -> bool {
    let line = match src.lines().nth(pos.pos.line.0 as usize) {
        Some(l) => l,
        None => return false,
    };

    let start = pos.pos.start_col as usize;
    let end = pos.pos.end_col as usize;

    // Guard against zero-length or out-of-bounds spans.
    if start >= end || end > line.as_bytes().len() {
        return false;
    }

    // start_col / end_col are byte offsets (see style.rs `substring_by_byte`
    // usage). Whitespace is ASCII-only so byte slicing is safe here.
    line.as_bytes()[start..end]
        .iter()
        .all(|&b| b == b' ' || b == b'\t')
}

// ─────────────────────────────────────────────────────────────────────────────
// Merge utilities
// ─────────────────────────────────────────────────────────────────────────────

/// Merge adjacent `MatchedPos` spans on the same line.
///
/// Two spans are considered adjacent when the `end_col` of the first equals
/// the `start_col` of the second and both are on the same line.
///
/// Merging before classification means that sub-token spans (e.g. the leading
/// and trailing whitespace that `split_atom_words` emits around a changed
/// word) are seen as a single unit by the classifiers.
pub fn merge_positions(mut positions: Vec<MatchedPos>) -> Vec<MatchedPos> {
    if positions.is_empty() {
        return vec![];
    }

    // Sort by line then start_col so adjacent spans are next to each other.
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
            // Extend the current span rightward.
            current.pos.end_col = pos.pos.end_col;
        } else {
            merged.push(current);
            current = pos;
        }
    }
    merged.push(current);
    merged
}

/// Convert a `Vec<MatchedPos>` into `Vec<ChangeSpan>`, collapsing runs of
/// positions on consecutive lines into a single span.
///
/// The returned `ChangeSpan` records the first and last line of the run and
/// the column info of the first position in the run. This is used by
/// `--summarize` display to print "Lines 7-12" rather than six separate
/// entries.
///
/// **Correctness note**: unlike the old single-field approach, this function
/// introduces an explicit `end_line` field so that a multi-line deletion such
/// as lines 7–9 is stored as `{ start_line: 7, end_line: 9, start_col: 0,
/// end_col: 1 }` rather than overwriting `end_col` with each line's value.
pub fn merge_consecutive_lines(positions: Vec<MatchedPos>) -> Vec<ChangeSpan> {
    if positions.is_empty() {
        return vec![];
    }

    let mut spans: Vec<ChangeSpan> = Vec::with_capacity(positions.len());
    let first = &positions[0];
    let mut current = ChangeSpan {
        start_line: first.pos.line.0,
        end_line: first.pos.line.0,
        start_col: first.pos.start_col,
        end_col: first.pos.end_col,
    };

    for pos in positions.iter().skip(1) {
        let line = pos.pos.line.0;
        if line <= current.end_line + 1 {
            // Consecutive or same line — extend the span downward.
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
///
/// When a single source line contains two separate novel token groups
/// (e.g. `std::` and `swap` when the whole expression is novel), they produce
/// two separate `ChangeSpan`s with the same `start_line`. This function
/// merges them into a single entry, taking the maximum `end_line` and
/// `end_col`.
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
/// Call once for LHS with `(lhs_positions, lhs_src, lhs_comments)` and once
/// for RHS with `(rhs_positions, rhs_src, rhs_comments)`. Do **not** mix
/// sides: each map lives in a different coordinate space.
///
/// Only novel positions (`is_novel()` == true) get an entry. Unchanged
/// positions are absent; `novel_color` in `style.rs` treats an absent key as
/// "use standard `novel_style`", so the fallback is transparent.
pub fn build_semantic_map_for_side(
    positions: &[MatchedPos],
    src: &str,
    comments: &[MatchedPos],
) -> SemanticMap {
    let mut map = SemanticMap::default();

    for pos in positions.iter().filter(|p| p.kind.is_novel()) {
        let stype = if is_comment_change(pos, comments) || is_whitespace_only_change(pos, src) {
            SemanticType::Cosmetic
        } else {
            SemanticType::Behavioral
        };

        let key = (pos.pos.line.0, pos.pos.start_col, pos.pos.end_col);

        // Behavioral wins on collision (see type-level doc).
        match map.get(&key) {
            Some(SemanticType::Behavioral) => {}
            _ => {
                map.insert(key, stype);
            }
        }
    }

    map
}