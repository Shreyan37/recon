//! Semantic normalisation: rewrite syntactically different but semantically
//! identical subtrees into a canonical form *before* diffing.
//!
//! # Four improvements over the initial version
//!
//! 1. **`ts_node_kind` awareness** — normalizers match on the real tree-sitter
//!    node kind string (`"match_expression"`, `"call_expression"`, …) instead
//!    of guessing from `open_content`.
//!
//! 2. **Parent context** — `SemanticNormalizer::normalize` now receives the
//!    parent node so context-sensitive patterns can be matched safely.
//!
//! 3. **Rename pre-pass** — before the per-node walk, `detect_renames` scans
//!    both trees and finds identifier atoms that appear N times on one side
//!    and N times on the other with different text (N≥2).  Both sides are
//!    rewritten to the canonical (RHS) name so the diff sees them as Unchanged.
//!
//! 4. **Synthetic nodes carry `""` as `ts_node_kind`** — they never
//!    accidentally match real tree-sitter node kinds.

use std::collections::HashMap;
use typed_arena::Arena;

use crate::parse::guess_language as guess;
use crate::parse::syntax::{AtomKind, Syntax};

pub mod c;
pub mod javascript;
pub mod python;
pub mod rust;

// ─────────────────────────────────────────────────────────────────────────────
// Public trait
// ─────────────────────────────────────────────────────────────────────────────

/// A language-specific semantic normalizer.
///
/// `normalize` is called **bottom-up** after children have already been
/// processed.
pub trait SemanticNormalizer: Send + Sync {
    fn normalize<'a>(
        &self,
        node: &'a Syntax<'a>,
        parent: Option<&'a Syntax<'a>>,
        arena: &'a Arena<Syntax<'a>>,
    ) -> Option<&'a Syntax<'a>>;

    fn language(&self) -> guess::Language;
}

// ─────────────────────────────────────────────────────────────────────────────
// Rename map
// ─────────────────────────────────────────────────────────────────────────────

/// A bijective rename mapping: LHS identifier content → RHS (canonical) content.
pub type RenameMap = HashMap<String, String>;

/// Scan both trees and detect high-confidence identifier renames.
///
/// Conditions for a rename `old → new`:
/// - Both look like plain identifiers (no spaces, starts with letter/`_`).
/// - `old` only appears on LHS, `new` only appears on RHS (strictly disjoint).
/// - Both appear the same number of times N, with N ≥ 2.
/// - The bijection is unambiguous: exactly one LHS-only id and one RHS-only id
///   share count N.
pub fn detect_renames<'a>(lhs: &[&'a Syntax<'a>], rhs: &[&'a Syntax<'a>]) -> RenameMap {
    let mut lhs_counts: HashMap<&str, usize> = HashMap::new();
    let mut rhs_counts: HashMap<&str, usize> = HashMap::new();

    collect_id_counts(lhs, &mut lhs_counts);
    collect_id_counts(rhs, &mut rhs_counts);

    let lhs_only: HashMap<&str, usize> = lhs_counts
        .iter()
        .filter(|(k, _)| !rhs_counts.contains_key(*k))
        .map(|(k, v)| (*k, *v))
        .collect();

    let rhs_only: HashMap<&str, usize> = rhs_counts
        .iter()
        .filter(|(k, _)| !lhs_counts.contains_key(*k))
        .map(|(k, v)| (*k, *v))
        .collect();

    // Group each side by count.
    let mut lhs_by_count: HashMap<usize, Vec<&str>> = HashMap::new();
    for (id, count) in &lhs_only {
        if *count >= 2 {
            lhs_by_count.entry(*count).or_default().push(id);
        }
    }
    let mut rhs_by_count: HashMap<usize, Vec<&str>> = HashMap::new();
    for (id, count) in &rhs_only {
        if *count >= 2 {
            rhs_by_count.entry(*count).or_default().push(id);
        }
    }

    let mut rename_map = RenameMap::new();
    for (count, lhs_ids) in &lhs_by_count {
        if lhs_ids.len() != 1 {
            continue;
        }
        if let Some(rhs_ids) = rhs_by_count.get(count) {
            if rhs_ids.len() == 1 {
                rename_map.insert(lhs_ids[0].to_owned(), rhs_ids[0].to_owned());
            }
        }
    }
    rename_map
}

fn collect_id_counts<'a>(nodes: &[&'a Syntax<'a>], counts: &mut HashMap<&'a str, usize>) {
    for n in nodes {
        collect_ids_rec(n, counts);
    }
}

fn collect_ids_rec<'a>(node: &'a Syntax<'a>, counts: &mut HashMap<&'a str, usize>) {
    match node {
        Syntax::Atom { content, kind, ts_node_kind, .. } => {
            if is_identifier_like(content, *kind, ts_node_kind) {
                *counts.entry(content.as_str()).or_insert(0) += 1;
            }
        }
        Syntax::List { children, .. } => {
            for c in children {
                collect_ids_rec(c, counts);
            }
        }
    }
}

fn is_identifier_like(content: &str, kind: AtomKind, ts_kind: &str) -> bool {
    if kind != AtomKind::Normal {
        return false;
    }
    // Reject known non-identifier ts_node_kinds.
    const IDENTIFIER_KINDS: &[&str] = &[
        "identifier",
        "field_identifier",
        "type_identifier",
        "property_identifier",
        "name",
        "variable_name",
        "symbol",
        "",  // synthetic / unknown — allow through, content check below guards it
    ];
    if !ts_kind.is_empty() && !IDENTIFIER_KINDS.contains(&ts_kind) {
        return false;
    }
    let mut chars = content.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    content.chars().all(|c| c.is_alphanumeric() || c == '_')
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry points
// ─────────────────────────────────────────────────────────────────────────────

/// Normalise both sides in-place.  Called from `main.rs` after
/// `to_syntax_with_limit` and before diffing.
pub fn normalize_all<'a>(
    language: guess::Language,
    lhs: &mut Vec<&'a Syntax<'a>>,
    rhs: &mut Vec<&'a Syntax<'a>>,
    arena: &'a Arena<Syntax<'a>>,
) {
    // 1. Rename pre-pass.
    let rename_map = detect_renames(lhs, rhs);
    if !rename_map.is_empty() {
        apply_renames(lhs, &rename_map, arena);
        // RHS already uses the canonical names; no rewrite needed.
    }

    // 2. Language-specific node-level normalization.
    if let Some(normalizer) = get_normalizer(language) {
        normalize_roots(lhs, arena, normalizer.as_ref(), None);
        normalize_roots(rhs, arena, normalizer.as_ref(), None);
    }
}

fn get_normalizer(language: guess::Language) -> Option<Box<dyn SemanticNormalizer>> {
    use guess::Language::*;
    match language {
        Rust => Some(Box::new(rust::RustNormalizer)),
        JavaScript | JavascriptJsx => Some(Box::new(javascript::JavaScriptNormalizer)),
        TypeScript | TypeScriptTsx => Some(Box::new(javascript::TypeScriptNormalizer)),
        Python => Some(Box::new(python::PythonNormalizer)),
        C => Some(Box::new(c::CNormalizer)),
        CPlusPlus => Some(Box::new(c::CPlusPlusNormalizer)),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rename application
// ─────────────────────────────────────────────────────────────────────────────

fn apply_renames<'a>(
    roots: &mut Vec<&'a Syntax<'a>>,
    rename_map: &RenameMap,
    arena: &'a Arena<Syntax<'a>>,
) {
    for node in roots.iter_mut() {
        *node = apply_renames_node(node, rename_map, arena);
    }
}

fn apply_renames_node<'a>(
    node: &'a Syntax<'a>,
    rename_map: &RenameMap,
    arena: &'a Arena<Syntax<'a>>,
) -> &'a Syntax<'a> {
    match node {
        Syntax::Atom { content, kind, ts_node_kind, .. } => {
            if let Some(canonical) = rename_map.get(content.as_str()) {
                return Syntax::new_atom(arena, vec![], canonical.clone(), *kind, ts_node_kind);
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
            let changed = new_children.iter().zip(children.iter()).any(|(a, b)| !std::ptr::eq(*a, *b));
            if changed {
                Syntax::new_list(arena, open_content, open_position.clone(),
                    new_children, close_content, close_position.clone(), ts_node_kind)
            } else {
                node
            }
        }
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
) {
    for node in roots.iter_mut() {
        *node = normalize_node(node, parent, arena, normalizer);
    }
}

pub(crate) fn normalize_node<'a>(
    node: &'a Syntax<'a>,
    parent: Option<&'a Syntax<'a>>,
    arena: &'a Arena<Syntax<'a>>,
    normalizer: &dyn SemanticNormalizer,
) -> &'a Syntax<'a> {
    // Bottom-up: children first.
    let node = match node {
        Syntax::List {
            open_content, open_position, children,
            close_content, close_position, ts_node_kind, ..
        } => {
            let new_children: Vec<_> = children
                .iter()
                .map(|c| normalize_node(c, Some(node), arena, normalizer))
                .collect();
            let changed = new_children.iter().zip(children.iter()).any(|(a, b)| !std::ptr::eq(*a, *b));
            if changed {
                Syntax::new_list(arena, open_content, open_position.clone(),
                    new_children, close_content, close_position.clone(), ts_node_kind)
            } else {
                node
            }
        }
        Syntax::Atom { .. } => node,
    };

    normalizer.normalize(node, parent, arena).unwrap_or(node)
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers for normalizers
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn synth_atom<'a>(arena: &'a Arena<Syntax<'a>>, content: &str) -> &'a Syntax<'a> {
    Syntax::new_atom(arena, vec![], content.to_owned(), AtomKind::Normal, "")
}

pub(crate) fn synth_keyword<'a>(arena: &'a Arena<Syntax<'a>>, content: &str) -> &'a Syntax<'a> {
    Syntax::new_atom(arena, vec![], content.to_owned(), AtomKind::Keyword, "")
}

pub(crate) fn synth_list<'a>(
    arena: &'a Arena<Syntax<'a>>,
    open: &str,
    children: Vec<&'a Syntax<'a>>,
    close: &str,
) -> &'a Syntax<'a> {
    Syntax::new_list(arena, open, vec![], children, close, vec![], "")
}

pub(crate) fn atom_content<'a>(node: &'a Syntax<'a>) -> Option<&'a str> {
    match node {
        Syntax::Atom { content, .. } => Some(content.as_str()),
        Syntax::List { .. } => None,
    }
}

pub(crate) fn node_kind(node: &Syntax) -> &'static str {
    match node {
        Syntax::Atom { ts_node_kind, .. } | Syntax::List { ts_node_kind, .. } => ts_node_kind,
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

pub(crate) fn list_children<'a, 'b>(
    node: &'b Syntax<'a>,
) -> Option<&'b Vec<&'a Syntax<'a>>> {
    match node {
        Syntax::List { children, .. } => Some(children),
        Syntax::Atom { .. } => None,
    }
}

/// Fixed: Added explicit lifetime parameters
pub(crate) fn is_atom<'a>(node: &'a Syntax<'a>, content: &str) -> bool {
    atom_content(node).map_or(false, |c| c == content)
}
pub(crate) fn is_atom_one_of<'a>(node: &'a Syntax<'a>, options: &[&str]) -> bool {
    atom_content(node).map_or(false, |c| options.contains(&c))
}
/// True if node is a List with the given ts_node_kind.
pub(crate) fn is_list_kind(node: &Syntax, kind: &str) -> bool {
    matches!(node, Syntax::List { ts_node_kind, .. } if *ts_node_kind == kind)
}

/// True if node is an Atom with the given ts_node_kind.
pub(crate) fn is_atom_kind(node: &Syntax, kind: &str) -> bool {
    matches!(node, Syntax::Atom { ts_node_kind, .. } if *ts_node_kind == kind)
}

/// Return the first child with the given ts_node_kind.
pub(crate) fn find_child_by_kind<'a, 'b>(
    node: &'b Syntax<'a>,
    kind: &str,
) -> Option<&'b &'a Syntax<'a>> {
    list_children(node)?.iter().find(|c| node_kind(c) == kind)
}

/// Return child at index i.
pub(crate) fn child_at<'a, 'b>(node: &'b Syntax<'a>, i: usize) -> Option<&'b &'a Syntax<'a>> {
    list_children(node)?.get(i)
}

/// Children that are not pure-punctuation atoms.
pub(crate) fn non_punct_children<'a, 'b>(
    node: &'b Syntax<'a>,
) -> Option<Vec<&'b &'a Syntax<'a>>> {
    Some(
        list_children(node)?
            .iter()
            .filter(|c| {
                atom_content(*c)
                    .map_or(true, |s: &str| !s.chars().all(|ch: char| ch.is_ascii_punctuation()))
            })
            .collect(),
    )
}

/// Return the parent's ts_node_kind, or "" if no parent.
pub(crate) fn parent_kind<'a>(parent: Option<&'a Syntax<'a>>) -> &'static str {
    parent.map_or("", node_kind)
}