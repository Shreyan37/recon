//! Cross-language algebraic and boolean property normalizations.
//!
//! Applied to every node in every language *after* the language-specific
//! normalizer has completed its bottom-up pass.  Because the walk is
//! bottom-up, children are already normalized when these rules fire, so
//! constant-folding propagates naturally through nested literals.
//!
//! # Properties implemented
//!
//! | Property               | Level    | Guard                                         |
//! |------------------------|----------|-----------------------------------------------|
//! | Constant folding       | Basic    | Both operands are integer literal atoms        |
//! | Identity elements      | Basic    | Identity side is `"0"` or `"1"` literal        |
//! | Inverse / self-cancel  | Basic    | Both sides are the **same** pure atom          |
//! | Commutativity `==`/`!=`| Basic    | Both operands are any pure atom                |
//! | Commutativity arith    | Basic    | Both operands are integer literal atoms        |
//! | Idempotency `&&`/`||`  | Basic    | Both sides are the **same** pure atom          |
//! | Absorption             | Advanced | Outer operand is a pure atom                  |
//!
//! # Context constraints
//!
//! **Floating-point** — identity/inverse rules only match the exact string
//! tokens `"0"` and `"1"`, never `"0.0"` or `"1.0"`, preventing false
//! positives with IEEE-754 edge-cases (`-0.0`, `NaN`, `Inf`).
//!
//! **Side effects** — commutativity and idempotency only fire when *all*
//! affected sub-expressions are leaf `Atom` nodes (identifiers or literals),
//! which cannot trigger side-effecting calls.  Function-call nodes are `List`
//! variants and are therefore excluded automatically.
//!
//! **Short-circuit evaluation** — idempotency (`a && a → a`) is guarded to
//! pure atoms for the same reason: silently dropping the second evaluation of
//! `f() && f()` could change observable program behaviour.
//!
//! # Node-kind coverage
//!
//! The helper [`binop_parts`] matches three tree-sitter node kinds:
//!
//! * `"binary_expression"` — C, C++, Rust, JavaScript
//! * `"binary_operator"`   — Python arithmetic (`+`, `-`, `*`, `/`, …)
//! * `"comparison_operator"` — Python comparisons (`==`, `!=`, `<`, …)

use typed_arena::Arena;
use crate::options::SemanticLevel;
use crate::parse::syntax::Syntax;
use super::{atom_content, list_children, node_kind, synth_atom};

// ─────────────────────────────────────────────────────────────────────────────
// Top-level entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Apply all algebraic normalizations in priority order.
///
/// Returns `Some(new_node)` when any rule fires; `None` when the node is
/// already in canonical form.
pub(super) fn normalize_algebraic<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
    level: SemanticLevel,
) -> Option<&'a Syntax<'a>> {
    // Basic rules — always applied.
    normalize_constant_fold(node, arena)
        .or_else(|| normalize_identity(node, arena))
        .or_else(|| normalize_inverse(node, arena))
        .or_else(|| normalize_commutative(node, arena))
        .or_else(|| normalize_idempotent(node, arena))
        // Advanced rules — gated to avoid hiding intentional refactors.
        .or_else(|| {
            if level == SemanticLevel::Advanced {
                normalize_absorption(node, arena)
            } else {
                None
            }
        })
}

// ─────────────────────────────────────────────────────────────────────────────
// Binary-expression structural helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Extract `(lhs, operator_str, rhs)` from any supported binary-expression
/// node kind.  Returns `None` for any other node shape.
fn binop_parts<'a>(
    node: &'a Syntax<'a>,
) -> Option<(&'a Syntax<'a>, &'a str, &'a Syntax<'a>)> {
    match node_kind(node) {
        "binary_expression" | "binary_operator" | "comparison_operator" => {}
        _ => return None,
    }
    let ch = list_children(node)?;
    // Strict 3-child check: [lhs, op_atom, rhs].
    if ch.len() != 3 {
        return None;
    }
    let op = atom_content(ch[1])?;
    Some((ch[0], op, ch[2]))
}

/// Rebuild a binary-expression node with new operands, preserving the original
/// `open_content`, `close_content`, and `ts_node_kind` so the resulting node
/// is structurally identical to the source node (only operand order changes).
fn rebuild_binop<'a>(
    original: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
    new_lhs: &'a Syntax<'a>,
    op_atom: &'a Syntax<'a>,
    new_rhs: &'a Syntax<'a>,
) -> &'a Syntax<'a> {
    match original {
        Syntax::List {
            open_content,
            open_position,
            close_content,
            close_position,
            ts_node_kind,
            ..
        } => Syntax::new_list(
            arena,
            open_content,
            open_position.clone(),
            vec![new_lhs, op_atom, new_rhs],
            close_content,
            close_position.clone(),
            ts_node_kind,
        ),
        _ => unreachable!("rebuild_binop: node must be a List"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Predicate helpers
// ─────────────────────────────────────────────────────────────────────────────

/// True when `node` is a leaf `Atom` whose content parses as a signed 64-bit
/// integer (e.g. `"0"`, `"42"`, `"-1"`).  Explicitly excludes float tokens
/// (`"0.0"`, `"1e3"`) to prevent IEEE-754 false positives.
///
/// The explicit `'a` lifetime is required because `Syntax<'a>` is invariant
/// over its lifetime parameter; using `&Syntax<'_>` would create two
/// independent lifetimes that `atom_content` cannot satisfy.
fn is_int_literal<'a>(node: &'a Syntax<'a>) -> bool {
    matches!(node, Syntax::Atom { content, .. } if content.parse::<i64>().is_ok())
}

fn int_val<'a>(node: &'a Syntax<'a>) -> Option<i64> {
    atom_content(node)?.parse().ok()
}

/// True when `node` is a leaf `Atom` (identifier, keyword, or literal).
/// List nodes (function calls, parenthesised expressions, etc.) return false.
fn is_pure_atom<'a>(node: &'a Syntax<'a>) -> bool {
    matches!(node, Syntax::Atom { .. })
}

/// True when both nodes are atoms with identical non-empty content.
fn atoms_equal<'a>(a: &'a Syntax<'a>, b: &'a Syntax<'a>) -> bool {
    match (atom_content(a), atom_content(b)) {
        (Some(ac), Some(bc)) => !ac.is_empty() && ac == bc,
        _ => false,
    }
}

fn is_literal_zero<'a>(node: &'a Syntax<'a>) -> bool {
    atom_content(node).map_or(false, |c| c == "0")
}

fn is_literal_one<'a>(node: &'a Syntax<'a>) -> bool {
    atom_content(node).map_or(false, |c| c == "1")
}

// ─────────────────────────────────────────────────────────────────────────────
// Rule 1 — Constant folding
// ─────────────────────────────────────────────────────────────────────────────

/// Evaluate `integer_literal OP integer_literal` at normalization time.
///
/// ```text
/// 3 + 4  →  7        10 - 3  →  7
/// 6 * 7  →  42       12 / 4  →  3   (exact only; 5/2 is NOT folded)
/// ```
///
/// Uses checked arithmetic; overflowing expressions are left unchanged.
/// Division is only folded when the quotient is exact (no remainder) to
/// avoid diverging from integer-truncation semantics.
fn normalize_constant_fold<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    let (lhs, op, rhs) = binop_parts(node)?;
    if !is_int_literal(lhs) || !is_int_literal(rhs) {
        return None;
    }
    let l = int_val(lhs)?;
    let r = int_val(rhs)?;
    let result: i64 = match op {
        "+" => l.checked_add(r)?,
        "-" => l.checked_sub(r)?,
        "*" => l.checked_mul(r)?,
        "/" if r != 0 && l % r == 0 => l / r,
        _ => return None,
    };
    Some(synth_atom(arena, &result.to_string()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Rule 2 — Identity elements
// ─────────────────────────────────────────────────────────────────────────────

/// Eliminate neutral elements that contribute nothing to the result.
///
/// ```text
/// x + 0  →  x        0 + x  →  x
/// x - 0  →  x
/// x * 1  →  x        1 * x  →  x
/// x / 1  →  x
/// x ** 1 →  x        (Python / Rust `**` exponentiation)
/// x ^ 0  →  x        0 ^ x  →  x   (bitwise XOR identity)
/// x | 0  →  x        0 | x  →  x   (bitwise OR identity)
/// ```
///
/// The non-identity operand `x` may be **any** sub-expression — the
/// elimination of a neutral element cannot introduce new side effects.
fn normalize_identity<'a>(
    node: &'a Syntax<'a>,
    _arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    let (lhs, op, rhs) = binop_parts(node)?;
    match op {
        "+" => {
            if is_literal_zero(rhs) { return Some(lhs); }
            if is_literal_zero(lhs) { return Some(rhs); }
        }
        "-" => {
            // x - 0 → x.  Note: 0 - x = -x, NOT an identity.
            if is_literal_zero(rhs) { return Some(lhs); }
        }
        "*" => {
            if is_literal_one(rhs) { return Some(lhs); }
            if is_literal_one(lhs) { return Some(rhs); }
        }
        "/" => {
            // x / 1 → x.  Note: 1 / x is NOT an identity.
            if is_literal_one(rhs) { return Some(lhs); }
        }
        "**" => {
            // x ** 1 → x  (exponentiation; Python, Rust).
            if is_literal_one(rhs) { return Some(lhs); }
        }
        "^" => {
            // Bitwise XOR identity: x ^ 0 = x.
            // (In Python `**` is exponentiation; `^` remains XOR for integers.)
            if is_literal_zero(rhs) { return Some(lhs); }
            if is_literal_zero(lhs) { return Some(rhs); }
        }
        "|" => {
            // Bitwise OR identity: x | 0 = x.
            if is_literal_zero(rhs) { return Some(lhs); }
            if is_literal_zero(lhs) { return Some(rhs); }
        }
        _ => {}
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Rule 3 — Inverse / self-cancellation
// ─────────────────────────────────────────────────────────────────────────────

/// Detect expressions that unconditionally cancel to a known constant.
///
/// ```text
/// x - x  →  __zero__       (subtraction self-cancels to zero)
/// x ^ x  →  __zero__       (XOR: every bit cancels; always 0)
/// ```
///
/// Division (`x / x = 1`) is intentionally excluded: it is only valid when
/// `x ≠ 0`, a condition the normalizer cannot verify statically.
///
/// The sentinel `__zero__` is distinct from the literal `"0"` so that
/// `x - x` on one side and an explicit `0` on the other still appears as
/// a diff when the user cares about the form.
///
/// Only pure atoms are considered (no risk of silently suppressing a
/// side-effecting function call that appears on both sides).
fn normalize_inverse<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    let (lhs, op, rhs) = binop_parts(node)?;
    if !is_pure_atom(lhs) || !is_pure_atom(rhs) {
        return None;
    }
    if !atoms_equal(lhs, rhs) {
        return None;
    }
    match op {
        "-" | "^" => Some(synth_atom(arena, "__zero__")),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rule 4 — Commutativity
// ─────────────────────────────────────────────────────────────────────────────

/// Canonicalise commutative binary operations by sorting their operands into
/// a stable lexicographic order, so that `b OP a` and `a OP b` produce
/// structurally identical nodes.
///
/// **Equality / inequality** (`==`, `!=`): always commutative for any two
/// atom operands.  `b == a` and `a == b` both normalise to `a == b`.
///
/// **Arithmetic / bitwise** (`+`, `*`, `&`, `|`): commutative only when
/// *both* operands are integer literals, preventing false equivalences for
/// string concatenation (`"foo" + "bar"`) or side-effecting calls.
///
/// The canonical ordering is strict lexicographic on the atom content string.
/// This is consistent and collision-free; numeric ordering is irrelevant
/// because constant-folding fires first (Rule 1) and collapses all-literal
/// arithmetic to a single token.
///
/// **Note on `>` / `>=`**: C/C++ commutativity of ordered comparisons is
/// handled separately in `c_compare_canon` (see `c.rs`), which converts
/// `>` → `__lt__` and `>=` → `__le__` with flipped operands.  Other
/// languages leave ordered comparisons as-is; they are not sorted here to
/// avoid the `__gt__`/`__ge__` negation-sentinel mismatch.
fn normalize_commutative<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    let (lhs, op, rhs) = binop_parts(node)?;
    let ch = list_children(node)?;
    let op_atom = ch[1];

    match op {
        // Equality/inequality: sort any atom operands.
        "==" | "!=" => {
            if is_pure_atom(lhs) && is_pure_atom(rhs) {
                let l_key = atom_content(lhs).unwrap_or("");
                let r_key = atom_content(rhs).unwrap_or("");
                if l_key > r_key {
                    return Some(rebuild_binop(node, arena, rhs, op_atom, lhs));
                }
            }
        }
        // Arithmetic / bitwise: sort only integer literals.
        "+" | "*" | "&" | "|" => {
            if is_int_literal(lhs) && is_int_literal(rhs) {
                let l_key = atom_content(lhs).unwrap_or("");
                let r_key = atom_content(rhs).unwrap_or("");
                if l_key > r_key {
                    return Some(rebuild_binop(node, arena, rhs, op_atom, lhs));
                }
            }
        }
        _ => {}
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Rule 5 — Idempotency
// ─────────────────────────────────────────────────────────────────────────────

/// Detect idempotent logical or bitwise operations: `a OP a → a`.
///
/// ```text
/// a && a  →  a       a || a  →  a
/// a and a →  a       a or a  →  a      (Python keywords)
/// a &  a  →  a       a |  a  →  a      (bitwise integers)
/// ```
///
/// Only pure atoms are considered (no duplicate evaluation of side effects).
/// Note that subtraction idempotency (`a - a`) is already covered by Rule 3
/// (self-cancellation → `__zero__`), which is the correct semantic.
fn normalize_idempotent<'a>(
    node: &'a Syntax<'a>,
    _arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    let (lhs, op, rhs) = binop_parts(node)?;
    if !is_pure_atom(lhs) || !is_pure_atom(rhs) {
        return None;
    }
    if !atoms_equal(lhs, rhs) {
        return None;
    }
    match op {
        "&&" | "||" | "and" | "or" | "&" | "|" => Some(lhs),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rule 6 — Absorption  [Advanced]
// ─────────────────────────────────────────────────────────────────────────────

/// Detect boolean absorption: `a OP (a OTHER b)  →  a`.
///
/// ```text
/// a || (a && b)    →  a        a || (b && a)    →  a
/// (a && b) || a   →  a
///
/// a && (a || b)    →  a        a && (b || a)    →  a
/// (a || b) && a   →  a
/// ```
///
/// Python (`and` / `or`) and bitwise (`&` / `|`) operators are also matched.
///
/// Gated at **Advanced** because absorption silently *removes* `b` from the
/// tree.  If the diff was intentionally adding or removing a sub-condition,
/// absorption would hide that signal.
///
/// The outer repeated operand must be a pure atom to guarantee that no
/// side-effecting call is suppressed when `b` is dropped.
fn normalize_absorption<'a>(
    node: &'a Syntax<'a>,
    _arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    let (outer_lhs, outer_op, outer_rhs) = binop_parts(node)?;

    let is_and = |op: &str| matches!(op, "&&" | "and" | "&");
    let is_or  = |op: &str| matches!(op, "||" | "or"  | "|");

    if is_or(outer_op) {
        // a || (a && b)  or  (a && b) || a  →  a
        if is_pure_atom(outer_lhs) {
            if let Some((il, iop, ir)) = binop_parts(outer_rhs) {
                if is_and(iop)
                    && (atoms_equal(outer_lhs, il) || atoms_equal(outer_lhs, ir))
                {
                    return Some(outer_lhs);
                }
            }
        }
        if is_pure_atom(outer_rhs) {
            if let Some((il, iop, ir)) = binop_parts(outer_lhs) {
                if is_and(iop)
                    && (atoms_equal(outer_rhs, il) || atoms_equal(outer_rhs, ir))
                {
                    return Some(outer_rhs);
                }
            }
        }
    } else if is_and(outer_op) {
        // a && (a || b)  or  (a || b) && a  →  a
        if is_pure_atom(outer_lhs) {
            if let Some((il, iop, ir)) = binop_parts(outer_rhs) {
                if is_or(iop)
                    && (atoms_equal(outer_lhs, il) || atoms_equal(outer_lhs, ir))
                {
                    return Some(outer_lhs);
                }
            }
        }
        if is_pure_atom(outer_rhs) {
            if let Some((il, iop, ir)) = binop_parts(outer_lhs) {
                if is_or(iop)
                    && (atoms_equal(outer_rhs, il) || atoms_equal(outer_rhs, ir))
                {
                    return Some(outer_rhs);
                }
            }
        }
    }

    None
}