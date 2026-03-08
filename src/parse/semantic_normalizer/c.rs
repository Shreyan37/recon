//! Semantic normalizers for C and C++.
//!
//! Basic level rules:
//! - `NULL` / `((void*)0)` / `nullptr` → `__null__`
//! - `sizeof(T)` / `sizeof T` → `__sizeof__(T)`
//! - `printf(…)` / `fprintf(stdout, …)` → `__printf__(…)`
//! - `assert(cond)` → `__assert__(cond)`
//! - `memset(p, 0, sizeof(*p))` → `__zero_init__(p)`
//! - `!!x` → `x` (double negation elimination)
//! - `a CMP b` → `__CMP__(a, b)` for comparison operators (sentinel form)
//! - `!(sentinel)` → negated sentinel (pairs with comparison canonicalization,
//!   so `!(a == b)` ↔ `a != b`, `!(a < b)` ↔ `a >= b`, etc.)
//! - `!(a && b)` ↔ `!a || !b` → `__demorgan_or__(a, b)`
//! - `!(a || b)` ↔ `!a && !b` → `__demorgan_and__(a, b)`
//!
//! De Morgan scope limitation:
//! Form 2 (`!a || !b`) only collapses when `a` and `b` remain as real
//! `unary_expression` nodes after bottom-up normalization. When `a` or `b`
//! are comparisons they are already converted to canonical sentinels (not
//! `unary_expression` nodes) by the time the `||` / `&&` node is visited, so
//! Form 2 cannot detect the De Morgan pattern in that case. Form 1
//! (`!(a && b)`) always works regardless of what `a` and `b` are.
//!
//! Advanced level rules:
//! - `std::make_shared<T>(…)` ↔ `std::make_unique<T>(…)` ↔ `new T(…)`
//!   → `__smart_ptr__(T, …)`. Advanced because shared/unique ownership
//!   semantics differ meaningfully.
//! - `static_cast<T>(x)` ↔ `(T)x` → `__cast__(T, x)`
//! - `std::cout << x` → `__cout__(x)`
//! - `std::cerr << x` → `__cerr__(x)`
//!
//! # Bottom-up interaction between rules
//!
//! The normalizer walks the tree bottom-up, so by the time a parent node
//! is visited, all its children are already normalized. This creates a
//! necessary ordering contract between the three new comparison rules:
//!
//! 1. `c_compare_canon` fires on `binary_expression { a, CMP, b }` and
//!    produces a synthetic sentinel list `__CMP__(a, b)`.
//!
//! 2. When `!(a == b)` is later visited, its child `a == b` has already
//!    been replaced by `__eq__(a, b)` (a synthetic list). The `!` node's
//!    child is therefore a sentinel, and `c_negate_sentinel` fires to
//!    flip it to `__ne__(a, b)`.
//!
//! 3. For De Morgan Form 1, `!(a && b)` — the `&&` binary_expression is NOT
//!    a comparison, so `c_compare_canon` does not fire on it. It remains a
//!    real `binary_expression` node when `c_demorgan` inspects it.
//!
//! Tree-sitter node kinds used:
//! C: call_expression, argument_list, unary_expression, binary_expression,
//!    sizeof_expression, null, pointer_declarator
//! C++: call_expression, cast_expression, static_cast, new_expression,
//!      null_literal, using_declaration

use typed_arena::Arena;
use crate::options::SemanticLevel;
use crate::parse::guess_language as guess;
use crate::parse::syntax::Syntax;
use super::{
    atom_content,
    list_children, list_open, node_kind, non_punct_children,
    synth_atom, synth_list, unwrap_paren, SemanticNormalizer,
};

pub struct CNormalizer;
pub struct CPlusPlusNormalizer;

impl SemanticNormalizer for CNormalizer {
    fn language(&self) -> guess::Language {
        guess::Language::C
    }

    fn normalize<'a>(
        &self,
        node: &'a Syntax<'a>,
        parent: Option<&'a Syntax<'a>>,
        arena: &'a Arena<Syntax<'a>>,
        _level: SemanticLevel,
    ) -> Option<&'a Syntax<'a>> {
        c_shared(node, parent, arena)
    }
}

impl SemanticNormalizer for CPlusPlusNormalizer {
    fn language(&self) -> guess::Language {
        guess::Language::CPlusPlus
    }

    fn normalize<'a>(
        &self,
        node: &'a Syntax<'a>,
        parent: Option<&'a Syntax<'a>>,
        arena: &'a Arena<Syntax<'a>>,
        level: SemanticLevel,
    ) -> Option<&'a Syntax<'a>> {
        // Basic rules (shared with C + C++-specific).
        c_shared(node, parent, arena)
            .or_else(|| cpp_null(node, arena))
            .or_else(|| cpp_cast(node, arena))
            .or_else(|| cpp_cout(node, arena))
            .or_else(|| cpp_cerr(node, arena))
            .or_else(|| cpp_string_ctor(node, arena))
            .or_else(|| cpp_move(node, arena))
            // Advanced rules.
            .or_else(|| {
                if level == SemanticLevel::Advanced {
                    cpp_new(node, arena).or_else(|| cpp_make_shared(node, arena))
                } else {
                    None
                }
            })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared C / C++
// ─────────────────────────────────────────────────────────────────────────────

fn c_shared<'a>(
    node: &'a Syntax<'a>,
    parent: Option<&'a Syntax<'a>>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    c_null(node, parent, arena)
        .or_else(|| c_sizeof(node, arena))
        .or_else(|| c_printf(node, arena))
        .or_else(|| c_assert(node, arena))
        .or_else(|| c_memset_zero(node, arena))
        // Null comparison – must come before c_compare_canon.
        .or_else(|| c_null_comparison(node, arena))
        .or_else(|| c_double_neg(node, parent, arena))
        .or_else(|| c_compare_canon(node, arena))
        .or_else(|| c_negate_sentinel(node, arena))
        .or_else(|| c_demorgan(node, arena))
}

// Helper to strip any number of wrapping parentheses.
fn strip_all_parens<'a>(node: &'a Syntax<'a>) -> &'a Syntax<'a> {
    let mut current = node;
    loop {
        match current {
            Syntax::List {
                open_content,
                children,
                close_content,
                ..
            } if open_content == "(" && close_content == ")" && children.len() == 1 => {
                current = children[0];
            }
            _ => break,
        }
    }
    current
}

// ─── NULL (C context) ────────────────────────────────────────────────────────
/// `NULL` / `(void*)0` / `((void*)0)` → `__null__`
fn c_null<'a>(
    node: &'a Syntax<'a>,
    _parent: Option<&'a Syntax<'a>>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    let node = strip_all_parens(node);

    // Atom form: bare `NULL` identifier.
    if let Some(c) = atom_content(node) {
        if c == "NULL" {
            return Some(synth_atom(arena, "__null__"));
        }
    }

    // C++ null node kind (tree-sitter-cpp).
    if node_kind(node) == "null_literal" {
        return Some(synth_atom(arena, "__null__"));
    }

    // Structural form: `(void*)0` parsed as cast_expression.
    if node_kind(node) == "cast_expression" {
        let children = non_punct_children(node)?;
        let non_paren: Vec<_> = children
            .iter()
            .filter(|c| atom_content(*c).map_or(true, |s| s != "(" && s != ")"))
            .collect();
        if non_paren.len() >= 2 {
            let type_node = non_paren[0];
            let value_node = non_paren[non_paren.len() - 1];
            if is_void_pointer_type(type_node) {
                if let Some(v) = atom_content(value_node) {
                    if v == "0" {
                        return Some(synth_atom(arena, "__null__"));
                    }
                }
            }
        }
    }

    // Bottom-up form: parenthesized_expression whose sole child is already `__null__`.
    if node_kind(node) == "parenthesized_expression" {
        let children = list_children(node)?;
        if children.len() == 1 {
            if atom_content(children[0]) == Some("__null__") {
                return Some(synth_atom(arena, "__null__"));
            }
        }
    }

    None
}

/// True if `node` represents a void-pointer type: `void*`, `void**`, etc.
fn is_void_pointer_type<'a>(node: &'a Syntax<'a>) -> bool {
    let node = if list_open(node).map_or(false, |o| o == "(") {
        match list_children(node).and_then(|ch| {
            if ch.len() == 1 { Some(ch[0]) } else { None }
        }) {
            Some(inner) => inner,
            None => node,
        }
    } else {
        node
    };

    if let Some(children) = list_children(node) {
        let has_void = children.iter().any(|c| {
            if atom_content(c) == Some("void") {
                return true;
            }
            if node_kind(c) == "primitive_type" {
                if let Some(sub) = list_children(c) {
                    return sub.iter().any(|sc| atom_content(sc) == Some("void"));
                }
            }
            false
        });
        let has_ptr = children.iter().any(|c| {
            if atom_content(c) == Some("*") {
                return true;
            }
            let k = node_kind(c);
            k == "pointer_declarator" || k == "abstract_pointer_declarator"
        });
        if has_void && has_ptr {
            return true;
        }
    }

    if let Some(s) = atom_content(node) {
        let s = s.replace(' ', "");
        return s.starts_with("void*");
    }

    false
}

// ─── Null comparison ─────────────────────────────────────────────────────────
/// Canonicalize null comparisons regardless of operand order.
/// `ptr == NULL` / `NULL == ptr` → `__is_null__(ptr)`
/// `ptr != NULL` / `NULL != ptr` → `__is_not_null__(ptr)`
fn c_null_comparison<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "binary_expression" {
        return None;
    }
    let ch = list_children(node)?;
    if ch.len() < 3 {
        return None;
    }
    let op = atom_content(ch[1])?;
    if op != "==" && op != "!=" {
        return None;
    }

    // Determine which operand is a null constant (NULL, nullptr, or __null__)
    let left = ch[0];
    let right = ch[2];
    let left_is_null = matches_null(left);
    let right_is_null = matches_null(right);

    if left_is_null && right_is_null {
        // Both null: e.g. `NULL == NULL`. Normalize to `true` / `false`.
        return Some(synth_atom(arena, if op == "==" { "__true__" } else { "__false__" }));
    }

    let (subject, negated) = if left_is_null {
        (right, op == "!=")
    } else if right_is_null {
        (left, op == "!=")
    } else {
        return None;
    };

    let sentinel = if negated {
        "__is_not_null__("
    } else {
        "__is_null__("
    };
    Some(synth_list(arena, sentinel, vec![subject], ")"))
}

/// True if `node` represents a null constant (NULL, nullptr, or already normalized __null__)
fn matches_null<'a>(node: &'a Syntax<'a>) -> bool {
    match node_kind(node) {
        "null_literal" => true,
        _ => {
            if let Some(c) = atom_content(node) {
                c == "NULL" || c == "nullptr" || c == "__null__"
            } else {
                false
            }
        }
    }
}

// ─── sizeof ───────────────────────────────────────────────────────────────────
/// `sizeof(T)` / `sizeof T` → `__sizeof__(T)`
fn c_sizeof<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "sizeof_expression" {
        let children = list_children(node)?;
        let type_node = children.iter().find(|c| {
            let k = node_kind(c);
            k != "sizeof" && k != "(" && k != ")"
        })?;
        let type_node = unwrap_paren(type_node);
        return Some(synth_list(arena, "__sizeof__(", vec![type_node], ")"));
    }

    if let Some(c) = atom_content(node) {
        if let Some(inner) = c.strip_prefix("sizeof(").and_then(|s| s.strip_suffix(')')) {
            return Some(synth_list(
                arena,
                "__sizeof__(",
                vec![synth_atom(arena, inner)],
                ")",
            ));
        }
    }

    None
}

// ─── printf ───────────────────────────────────────────────────────────────────
/// `printf(…)` / `fprintf(stdout, …)` → `__printf__(…)`
fn c_printf<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name == "printf" || fn_name == "fprintf" || fn_name == "sprintf" {
                let args_node = children.get(1).copied()?;
                let arg_ch = non_punct_children(args_node)?;

                let inner_args: Vec<&Syntax> = if fn_name == "fprintf" {
                    if arg_ch.len() < 2 { return None; }
                    arg_ch[1..].iter().copied().collect()
                } else {
                    arg_ch.iter().copied().collect()
                };

                let inner = synth_list(arena, "(", inner_args, ")");
                return Some(synth_list(arena, "__printf__(", vec![inner], ")"));
            }
        }
    }

    None
}

// ─── assert ───────────────────────────────────────────────────────────────────
/// `assert(cond)` → `__assert__(cond)`
fn c_assert<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "call_expression" {
        return None;
    }

    let children = list_children(node)?;
    if atom_content(children.first().copied()?)? != "assert" {
        return None;
    }

    let args = children.get(1)?;
    let arg_ch = non_punct_children(args)?;
    if arg_ch.len() == 1 {
        return Some(synth_list(arena, "__assert__(", vec![arg_ch[0]], ")"));
    }

    None
}

// ─── memset zero ─────────────────────────────────────────────────────────────
/// `memset(p, 0, sizeof(*p))` / `memset(p, 0, sizeof(T))` → `__zero_init__(p)`
fn c_memset_zero<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "call_expression" {
        return None;
    }

    let children = list_children(node)?;
    if atom_content(children.first().copied()?)? != "memset" {
        return None;
    }

    let args = children.get(1)?;
    let arg_ch = non_punct_children(args)?;
    if arg_ch.len() != 3 {
        return None;
    }

    if atom_content(arg_ch[1])? != "0" {
        return None;
    }

    let third = arg_ch[2];
    let is_sizeof = node_kind(third) == "sizeof_expression"
        || atom_content(third).map_or(false, |s| s.starts_with("sizeof"))
        || list_open(third).map_or(false, |o| o.starts_with("__sizeof__"));

    if !is_sizeof {
        return None;
    }

    let ptr = arg_ch[0];
    Some(synth_list(arena, "__zero_init__(", vec![ptr], ")"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Double negation, comparison canonicalization, negated sentinel, De Morgan
// ─────────────────────────────────────────────────────────────────────────────

// ─── Double negation ─────────────────────────────────────────────────────────
/// `!!x` → `x`
fn c_double_neg<'a>(
    node: &'a Syntax<'a>,
    _parent: Option<&'a Syntax<'a>>,
    _arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "unary_expression" {
        return None;
    }

    let ch = list_children(node)?;
    if ch.len() < 2 || atom_content(ch[0])? != "!" {
        return None;
    }

    let inner = unwrap_paren(ch[1]);

    if node_kind(inner) != "unary_expression" {
        return None;
    }

    let inner_ch = list_children(inner)?;
    if inner_ch.len() < 2 || atom_content(inner_ch[0])? != "!" {
        return None;
    }

    Some(unwrap_paren(inner_ch[1]))
}

// ─── Comparison canonicalization ─────────────────────────────────────────────
/// Canonicalise comparison operators to typed sentinels.
///
/// **Commutativity of `==` and `!=`:**
/// When both operands are atoms (identifiers or literals), `==` and `!=` are
/// sorted into lexicographic order so that `b == a` and `a == b` produce
/// identical sentinel nodes — `__eq__(a, b)` in both cases.
///
/// Ordered comparisons (`<`, `>`, `<=`, `>=`) are **not** sorted: the
/// relationship `a > b ≡ b < a` would require also renaming the sentinel
/// (`__gt__` ↔ `__lt__`), which would break the `c_negate_sentinel` mapping.
/// That equivalence is therefore left to a future dedicated pass.
fn c_compare_canon<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "binary_expression" {
        return None;
    }

    let ch = list_children(node)?;
    if ch.len() < 3 {
        return None;
    }

    let lhs = ch[0];
    let rhs = ch[2];
    let op = atom_content(ch[1])?;

    // For equality/inequality, sort atom operands into canonical order so that
    // `b == a` and `a == b` produce the same sentinel tree.
    let (final_lhs, final_rhs) = match op {
        "==" | "!=" => sort_atoms_canonical(lhs, rhs),
        _           => (lhs, rhs),
    };

    let sentinel = comparison_sentinel_open(op)?;
    Some(synth_list(
        arena,
        sentinel,
        vec![final_lhs, synth_atom(arena, ", "), final_rhs],
        ")",
    ))
}

/// If both `lhs` and `rhs` are atoms, return them in lexicographic order
/// (smaller content first).  Otherwise return them unchanged.
///
/// This provides a stable canonical ordering for commutative comparisons
/// (`==`, `!=`) without affecting ordered comparisons.
fn sort_atoms_canonical<'a>(
    lhs: &'a Syntax<'a>,
    rhs: &'a Syntax<'a>,
) -> (&'a Syntax<'a>, &'a Syntax<'a>) {
    match (atom_content(lhs), atom_content(rhs)) {
        (Some(lk), Some(rk)) if lk > rk => (rhs, lhs),
        _ => (lhs, rhs),
    }
}

#[inline]
fn comparison_sentinel_open(op: &str) -> Option<&'static str> {
    match op {
        "==" => Some("__eq__("),
        "!=" => Some("__ne__("),
        "<"  => Some("__lt__("),
        ">"  => Some("__gt__("),
        "<=" => Some("__le__("),
        ">=" => Some("__ge__("),
        _ => None,
    }
}

// ─── Negated sentinel ─────────────────────────────────────────────────────────
fn c_negate_sentinel<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "unary_expression" {
        return None;
    }

    let ch = list_children(node)?;
    if ch.len() < 2 || atom_content(ch[0])? != "!" {
        return None;
    }

    let inner = unwrap_paren(ch[1]);
    let open = list_open(inner)?;
    let neg_open = negate_comparison_sentinel(open)?;

    let inner_ch = list_children(inner)?.to_vec();
    Some(synth_list(arena, neg_open, inner_ch, ")"))
}

#[inline]
fn negate_comparison_sentinel(open: &str) -> Option<&'static str> {
    match open {
        "__eq__(" => Some("__ne__("),
        "__ne__(" => Some("__eq__("),
        "__lt__(" => Some("__ge__("),
        "__gt__(" => Some("__le__("),
        "__le__(" => Some("__gt__("),
        "__ge__(" => Some("__lt__("),
        _ => None,
    }
}

// ─── De Morgan ───────────────────────────────────────────────────────────────
fn c_demorgan<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    // Form 1: !(a OP b)
    if node_kind(node) == "unary_expression" {
        let ch = list_children(node)?;
        if ch.len() < 2 || atom_content(ch[0])? != "!" {
            return None;
        }

        let inner = unwrap_paren(ch[1]);
        if node_kind(inner) != "binary_expression" {
            return None;
        }

        let bin_ch = list_children(inner)?;
        if bin_ch.len() < 3 {
            return None;
        }

        let op = atom_content(bin_ch[1])?;
        return match op {
            "&&" => Some(synth_list(
                arena,
                "__demorgan_or__(",
                vec![bin_ch[0], synth_atom(arena, ", "), bin_ch[2]],
                ")",
            )),
            "||" => Some(synth_list(
                arena,
                "__demorgan_and__(",
                vec![bin_ch[0], synth_atom(arena, ", "), bin_ch[2]],
                ")",
            )),
            _ => None,
        };
    }

    // Form 2: !a OP !b
    if node_kind(node) == "binary_expression" {
        let bin_ch = list_children(node)?;
        if bin_ch.len() < 3 {
            return None;
        }

        let op = atom_content(bin_ch[1])?;
        let sentinel = match op {
            "||" => "__demorgan_or__(",
            "&&" => "__demorgan_and__(",
            _ => return None,
        };

        let lhs_inner = extract_neg(bin_ch[0])?;
        let rhs_inner = extract_neg(bin_ch[2])?;

        return Some(synth_list(
            arena,
            sentinel,
            vec![lhs_inner, synth_atom(arena, ", "), rhs_inner],
            ")",
        ));
    }

    None
}

#[inline]
fn extract_neg<'a>(node: &'a Syntax<'a>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "unary_expression" {
        return None;
    }

    let ch = list_children(node)?;
    if ch.len() < 2 || atom_content(ch[0])? != "!" {
        return None;
    }

    Some(unwrap_paren(ch[1]))
}

// ─────────────────────────────────────────────────────────────────────────────
// C++ only
// ─────────────────────────────────────────────────────────────────────────────

/// `nullptr` / `NULL` → `__null__`
fn cpp_null<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "null_literal" {
        return Some(synth_atom(arena, "__null__"));
    }

    if let Some(c) = atom_content(node) {
        if c == "nullptr" {
            return Some(synth_atom(arena, "__null__"));
        }
    }

    None
}

/// `static_cast<T>(x)` / `(T)x` → `__cast__(T, x)`
fn cpp_cast<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "cast_expression" {
        let all = list_children(node)?;
        let children: Vec<_> = all
            .iter()
            .filter(|c| atom_content(*c).map_or(true, |s| s != "(" && s != ")"))
            .collect();
        if children.len() >= 2 {
            let ty = children[0];
            let expr = children[children.len() - 1];
            return Some(synth_list(
                arena,
                "__cast__(",
                vec![ty, synth_atom(arena, ", "), expr],
                ")",
            ));
        }
    }

    None
}

/// `std::cout << x` → `__cout__(x)`
fn cpp_cout<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "binary_expression" {
        let children = non_punct_children(node)?;
        if children.len() == 3 {
            let lhs = atom_content(children[0])?;
            let op = atom_content(children[1])?;
            if (lhs == "std::cout" || lhs == "cout") && op == "<<" {
                return Some(synth_list(arena, "__cout__(", vec![children[2]], ")"));
            }
        }
    }

    None
}

/// `std::cerr << x` → `__cerr__(x)`
fn cpp_cerr<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "binary_expression" {
        let children = non_punct_children(node)?;
        if children.len() == 3 {
            let lhs = atom_content(children[0])?;
            let op = atom_content(children[1])?;
            if (lhs == "std::cerr" || lhs == "cerr") && op == "<<" {
                return Some(synth_list(arena, "__cerr__(", vec![children[2]], ")"));
            }
        }
    }

    None
}

/// `new T(args)` → `__new__(T, args)` (Advanced)
fn cpp_new<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "new_expression" {
        let children = list_children(node)?;
        let non_new: Vec<_> = children
            .iter()
            .filter(|c| atom_content(*c).map_or(true, |s| s != "new"))
            .collect();
        if non_new.len() >= 1 {
            let ty = non_new[0];
            let args_children: Vec<_> = non_new.iter().skip(1).map(|c| **c).collect();
            let mut parts = vec![*ty];
            if !args_children.is_empty() {
                parts.push(synth_atom(arena, ", "));
                parts.extend(args_children);
            }
            return Some(synth_list(arena, "__new__(", parts, ")"));
        }
    }

    None
}

/// `std::string("x")` → `__string__("x")`
fn cpp_string_ctor<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name == "std::string" || fn_name == "string" {
                let args = children.get(1)?;
                let arg_ch = non_punct_children(args)?;
                if arg_ch.len() == 1 {
                    return Some(synth_list(arena, "__string__(", vec![arg_ch[0]], ")"));
                }
            }
        }
    }

    None
}

/// `std::move(x)` → `x`
fn cpp_move<'a>(
    node: &'a Syntax<'a>,
    _arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name == "std::move" || fn_name == "move" {
                let args = children.get(1)?;
                let arg_ch = non_punct_children(args)?;
                if arg_ch.len() == 1 {
                    return Some(arg_ch[0]);
                }
            }
        }
    }

    None
}

/// `std::make_shared<T>(…)` ↔ `std::make_unique<T>(…)` → `__smart_ptr__(T, …)` (Advanced)
fn cpp_make_shared<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "call_expression" {
        return None;
    }

    let children = list_children(node)?;
    let callee = atom_content(children.first().copied()?)?;
    let is_factory = callee.starts_with("std::make_shared")
        || callee.starts_with("std::make_unique")
        || callee.starts_with("make_shared")
        || callee.starts_with("make_unique");

    if !is_factory {
        return None;
    }

    let type_arg = callee
        .find('<')
        .and_then(|s| callee.rfind('>').map(|e| &callee[s + 1..e]))?;

    let args = children.get(1)?;
    let arg_ch = non_punct_children(args)?;
    let mut parts: Vec<&Syntax> = vec![synth_atom(arena, type_arg)];
    for a in arg_ch {
        parts.push(synth_atom(arena, ", "));
        parts.push(a);
    }

    Some(synth_list(arena, "__smart_ptr__(", parts, ")"))
}