//! Semantic normalizer for Rust.
//!
//! Uses `ts_node_kind` for reliable pattern matching.  Parent context restricts
//! patterns to positions where they are unambiguously equivalent.
//!
//! **Basic level rules:**
//!   - `ptr::null()` / `ptr::null_mut()` / `0 as *const T` → `__null_ptr__`
//!   - `vec![…]` → `__vec__[…]`
//!   - `x.clone()` on Copy-type literals → `x`
//!   - `Default::default()` / `T::default()` → `__default__`
//!   - `bool` match on two arms → canonical `__bool_match__(x, t, f)`
//!   - `if x { t } else { f }` → canonical `__bool_match__(x, t, f)`
//!     - **Branch inversion**: `if !x { f } else { t }` → same canonical form.
//!       The positive condition is always used; branches are swapped when the
//!       condition is a unary `!` negation, so `if !a { Y } else { X }` and
//!       `if a { X } else { Y }` produce identical trees.
//!   - `println!` / `print!` → `__println__` (stdout only)
//!   - `eprintln!` / `eprint!` → `__eprintln__` (stderr; kept separate)
//!   - `if a { if b { body } }` → `__if_and__(a, b, body)`
//!     **Short-circuit equivalence**: a doubly-nested `if` with no `else`
//!     clauses is semantically identical to `if a && b { body }`.  Both forms
//!     evaluate `b` only when `a` is true; the canonical `__if_and__` sentinel
//!     unifies them.
//!
//! **Advanced level rules:**
//!   - `x.to_string()` ↔ `format!("{}", x)` ↔ `String::from(x)` — string
//!     coercion unification.  Advanced because the forms differ in allocation
//!     path and some have different trait bounds (`Display` vs `Into<String>`).
//!   - `opt.unwrap_or_else(|| v)` ↔ `opt.unwrap_or(v)` — only valid for
//!     zero-arity closures with no captures; Advanced because closures with
//!     captures may affect evaluation order.
//!
//! Tree-sitter node kinds used (tree-sitter-rust):
//!   call_expression, method_call_expression, match_expression, match_arm,
//!   macro_invocation, closure_expression, arguments, block, field_identifier,
//!   if_expression, unary_expression

use typed_arena::Arena;
use crate::options::SemanticLevel;
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
        level: SemanticLevel,
    ) -> Option<&'a Syntax<'a>> {
        // Basic rules.
        normalize_null_ptr(node, arena)
            .or_else(|| normalize_vec_macro(node, arena))
            .or_else(|| normalize_clone_of_copy(node, arena))
            .or_else(|| normalize_default_call(node, arena))
            .or_else(|| normalize_bool_match(node, arena))
            // normalize_bool_if subsumes branch inversion (see below).
            .or_else(|| normalize_bool_if(node, arena))
            // Nested if → if-and canonicalization (short-circuit equivalence).
            .or_else(|| normalize_nested_if_and(node, arena))
            .or_else(|| normalize_print_macro(node, arena))
            // Advanced rules.
            .or_else(|| {
                if level == SemanticLevel::Advanced {
                    normalize_to_string(node, arena)
                        .or_else(|| normalize_unwrap_or_else(node, arena))
                } else {
                    None
                }
            })
    }
}

// ─── null pointer ─────────────────────────────────────────────────────────────
/// `ptr::null()` / `std::ptr::null_mut()` / `0 as *const T` → `__null_ptr__`
fn normalize_null_ptr<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name.ends_with("::null") || fn_name.ends_with("::null_mut") {
                return Some(synth_atom(arena, "__null_ptr__"));
            }
        }
    }

    if let Some(c) = atom_content(node) {
        if c.ends_with("::null()") || c.ends_with("::null_mut()") || c.starts_with("0 as *") {
            return Some(synth_atom(arena, "__null_ptr__"));
        }
    }
    None
}

// ─── vec! macro ───────────────────────────────────────────────────────────────
/// `vec![…]` → `__vec__[…]`   (macro_invocation with name "vec")
fn normalize_vec_macro<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "macro_invocation" {
        if let Some(open) = list_open(node) {
            if open == "vec![" {
                let children = list_children(node)?;
                return Some(Syntax::new_list(
                    arena,
                    "__vec__[",
                    vec![],
                    children.to_vec(),
                    "]",
                    vec![],
                    "",
                ));
            }
        }
    }
    None
}

// ─── clone of Copy ────────────────────────────────────────────────────────────
/// `x.clone()` where x is a Copy-type literal → `x`
fn normalize_clone_of_copy<'a>(
    node: &'a Syntax<'a>,
    _arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "method_call_expression" {
        if let Some(method_node) = find_child_by_kind(node, "field_identifier") {
            if atom_content(method_node)? == "clone" {
                let children = list_children(node)?;
                if let Some(receiver) = children.first() {
                    let receiver_content = atom_content(receiver)?;
                    let is_copy_literal = receiver_content.parse::<i64>().is_ok()
                        || receiver_content == "true"
                        || receiver_content == "false"
                        || receiver_content.starts_with("'")
                        || receiver_content == "()";
                    if is_copy_literal {
                        return Some(receiver);
                    }
                }
            }
        }
    }
    None
}

// ─── Default::default() ───────────────────────────────────────────────────────
/// `Default::default()` / `T::default()` → `__default__`
fn normalize_default_call<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name.ends_with("::default") || fn_name == "Default::default" {
                return Some(synth_atom(arena, "__default__"));
            }
        }
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Branch inversion helpers
// ─────────────────────────────────────────────────────────────────────────────

/// True when `node` is a `unary_expression` whose operator is `!`.
fn is_negation<'a>(node: &'a Syntax<'a>) -> bool {
    if node_kind(node) != "unary_expression" {
        return false;
    }
    list_children(node)
        .and_then(|ch| ch.first().copied())
        .and_then(atom_content)
        .map_or(false, |op| op == "!")
}

/// If `node` is `!inner`, return `inner`; otherwise `None`.
fn negation_inner<'a>(node: &'a Syntax<'a>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "unary_expression" {
        return None;
    }
    let ch = list_children(node)?;
    if ch.len() < 2 {
        return None;
    }
    if atom_content(ch[0])? != "!" {
        return None;
    }
    Some(ch[1])
}

// ─── bool if ─────────────────────────────────────────────────────────────────
/// Unify `if`-`else` expressions with their `match bool` counterparts, and
/// additionally canonicalise branch-inverted forms.
///
/// **Standard form:**
/// ```rust
/// if x { t } else { f }  →  __bool_match__(x, t, f)
/// ```
///
/// **Branch inversion** (new):
/// ```rust
/// if !x { f } else { t }  →  __bool_match__(x, t, f)
/// ```
///
/// When the condition is a unary `!negation`, the condition is unwrapped and
/// the true/false branches are swapped, producing the same canonical tree as
/// the non-negated form.  This makes the following pair show *no* diff:
///
/// ```rust
/// // Version A               // Version B
/// if !cond { Y } else { X }  if cond { X } else { Y }
/// ```
///
/// Guard clauses (`if let`, multi-arm `if`/`else if` chains) are intentionally
/// NOT collapsed.
fn normalize_bool_if<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "if_expression" {
        return None;
    }

    let children = list_children(node)?;

    // Find condition: first child that is neither a keyword nor a block.
    let condition = children.iter().find(|c| {
        let k = node_kind(c);
        k != "if" && k != "else" && k != "block"
    })?;

    // Exactly two blocks: if-branch and else-branch.
    let blocks: Vec<_> = children.iter().filter(|c| node_kind(c) == "block").collect();
    if blocks.len() != 2 {
        return None;
    }

    // Reject `else if` chains: the else clause would contain an if_expression.
    if children.iter().any(|c| node_kind(c) == "if_expression") {
        return None;
    }

    // Branch inversion: if the condition is `!inner`, use `inner` as the
    // canonical condition and swap the two branches.
    //
    // `condition` and `blocks[N]` are `&&'a Syntax<'a>` (double-ref) because
    // they come from `children.iter()` which yields `&&'a Syntax<'a>`.  We
    // dereference them once here so the tuple elements are uniformly
    // `&'a Syntax<'a>`, matching `synth_list`'s expected element type.
    let (canonical_cond, true_block, false_block): (&Syntax, &Syntax, &Syntax) =
        if is_negation(*condition) {
            match negation_inner(*condition) {
                Some(inner) => (inner, *blocks[1], *blocks[0]),
                None        => (*condition, *blocks[0], *blocks[1]),
            }
        } else {
            (*condition, *blocks[0], *blocks[1])
        };

    Some(synth_list(
        arena,
        "__bool_match__(",
        vec![
            canonical_cond,
            synth_atom(arena, ", "),
            true_block,
            synth_atom(arena, ", "),
            false_block,
        ],
        ")",
    ))
}

// ─── bool match ───────────────────────────────────────────────────────────────
/// `match x { true => t, false => f }` and `match x { false => f, true => t }`
/// → `__bool_match__(x, t, f)`
///
/// Guard clauses (match arms with `if` guards) are intentionally NOT collapsed:
/// they carry semantic meaning beyond a simple boolean branch.
fn normalize_bool_match<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "match_expression" {
        return None;
    }
    let children = list_children(node)?;
    // First child is the scrutinee; remaining children are match arms.
    let scrutinee = children.first()?;
    let arms: Vec<_> = children
        .iter()
        .skip(1)
        .filter(|c| node_kind(c) == "match_arm")
        .collect();
    if arms.len() != 2 {
        return None;
    }

    // Each arm: [pattern, "=>", body].  Reject arms with guards ("if" token).
    let arm_parts = |arm: &&'a Syntax<'a>| -> Option<(&'a str, &'a Syntax<'a>)> {
        let ac = list_children(arm)?;
        // Guard check: any child is an "if_guard" node.
        if ac.iter().any(|c| node_kind(c) == "if_guard") {
            return None;
        }
        let pat = atom_content(ac.first().copied()?)?;
        let body = ac.last().copied()?;
        Some((pat, body))
    };

    let (p0, b0) = arm_parts(&arms[0])?;
    let (p1, b1) = arm_parts(&arms[1])?;

    let (true_body, false_body) = match (p0, p1) {
        ("true", "false") => (b0, b1),
        ("false", "true") => (b1, b0),
        _ => return None,
    };

    Some(synth_list(
        arena,
        "__bool_match__(",
        vec![
            scrutinee,
            synth_atom(arena, ", "),
            true_body,
            synth_atom(arena, ", "),
            false_body,
        ],
        ")",
    ))
}

// ─── nested if → if-and (short-circuit equivalence) ──────────────────────────
/// Canonicalise doubly-nested `if` expressions with no `else` clause.
///
/// ```rust
/// if a { if b { body } }   ≡   if a && b { body }
/// ```
///
/// Both forms evaluate `b` only when `a` is truthy, and execute `body` only
/// when both conditions hold.  The normalisation produces the sentinel:
///
/// ```
/// __if_and__(a, b, body)
/// ```
///
/// **Safety**: the equivalence holds exactly when:
/// 1. The outer `if` has **no else** clause.
/// 2. The inner `if` has **no else** clause.
/// 3. The inner block contains **exactly one** statement (the inner `if`).
///
/// Condition 3 prevents false positives when multiple statements share the
/// outer `if` scope — those extra statements would be lost in the
/// short-circuit form.
///
/// **Note on `else if` chains**: rejected by the check that neither the outer
/// nor inner `if_expression` children contain a nested `if_expression` sibling
/// to their block (the `else if` continuation).
fn normalize_nested_if_and<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "if_expression" {
        return None;
    }
    let children = list_children(node)?;

    // Outer if must have NO else (exactly one block child).
    let blocks: Vec<_> = children.iter().filter(|c| node_kind(c) == "block").collect();
    if blocks.len() != 1 {
        return None;
    }

    // Reject `else if` chains at the outer level.
    if children.iter().any(|c| node_kind(c) == "if_expression") {
        return None;
    }

    let outer_cond = children.iter().find(|c| {
        let k = node_kind(c);
        k != "if" && k != "else" && k != "block"
    })?;

    let outer_block = blocks[0];
    let block_stmts = list_children(*outer_block)?;

    // The outer block must contain exactly one statement — the inner if.
    if block_stmts.len() != 1 {
        return None;
    }

    let inner = block_stmts[0];
    if node_kind(inner) != "if_expression" {
        return None;
    }

    let inner_children = list_children(inner)?;

    // Inner if must also have NO else.
    let inner_blocks: Vec<_> = inner_children.iter().filter(|c| node_kind(c) == "block").collect();
    if inner_blocks.len() != 1 {
        return None;
    }

    // Reject `else if` chains at the inner level.
    if inner_children.iter().any(|c| node_kind(c) == "if_expression") {
        return None;
    }

    let inner_cond = inner_children.iter().find(|c| {
        let k = node_kind(c);
        k != "if" && k != "else" && k != "block"
    })?;

    let inner_body = inner_blocks[0];

    // Dereference all `&&Syntax` pointers once so synth_list receives
    // uniformly `&'a Syntax<'a>` elements.
    Some(synth_list(
        arena,
        "__if_and__(",
        vec![
            *outer_cond,
            synth_atom(arena, ", "),
            *inner_cond,
            synth_atom(arena, ", "),
            *inner_body,
        ],
        ")",
    ))
}

// ─── print macros ─────────────────────────────────────────────────────────────
/// Canonicalise print macros by stream:
///   `println!` / `print!` → `__println__(…)`   (stdout)
///   `eprintln!` / `eprint!` → `__eprintln__(…)`  (stderr)
///
/// Stdout and stderr are kept separate because changing print stream is a real
/// semantic change (e.g. redirecting error output to stdout breaks log parsing).
fn normalize_print_macro<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "macro_invocation" {
        return None;
    }
    let open = list_open(node)?;
    let canonical = match open {
        s if s.starts_with("println!(") || s.starts_with("print!(") => "__println__",
        s if s.starts_with("eprintln!(") || s.starts_with("eprint!(") => "__eprintln__",
        _ => return None,
    };

    let children = list_children(node)?;
    Some(synth_list(
        arena,
        &format!("{}(", canonical),
        children.to_vec(),
        ")",
    ))
}

// ─── to_string (Advanced) ────────────────────────────────────────────────────
/// `x.to_string()` ↔ `format!("{}", x)` ↔ `String::from(x)` → `__to_string__(x)`
///
/// Advanced: `.to_string()` requires `Display`, `String::from` requires
/// `Into<String>`, and `format!` requires `Display`.  These have subtly
/// different trait bounds and allocation paths; collapsing them may hide
/// a meaningful API-choice change.
fn normalize_to_string<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    // x.to_string()
    if node_kind(node) == "method_call_expression" {
        if let Some(method) = find_child_by_kind(node, "field_identifier") {
            if atom_content(method)? == "to_string" {
                let children = list_children(node)?;
                let receiver = children.first()?;
                return Some(synth_list(arena, "__to_string__(", vec![receiver], ")"));
            }
        }
    }

    // format!("{}", x)
    if node_kind(node) == "macro_invocation" {
        if let Some(open) = list_open(node) {
            if open.starts_with("format!(") {
                let children = list_children(node)?;
                let args = non_punct_children(node)?;
                // Expect exactly: "{}", x
                if args.len() == 2 {
                    let fmt_str = atom_content(args[0])?;
                    if fmt_str == "\"{}\"" || fmt_str == "\"{}\\n\"" {
                        let subject = args[1];
                        return Some(synth_list(arena, "__to_string__(", vec![subject], ")"));
                    }
                }
                // format!("{x}") form
                if children.len() == 1 {
                    if let Some(s) = atom_content(children[0]) {
                        if s.starts_with("\"{") && s.ends_with("}\"") {
                            let inner = &s[2..s.len() - 2];
                            return Some(synth_list(
                                arena,
                                "__to_string__(",
                                vec![synth_atom(arena, inner)],
                                ")",
                            ));
                        }
                    }
                }
            }
        }
    }

    // String::from(x)
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name == "String::from" {
                let args = children.get(1)?;
                let arg_children = non_punct_children(args)?;
                if arg_children.len() == 1 {
                    return Some(synth_list(
                        arena,
                        "__to_string__(",
                        vec![arg_children[0]],
                        ")",
                    ));
                }
            }
        }
    }
    None
}

// ─── unwrap_or_else (Advanced) ────────────────────────────────────────────────
/// `opt.unwrap_or_else(|| v)` ↔ `opt.unwrap_or(v)` → `__unwrap_or__(opt, v)`
///
/// Advanced: only valid when the closure has zero parameters and no captures.
/// A closure `|| expr` with captures has different evaluation semantics than
/// `unwrap_or(expr)` in cases where `expr` has side effects.
fn normalize_unwrap_or_else<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "method_call_expression" {
        return None;
    }
    let method = find_child_by_kind(node, "field_identifier")?;
    let method_name = atom_content(method)?;
    let children = list_children(node)?;
    let receiver = children.first()?;

    if method_name == "unwrap_or" {
        let args = children.get(2)?;
        let arg_ch = non_punct_children(args)?;
        if arg_ch.len() == 1 {
            return Some(synth_list(
                arena,
                "__unwrap_or__(",
                vec![receiver, synth_atom(arena, ", "), arg_ch[0]],
                ")",
            ));
        }
    }

    if method_name == "unwrap_or_else" {
        let args = children.get(2)?;
        let arg_ch = non_punct_children(args)?;
        if arg_ch.len() == 1 {
            // closure must be `|| expr` — zero params, no captures.
            let closure = arg_ch[0];
            if node_kind(closure) == "closure_expression" {
                let cch = list_children(closure)?;
                // closure_expression children: ["|", "|", body] or [params_node, body]
                // Check that the param list is empty.
                let params_node = cch.first()?;
                let params_empty = list_children(params_node)
                    .map_or(false, |pc| pc.is_empty());
                if params_empty && cch.len() >= 2 {
                    let body = cch.last()?;
                    return Some(synth_list(
                        arena,
                        "__unwrap_or__(",
                        vec![receiver, synth_atom(arena, ", "), body],
                        ")",
                    ));
                }
            }
        }
    }
    None
}