//! Semantic normalizer for Python.
//!
//! Tree-sitter node kinds used (tree-sitter-python):
//!   call_expression, unary_expression, binary_expression, attribute,
//!   list, dictionary, set, comparison_operator

use typed_arena::Arena;
use crate::parse::guess_language as guess;
use crate::parse::syntax::Syntax;
use super::{
    atom_content, list_children, node_kind, non_punct_children,
    synth_atom, synth_list, SemanticNormalizer,
};

pub struct PythonNormalizer;

impl SemanticNormalizer for PythonNormalizer {
    fn language(&self) -> guess::Language {
        guess::Language::Python
    }

    fn normalize<'a>(
        &self,
        node: &'a Syntax<'a>,
        _parent: Option<&'a Syntax<'a>>,
        arena: &'a Arena<Syntax<'a>>,
    ) -> Option<&'a Syntax<'a>> {
        normalize_none_check(node, arena)
            .or_else(|| normalize_len_check(node, arena))
            .or_else(|| normalize_print_call(node, arena))
            .or_else(|| normalize_list_comprehension(node, arena))
    }
}

// ─── None check ──────────────────────────────────────────────────────────────
///  `x is None` / `x == None` → `__is_none__(x)`
fn normalize_none_check<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "comparison_operator" {
        let children = non_punct_children(node)?;
        if children.len() == 3 {
            let op = atom_content(children[1])?;
            if op == "is" || op == "==" {
                let rhs = atom_content(children[2])?;
                if rhs == "None" {
                    return Some(synth_list(arena, "__is_none__(", vec![children[0]], ")"));
                }
            }
        }
    }
    None
}

// ─── len() check ─────────────────────────────────────────────────────────────
///  `len(x) == 0` / `len(x) > 0` → `__is_empty__(x)` / `__is_nonempty__(x)`
fn normalize_len_check<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "comparison_operator" {
        let children = non_punct_children(node)?;
        if children.len() == 3 {
            let lhs = children[0];
            if node_kind(lhs) == "call_expression" {
                let call_ch = list_children(lhs)?;
                if let Some(fn_name) = atom_content(call_ch.first().copied()?) {
                    if fn_name == "len" {
                        let arg_list = call_ch.get(1)?;
                        let arg_ch = list_children(arg_list)?;
                        let target = arg_ch.first().copied()?;
                        let op = atom_content(children[1])?;
                        let rhs = atom_content(children[2])?;

                        if op == "==" && rhs == "0" {
                            return Some(synth_list(arena, "__is_empty__(", vec![target], ")"));
                        }
                        if (op == ">" || op == "!=") && rhs == "0" {
                            return Some(synth_list(arena, "__is_nonempty__(", vec![target], ")"));
                        }
                    }
                }
            }
        }
    }
    None
}

// ─── print() call ────────────────────────────────────────────────────────────
///  `print(…)` → `__print__(…)`
fn normalize_print_call<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        let fn_name = atom_content(children.first().copied()?)?;
        if fn_name == "print" {
            let args = children.get(1).copied();
            return Some(synth_list(arena, "__print__(", args.map_or(vec![], |a| vec![a]), ")"));
        }
    }
    None
}

// ─── List comprehension ──────────────────────────────────────────────────────
///  `[x for x in y]` → `__list_comp__(y, x)`
fn normalize_list_comprehension<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "list_comprehension" {
        let children = list_children(node)?;
        if children.len() >= 2 {
            let expr = children[0];
            let comprehension = children[1];
            if node_kind(comprehension) == "for_in_clause" {
                let comp_ch = list_children(comprehension)?;
                if comp_ch.len() >= 3 {
                    let target = comp_ch[0];
                    let iterable = comp_ch[2];
                    return Some(synth_list(arena, "__list_comp__(", vec![iterable, synth_atom(arena, ", "), expr], ")"));
                }
            }
        }
    }
    None
}