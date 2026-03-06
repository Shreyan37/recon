//! Semantic normalizer for Rust.
//!
//! Uses `ts_node_kind` for reliable pattern matching.  Parent context restricts
//! patterns to positions where they are unambiguously equivalent.
//!
//! Tree-sitter node kinds used (tree-sitter-rust):
//!   call_expression, method_call_expression, match_expression, match_arm,
//!   macro_invocation, closure_expression, arguments, block, field_identifier

use typed_arena::Arena;
use crate::parse::guess_language as guess;
use crate::parse::syntax::Syntax;
use super::{
    atom_content, find_child_by_kind, list_children, list_open,
    node_kind, non_punct_children, synth_atom, synth_list, SemanticNormalizer,
};

pub struct RustNormalizer;

impl SemanticNormalizer for RustNormalizer {
    fn language(&self) -> guess::Language {
        guess::Language::Rust
    }

    fn normalize<'a>(
        &self,
        node: &'a Syntax<'a>,
        _parent: Option<&'a Syntax<'a>>,
        arena: &'a Arena<Syntax<'a>>,
    ) -> Option<&'a Syntax<'a>> {
        normalize_null_ptr(node, arena)
            .or_else(|| normalize_vec_macro(node, arena))
            .or_else(|| normalize_clone_of_copy(node, arena))
            .or_else(|| normalize_default_call(node, arena))
            .or_else(|| normalize_bool_match(node, arena))
            .or_else(|| normalize_print_macro(node, arena))
    }
}

///  `ptr::null()` / `std::ptr::null_mut()` / `0 as *const T` → `__null_ptr__`
fn normalize_null_ptr<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    // As a call_expression List: function name is the first child
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name.ends_with("::null") || fn_name.ends_with("::null_mut") {
                return Some(synth_atom(arena, "__null_ptr__"));
            }
        }
    }

    // As a flat atom (already collapsed)
    if let Some(c) = atom_content(node) {
        if c.ends_with("::null()") || c.ends_with("::null_mut()") || c.starts_with("0 as *") {
            return Some(synth_atom(arena, "__null_ptr__"));
        }
    }
    None
}

///  `vec![…]` → `__vec__[…]`   (macro_invocation with name "vec")
fn normalize_vec_macro<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "macro_invocation" {
        if let Some(open) = list_open(node) {
            if open == "vec![" {
                let children = list_children(node)?;
                return Some(Syntax::new_list(arena, "__vec__[", vec![], children.clone(), "]", vec![], ""));
            }
        }
    }
    None
}

///  `x.clone()` where x is a Copy-type literal → `x`
fn normalize_clone_of_copy<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "method_call_expression" {
        // Find method name child (field_identifier)
        if let Some(method_node) = find_child_by_kind(node, "field_identifier") {
            if atom_content(method_node)? == "clone" {
                // Receiver is the first child (self expression)
                let children = list_children(node)?;
                if let Some(receiver) = children.first() {
                    if let Some(content) = atom_content(receiver) {
                        if is_copy_literal(content) {
                            return Some(receiver);
                        }
                    }
                }
            }
        }
    }

    // Flat atom form: "42.clone()"
    if let Some(c) = atom_content(node) {
        if let Some(recv) = c.strip_suffix(".clone()") {
            if is_copy_literal(recv) {
                return Some(synth_atom(arena, recv));
            }
        }
    }
    None
}

///  `T::default()` / `Default::default()` → `__default__()`
fn normalize_default_call<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name.ends_with("::default") {
                return Some(synth_atom(arena, "__default__()"));
            }
        }
    }

    if let Some(c) = atom_content(node) {
        if c == "Default::default()" || c.ends_with("::default()") {
            return Some(synth_atom(arena, "__default__()"));
        }
    }
    None
}

///  `match x { true => a, false => b }` and arm-order swapped
/// → canonical `__bool_match__(x, a, b)`
fn normalize_bool_match<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "match_expression" {
        return None;
    }
    let children = non_punct_children(node)?;
    if children.len() != 3 {
        return None;
    }
    let scrutinee = children[0];
    let arms = &children[1..];

    fn arm_parts<'a>(arm: &'a Syntax<'a>) -> Option<(&'a str, &'a Syntax<'a>)> {
        if node_kind(arm) != "match_arm" {
            return None;
        }
        let ch = list_children(arm)?;
        let pat = atom_content(ch.first().copied()?)?;
        let body = ch.last().copied()?;
        Some((pat, body))
    }

    let (p0, b0) = arm_parts(arms[0])?;
    let (p1, b1) = arm_parts(arms[1])?;
    let (true_b, false_b) = match (p0, p1) {
        ("true", "false") => (b0, b1),
        ("false", "true") => (b1, b0),
        _ => return None,
    };

    Some(synth_list(
        arena, "__bool_match__(",
        vec![scrutinee, synth_atom(arena, ", "), true_b, synth_atom(arena, ", "), false_b],
        ")",
    ))
}

///  `println!` / `eprintln!` → `__println__`
///  `print!` / `eprint!` → `__print__`
fn normalize_print_macro<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "macro_invocation" {
        return None;
    }
    let open = list_open(node)?;
    let canonical = match open {
        "println!(" | "eprintln!(" => "__println__!(",
        "print!(" | "eprint!(" => "__print__!(",
        _ => return None,
    };
    let children = list_children(node)?;
    let args: Vec<_> = if children.len() > 1 { children[1..].to_vec() } else { vec![] };
    Some(Syntax::new_list(arena, canonical, vec![], args, ")", vec![], ""))
}

fn is_copy_literal(s: &str) -> bool {
    s.parse::<i64>().is_ok()
        || s.parse::<u64>().is_ok()
        || s.parse::<f64>().is_ok()
        || s == "true"
        || s == "false"
        || (s.starts_with('\'') && s.ends_with('\''))
}