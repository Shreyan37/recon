use crate::hash::DftHashMap;
use crate::parse::syntax::MatchedPos;

// ─── ChangeSpan ──────────────────────────────────────────────────────────────

/// A change span that may cover multiple source lines.
///
/// `start_line`/`end_line` are 0-indexed; callers that display them add 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSpan {
    pub start_line: u32,
    pub end_line: u32,
    pub start_col: u32,
    pub end_col: u32,
}

// ─── SemanticType + SemanticMap ──────────────────────────────────────────────

/// Whether a novel change is likely to affect program behaviour.
///
/// Used by `--semantic-colors` to choose the highlight colour for each
/// individual novel token rather than showing all changes in a single
/// left=red / right=green scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticType {
    /// Change affects program semantics. Shown in red (standard novel colour).
    Behavioral,
    /// Change does not affect semantics (comments, whitespace). Shown in blue.
    Cosmetic,
}

/// Maps `(line, start_col, end_col)` → `SemanticType` for every novel
/// `MatchedPos` on one side of a diff.
///
/// The key uses the **original** (un-merged) position triple, matching what
/// `color_positions` in `style.rs` iterates.
///
/// On key collision, `Behavioral` wins: a change is semantic if *any*
/// interpretation of the position is semantic.
pub type SemanticMap = DftHashMap<(u32, u32, u32), SemanticType>;

// ─── Classification helpers ───────────────────────────────────────────────────

/// Returns true if the span is entirely whitespace in the given source.
///
/// Pass `lhs_src` for lhs positions and `rhs_src` for rhs positions.
pub fn is_whitespace_only_change(pos: &MatchedPos, src: &str) -> bool {
    let line = match src.lines().nth(pos.pos.line.0 as usize) {
        Some(l) => l,
        None => return false,
    };
    let start = pos.pos.start_col as usize;
    let end = pos.pos.end_col as usize;
    if start >= line.len() || end > line.len() || start >= end {
        return false;
    }
    line[start..end].trim().is_empty()
}

/// Returns true if `pos` overlaps any comment position on the same side.
///
/// Always pass comments from the same file as `pos`.
pub fn is_comment_change(pos: &MatchedPos, same_side_comments: &[MatchedPos]) -> bool {
    same_side_comments.iter().any(|c| c.overlaps(pos))
}

/// Merge adjacent / overlapping spans on the same line (returns `Vec<MatchedPos>`).
pub fn merge_positions(positions: Vec<MatchedPos>) -> Vec<MatchedPos> {
    if positions.is_empty() {
        return positions;
    }
    let mut sorted = positions;
    sorted.sort_by(|a, b| {
        a.pos
            .line
            .cmp(&b.pos.line)
            .then(a.pos.start_col.cmp(&b.pos.start_col))
    });
    let mut merged: Vec<MatchedPos> = Vec::new();
    let mut current = sorted[0].clone();
    for pos in &sorted[1..] {
        if pos.pos.line == current.pos.line && pos.pos.start_col <= current.pos.end_col + 1 {
            current.pos.end_col = current.pos.end_col.max(pos.pos.end_col);
        } else {
            merged.push(current);
            current = pos.clone();
        }
    }
    merged.push(current);
    merged
}

/// Merge spans on the same starting line (returns `Vec<ChangeSpan>`).
pub fn merge_spans_by_line(positions: Vec<ChangeSpan>) -> Vec<ChangeSpan> {
    if positions.is_empty() {
        return positions;
    }
    let mut sorted = positions;
    sorted.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.start_col.cmp(&b.start_col))
    });
    let mut merged: Vec<ChangeSpan> = Vec::new();
    let mut current = sorted[0].clone();
    for span in &sorted[1..] {
        if span.start_line == current.start_line {
            current.end_col = current.end_col.max(span.end_col);
            current.end_line = current.end_line.max(span.end_line);
        } else {
            merged.push(current);
            current = span.clone();
        }
    }
    merged.push(current);
    merged
}

/// Merge spans on consecutive lines into a single `ChangeSpan`.
///
/// The `start_col`/`end_col` of the result reflect the **first** line of the
/// run, which is typically the most informative for display.
pub fn merge_consecutive_lines(positions: Vec<MatchedPos>) -> Vec<ChangeSpan> {
    if positions.is_empty() {
        return vec![];
    }
    let mut sorted = positions;
    sorted.sort_by(|a, b| {
        a.pos
            .line
            .cmp(&b.pos.line)
            .then(a.pos.start_col.cmp(&b.pos.start_col))
    });

    let first = &sorted[0];
    let mut current_span = ChangeSpan {
        start_line: first.pos.line.0,
        end_line: first.pos.line.0,
        start_col: first.pos.start_col,
        end_col: first.pos.end_col,
    };
    let mut last_line = first.pos.line.0;
    let mut result: Vec<ChangeSpan> = Vec::new();

    for pos in &sorted[1..] {
        let this_line = pos.pos.line.0;
        if this_line == last_line {
            current_span.end_col = current_span.end_col.max(pos.pos.end_col);
        } else if this_line == last_line + 1 {
            current_span.end_line = this_line;
            last_line = this_line;
        } else {
            result.push(current_span);
            current_span = ChangeSpan {
                start_line: this_line,
                end_line: this_line,
                start_col: pos.pos.start_col,
                end_col: pos.pos.end_col,
            };
            last_line = this_line;
        }
    }
    result.push(current_span);
    result
}

// ─── SemanticMap builder ──────────────────────────────────────────────────────

/// Build a `SemanticMap` for one side of the diff.
///
/// Call once for LHS with `(lhs_positions, lhs_src, lhs_comments)` and once
/// for RHS with `(rhs_positions, rhs_src, rhs_comments)`.
/// Do **not** mix sides: each map lives in a different coordinate space.
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
        match map.get(&key) {
            Some(SemanticType::Behavioral) => {}
            _ => {
                map.insert(key, stype);
            }
        }
    }
    map
}

// ─── MatchedPosExt ────────────────────────────────────────────────────────────

pub trait MatchedPosExt {
    fn overlaps(&self, other: &MatchedPos) -> bool;
}

impl MatchedPosExt for MatchedPos {
    fn overlaps(&self, other: &MatchedPos) -> bool {
        self.pos.line == other.pos.line
            && self.pos.start_col < other.pos.end_col
            && other.pos.start_col < self.pos.end_col
    }
}