//! Data types that track the change state for syntax nodes.
use crate::hash::DftHashMap;
use crate::parse::syntax::{Syntax, SyntaxId};

#[derive(PartialEq, Eq, Clone, Copy)]
pub(crate) enum ChangeKind<'a> {
    /// This node is shallowly unchanged. For lists, this means that
    /// the delimiters match, but there may still be some differences
    /// in the children between LHS and RHS.
    Unchanged(&'a Syntax<'a>),
    /// Similarity percentage (0-100) from Levenshtein comparison.
    ReplacedComment(&'a Syntax<'a>, &'a Syntax<'a>, u8),
    /// Similarity percentage (0-100) from Levenshtein comparison.
    ReplacedString(&'a Syntax<'a>, &'a Syntax<'a>, u8),
    Novel,
}

#[derive(Debug, Default)]
pub(crate) struct ChangeMap<'a> {
    changes: DftHashMap<SyntaxId, ChangeKind<'a>>,
}

impl<'a> ChangeMap<'a> {
    pub(crate) fn insert(&mut self, node: &'a Syntax<'a>, ck: ChangeKind<'a>) {
        self.changes.insert(node.id(), ck);
    }
    pub(crate) fn get(&self, node: &Syntax<'a>) -> Option<ChangeKind<'a>> {
        self.changes.get(&node.id()).copied()
    }
}

pub(crate) fn insert_deep_unchanged<'a>(
    node: &'a Syntax<'a>,
    opposite_node: &'a Syntax<'a>,
    change_map: &mut ChangeMap<'a>,
) {
    change_map.insert(node, ChangeKind::Unchanged(opposite_node));
    
    // Only recurse if both nodes are Lists with matching structure.
    // The semantic normalizer may create nodes with different types
    // (e.g., List vs Atom) for canonical forms, so we can't assume
    // both sides will always match.
    match (node, opposite_node) {
        (
            Syntax::List {
                children: node_children,
                ..
            },
            Syntax::List {
                children: opposite_children,
                ..
            },
        ) => {
            // Only recurse if both lists have the same number of children.
            // If the normalizer changed the structure, treat children as novel.
            if node_children.len() == opposite_children.len() {
                for (child, opposite_child) in node_children.iter().zip(opposite_children) {
                    insert_deep_unchanged(child, opposite_child, change_map);
                }
            }
        }
        (Syntax::Atom { .. }, Syntax::Atom { .. }) => {
            // Atoms have no children, nothing to recurse into.
        }
        // Mismatched types (List vs Atom) - this can happen when the
        // semantic normalizer creates canonical forms that change node types.
        // The parent nodes are marked as Unchanged, but we don't recurse.
        _ => {}
    }
}

pub(crate) fn insert_deep_novel<'a>(node: &'a Syntax<'a>, change_map: &mut ChangeMap<'a>) {
    change_map.insert(node, ChangeKind::Novel);
    if let Syntax::List { children, .. } = node {
        for child in children.iter() {
            insert_deep_novel(child, change_map);
        }
    }
}