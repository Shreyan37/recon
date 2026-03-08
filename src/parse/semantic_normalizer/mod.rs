//! Semantic normalization module.
//!
//! This module provides the infrastructure for rewriting stylistically
//! different but semantically equivalent code to a shared canonical form.

use std::collections::HashMap;
use typed_arena::Arena;

use crate::options::SemanticLevel;
use crate::parse::guess_language as guess;
use crate::parse::syntax::{AtomKind, Syntax};

pub mod algebraic;
pub mod c;
pub mod javascript;
pub mod python;
pub mod rust;

// ─────────────────────────────────────────────────────────────────────────────
// Public trait
// ─────────────────────────────────────────────────────────────────────────────

/// Trait for language-specific semantic normalizers.
pub trait SemanticNormalizer {
    fn language(&self) -> guess::Language;

    fn normalize<'a>(
        &self,
        node: &'a Syntax<'a>,
        parent: Option<&'a Syntax<'a>>,
        arena: &'a Arena<Syntax<'a>>,
        level: SemanticLevel,
    ) -> Option<&'a Syntax<'a>>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

type RenameMap = HashMap<String, String>;

/// Normalize all roots using the language-specific normalizer.
///
/// IMPORTANT: After calling this function, you MUST re-run
/// `syntax::init_all_info` on the returned trees — the normalizer
/// allocates new nodes in the arena and clears the `id` counters
/// on the Advanced level per §3.3 of the book.
pub fn normalize_all<'a>(
    language: guess::Language,
    level: SemanticLevel,
    lhs: &mut Vec<&'a Syntax<'a>>,
    rhs: &mut Vec<&'a Syntax<'a>>,
    arena: &'a Arena<Syntax<'a>>,
) {
    // 1. Rename pre-pass.
    let rename_map = detect_renames(lhs, rhs, level);
    if !rename_map.is_empty() {
        apply_renames(lhs, &rename_map, arena);
        // RHS already uses the canonical names; no rewrite needed.
    }

    // 2. Language-specific node-level normalisation.
    if let Some(normalizer) = get_normalizer(language) {
        normalize_roots(lhs, arena, normalizer.as_ref(), None, level);
        normalize_roots(rhs, arena, normalizer.as_ref(), None, level);
    }
}

fn get_normalizer(language: guess::Language) -> Option<Box<dyn SemanticNormalizer>> {
    use guess::Language::*;
    match language {
        Rust => Some(Box::new(rust::RustNormalizer)),
        JavaScript | JavascriptJsx => Some(Box::new(javascript::JavaScriptNormalizer)),
        C => Some(Box::new(c::CNormalizer)),
        CPlusPlus => Some(Box::new(c::CPlusPlusNormalizer)),
        Python => Some(Box::new(python::PythonNormalizer)),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Per-node walker
// ─────────────────────────────────────────────────────────────────────────────

fn normalize_roots<'a>(
    roots: &mut Vec<&'a Syntax<'a>>,
    arena: &'a Arena<Syntax<'a>>,
    normalizer: &dyn SemanticNormalizer,
    parent: Option<&'a Syntax<'a>>,
    level: SemanticLevel,
) {
    for root in roots {
        *root = normalize_node(root, parent, arena, normalizer, level);
    }
}

fn normalize_node<'a>(
    node: &'a Syntax<'a>,
    parent: Option<&'a Syntax<'a>>,
    arena: &'a Arena<Syntax<'a>>,
    normalizer: &dyn SemanticNormalizer,
    level: SemanticLevel,
) -> &'a Syntax<'a> {
    // ── Step 1: recurse into children bottom-up ──────────────────────────────
    //
    // All rules (language-specific and algebraic) see fully-normalised children
    // by the time they fire on the parent.  In particular, constant-folding
    // propagates naturally: `(1 + 2) * x` has its sub-expression folded to
    // `3 * x` before the `*` node is visited.
    let node = match node {
        Syntax::List {
            open_content,
            open_position,
            children,
            close_content,
            close_position,
            ts_node_kind,
            ..
        } => {
            let new_children: Vec<_> = children
                .iter()
                .map(|c| normalize_node(c, Some(node), arena, normalizer, level))
                .collect();

            let changed = new_children
                .iter()
                .zip(children.iter())
                .any(|(a, b)| !std::ptr::eq(*a, *b));

            if changed {
                Syntax::new_list(
                    arena,
                    open_content,
                    open_position.clone(),
                    new_children,
                    close_content,
                    close_position.clone(),
                    ts_node_kind,
                )
            } else {
                node
            }
        }
        Syntax::Atom { .. } => node,
    };

    // ── Step 2: language-specific normalizer ─────────────────────────────────
    //
    // Handles idiom-specific patterns: null unification, De Morgan rewriting,
    // macro canonicalization, bool-match sentinels, etc.
    let node = normalizer
        .normalize(node, parent, arena, level)
        .unwrap_or(node);

    // ── Step 3: cross-language algebraic normalizations ───────────────────────
    //
    // Handles mathematical properties that are universal regardless of language:
    // constant folding, identity elements, self-cancellation, commutativity,
    // idempotency, and (at Advanced level) absorption.
    //
    // These run *after* the language-specific normalizer so that, for example,
    // a C `binary_expression { a, "==", b }` that was NOT consumed by
    // `c_compare_canon` (e.g. because it is not a simple comparison) can still
    // be sorted for commutativity by the algebraic pass.
    algebraic::normalize_algebraic(node, arena, level)
        .unwrap_or(node)
}

// ─────────────────────────────────────────────────────────────────────────────
// Rename detection (Advanced only)
// ─────────────────────────────────────────────────────────────────────────────

/// Detect likely renames by comparing identifier counts on both sides.
///
/// # Limitations
///
/// This is a simple count-based heuristic that can produce false positives
/// (e.g., when two different identifiers happen to appear the same number
/// of times) or false negatives (when a renamed identifier's count changes
/// due to other edits).  It is intended only for the Advanced level, where
/// the user accepts that some semantic equivalences may be hidden.
///
/// The algorithm:
/// 1. Count occurrences of each identifier on both sides.
/// 2. For each identifier on LHS that does not have the same count on RHS,
///    look for an identifier on RHS with exactly the same count.
/// 3. If exactly one such candidate exists, assume it's a rename.
///
/// This simple approach avoids more complex context analysis, which would
/// require a full syntax diff and is out of scope for the rename pass.
fn detect_renames<'a>(
    lhs: &[&'a Syntax<'a>],
    rhs: &[&'a Syntax<'a>],
    level: SemanticLevel,
) -> RenameMap {
    if level == SemanticLevel::None || level == SemanticLevel::Basic {
        return RenameMap::new();
    }

    let mut lhs_counts: HashMap<&str, usize> = HashMap::new();
    let mut rhs_counts: HashMap<&str, usize> = HashMap::new();

    for &node in lhs {
        count_identifiers(node, &mut lhs_counts);
    }
    for &node in rhs {
        count_identifiers(node, &mut rhs_counts);
    }

    let mut rename_map = RenameMap::new();

    for (&lhs_name, &lhs_count) in &lhs_counts {
        if let Some(&rhs_count) = rhs_counts.get(lhs_name) {
            if lhs_count == rhs_count {
                continue;
            }
        }

        let rhs_names = find_candidates_by_count(&rhs_counts, lhs_count);
        if rhs_names.len() == 1 {
            rename_map.insert(lhs_name.to_string(), rhs_names[0].to_string());
        }
    }

    rename_map
}

fn count_identifiers<'a>(node: &'a Syntax<'a>, counts: &mut HashMap<&'a str, usize>) {
    match node {
        Syntax::Atom { content, ts_node_kind, .. } => {
            if is_identifier(content, ts_node_kind) {
                *counts.entry(content.as_str()).or_insert(0) += 1;
            }
        }
        Syntax::List { children, .. } => {
            for child in children {
                count_identifiers(child, counts);
            }
        }
    }
}

fn is_identifier(content: &str, ts_node_kind: &str) -> bool {
    if ts_node_kind.is_empty() {
        return false;
    }
    !content.is_empty()
        && content.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_')
        && !is_keyword(content)
}

fn is_keyword(s: &str) -> bool {
    matches!(
        s,
        "if" | "else" | "for" | "while" | "return" | "int" | "void" | "char" | "const"
            | "struct" | "typedef" | "NULL" | "sizeof" | "printf" | "fprintf" | "memset"
            | "assert" | "true" | "false" | "nullptr"
    )
}

fn find_candidates_by_count<'a>(
    counts: &'a HashMap<&str, usize>,
    target: usize,
) -> Vec<&'a str> {
    counts
        .iter()
        .filter(|(_, &count)| count == target)
        .map(|(&name, _)| name)
        .collect()
}

fn apply_renames<'a>(
    roots: &mut Vec<&'a Syntax<'a>>,
    rename_map: &RenameMap,
    arena: &'a Arena<Syntax<'a>>,
) {
    for root in roots {
        *root = apply_renames_node(root, rename_map, arena);
    }
}

fn apply_renames_node<'a>(
    node: &'a Syntax<'a>,
    rename_map: &RenameMap,
    arena: &'a Arena<Syntax<'a>>,
) -> &'a Syntax<'a> {
    match node {
        Syntax::Atom {
            content,
            ts_node_kind,
            ..
        } => {
            if let Some(new_name) = rename_map.get(content.as_str()) {
                return Syntax::new_atom(
                    arena,
                    vec![],
                    new_name.clone(),
                    AtomKind::Normal,
                    ts_node_kind,
                );
            }
            node
        }
        Syntax::List {
            open_content,
            open_position,
            children,
            close_content,
            close_position,
            ts_node_kind,
            ..
        } => {
            let new_children: Vec<_> = children
                .iter()
                .map(|c| apply_renames_node(c, rename_map, arena))
                .collect();

            let changed = new_children
                .iter()
                .zip(children.iter())
                .any(|(a, b)| !std::ptr::eq(*a, *b));

            if changed {
                Syntax::new_list(
                    arena,
                    open_content,
                    open_position.clone(),
                    new_children,
                    close_content,
                    close_position.clone(),
                    ts_node_kind,
                )
            } else {
                node
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers for normalizers
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn synth_atom<'a>(
    arena: &'a Arena<Syntax<'a>>,
    content: &str,
) -> &'a Syntax<'a> {
    Syntax::new_atom(arena, vec![], content.to_owned(), AtomKind::Normal, "")
}

pub(crate) fn synth_list<'a>(
    arena: &'a Arena<Syntax<'a>>,
    open: &str,
    children: Vec<&'a Syntax<'a>>,
    close: &str,
) -> &'a Syntax<'a> {
    Syntax::new_list(
        arena,
        open,
        vec![],
        children,
        close,
        vec![],
        "",
    )
}

pub(crate) fn atom_content<'a>(node: &'a Syntax<'a>) -> Option<&'a str> {
    match node {
        Syntax::Atom { content, .. } => Some(content.as_str()),
        Syntax::List { .. } => None,
    }
}

pub(crate) fn list_children<'a>(node: &'a Syntax<'a>) -> Option<&'a [&'a Syntax<'a>]> {
    match node {
        Syntax::List { children, .. } => Some(children.as_slice()),
        Syntax::Atom { .. } => None,
    }
}

pub(crate) fn node_kind(node: &Syntax<'_>) -> &'static str {
    match node {
        Syntax::Atom { ts_node_kind, .. } | Syntax::List { ts_node_kind, .. } => {
            ts_node_kind
        }
    }
}

pub(crate) fn list_open<'a>(node: &'a Syntax<'a>) -> Option<&'a str> {
    match node {
        Syntax::List { open_content, .. } => Some(open_content.as_str()),
        Syntax::Atom { .. } => None,
    }
}

pub(crate) fn list_close<'a>(node: &'a Syntax<'a>) -> Option<&'a str> {
    match node {
        Syntax::List { close_content, .. } => Some(close_content.as_str()),
        Syntax::Atom { .. } => None,
    }
}

pub(crate) fn non_punct_children<'a>(
    node: &'a Syntax<'a>,
) -> Option<Vec<&'a Syntax<'a>>> {
    let children = list_children(node)?;
    Some(
        children
            .iter()
            .copied()
            .filter(|c| !is_punct(c))
            .collect(),
    )
}

fn is_punct<'a>(node: &'a Syntax<'a>) -> bool {
    atom_content(node).map_or(false, |c| matches!(c, "," | ";"))
}

pub(crate) fn parent_kind<'a>(parent: Option<&'a Syntax<'a>>) -> &'static str {
    parent.map_or("", node_kind)
}

/// Unwrap a single layer of parentheses, if present.
///
/// This handles both explicit parentheses (`( expr )`) and the special case
/// where a list node has no open/close content but a single child (used for
/// synthetic nodes).  It recurses only one level; callers may need to loop.
pub(crate) fn unwrap_paren<'a>(node: &'a Syntax<'a>) -> &'a Syntax<'a> {
    let mut current = node;
    loop {
        match current {
            Syntax::List {
                open_content,
                children,
                close_content,
                ..
            } => {
                let is_paren = (open_content == "(" && close_content == ")")
                    || (open_content.is_empty()
                        && children.len() == 1
                        && close_content.is_empty());
                if is_paren && children.len() == 1 {
                    current = children[0];
                    continue;
                }
            }
            Syntax::Atom { .. } => {}
        }
        break;
    }
    current
}

pub(crate) fn is_list_kind(node: &Syntax<'_>, kind: &str) -> bool {
    matches!(node, Syntax::List { ts_node_kind, .. } if *ts_node_kind == kind)
}

pub(crate) fn is_atom_kind(node: &Syntax<'_>, kind: &str) -> bool {
    matches!(node, Syntax::Atom { ts_node_kind, .. } if *ts_node_kind == kind)
}

pub(crate) fn is_atom_one_of<'a>(node: &'a Syntax<'a>, strs: &[&str]) -> bool {
    atom_content(node).map_or(false, |c| strs.contains(&c))
}

pub(crate) fn find_child_by_kind<'a>(
    node: &'a Syntax<'a>,
    kind: &str,
) -> Option<&'a Syntax<'a>> {
    list_children(node)?.iter().find(|c| node_kind(c) == kind).copied()
}