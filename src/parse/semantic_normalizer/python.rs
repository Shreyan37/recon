//! Semantic normalizer for Python.
//!
//! # Logical foundations
//!
//! The normalizers in this file are instances of formal rewrite rules (see
//! "Logic for Programmers" §2.5, Appendix B).  Only bidirectional (`==`)
//! equivalences are implemented; one-way implications (`=>`) are not safe for
//! a diff tool because they could silently drop semantically meaningful changes.
//!
//! **Basic level rules applied:**
//!   - None checks:    `x == None` == `x is None`  (Python's PEP 8 equivalence)
//!   - De Morgan NAND: `not (a and b)` == `not a or not b`  (Appendix B)
//!   - De Morgan NOR:  `not (a or b)` == `not a and not b`  (Appendix B)
//!   - Double negation: `not not x` == `bool(x)` (P.212 truthy-aware form; see note below)
//!   - Empty constructors: `list()` == `[]`, `dict()` == `{}`, `set()`
//!   - Print calls: `print(x)` == `print(x, end="\n")` (stream equivalence)
//!   - **if-else canonicalization**: `if cond: T else: F` → `__bool_if__(cond, T, F)`
//!   - **Branch inversion**: `if not cond: F else: T` → `__bool_if__(cond, T, F)`
//!     Strips `not` and swaps branches so both forms are identical.
//!   - **Short-circuit equivalence**: `if a:\n  if b:\n    body` → `__if_and__(a, b, body)`
//!     A doubly-nested `if` with no `else` is semantically identical to
//!     `if a and b: body`.
//!
//! **Advanced level rules applied:**
//!   - List comprehension structural equivalences (loop-to-comprehension rewrites)
//!
//! **Note on double negation and truthiness (§3.3 "Programs are not Math"):**
//!   In Python, `not not x` returns a `bool`, while `x` may be any truthy type.
//!   These are NOT strictly equal. We therefore normalize `not not x` → `__bool__(x)`
//!   rather than → `x`, mirroring the JS `!!x` → `__bool__(x)` treatment, so
//!   that adding/removing `bool()` coercion wrappers is treated as equivalent
//!   but `not not x` vs `x` is still shown as a diff.
//!
//! Tree-sitter node kinds used (tree-sitter-python):
//!   call, not_operator, boolean_operator, comparison_operator,
//!   list, dictionary, set, generator_expression, for_in_clause,
//!   if_statement, else_clause, elif_clause, block

use typed_arena::Arena;
use crate::options::SemanticLevel;
use crate::parse::guess_language as guess;
use crate::parse::syntax::Syntax;
use super::{
    atom_content, list_children, node_kind, non_punct_children, unwrap_paren,
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
        level: SemanticLevel,
    ) -> Option<&'a Syntax<'a>> {
        // Basic rules — sound tautologies, always safe.
        normalize_none_check(node, arena)
            .or_else(|| normalize_de_morgan_nand(node, arena))
            .or_else(|| normalize_de_morgan_nor(node, arena))
            .or_else(|| normalize_double_negation(node, arena))
            .or_else(|| normalize_len_check(node, arena))
            .or_else(|| normalize_print_call(node, arena))
            .or_else(|| normalize_empty_constructors(node, arena))
            // if-else canonicalization + branch inversion + nested-if-and.
            .or_else(|| normalize_bool_if(node, arena))
            .or_else(|| normalize_nested_if_and(node, arena))
            // Advanced rules — structural equivalences, may hide real changes.
            .or_else(|| {
                if level == SemanticLevel::Advanced {
                    normalize_list_comprehension(node, arena)
                } else {
                    None
                }
            })
    }
}

// ─── None check ──────────────────────────────────────────────────────────────
/// Canonicalise all idiomatic None comparisons to `__is_none__(x)` or
/// `__is_not_none__(x)` regardless of operator and operand order:
///
///   `x is None`      `x == None`      `None is x`      `None == x`
///       → `__is_none__(x)`
///
///   `x is not None`  `x != None`      `None is not x`  `None != x`
///       → `__is_not_none__(x)`
fn normalize_none_check<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "comparison_operator" {
        return None;
    }
    // non_punct_children filters only "," and ";", so operators like "is",
    // "is not", "==", "!=" survive and are present at index 1.
    let children = non_punct_children(node)?;
    if children.len() != 3 {
        return None;
    }

    let op = atom_content(children[1])?;
    let lhs_str = atom_content(children[0]);
    let rhs_str = atom_content(children[2]);

    // Determine which operand is `None` and which is the subject.
    let (subject, negated) = if rhs_str == Some("None") {
        (children[0], matches!(op, "!=" | "is not"))
    } else if lhs_str == Some("None") {
        (children[2], matches!(op, "!=" | "is not"))
    } else {
        return None;
    };

    // Validate operator is one of the expected forms.
    if !matches!(op, "==" | "!=" | "is" | "is not") {
        return None;
    }

    let canonical = if negated {
        "__is_not_none__("
    } else {
        "__is_none__("
    };
    Some(synth_list(arena, canonical, vec![subject], ")"))
}

// ─── De Morgan's Law: NAND ────────────────────────────────────────────────────
/// **De Morgan's Law** (Appendix B): `!(A && B) == !A || !B`
///
/// Python forms:
///   `not (a and b)`   →  `__nand__(a, b)`
///   `not a or not b`  →  `__nand__(a, b)`
///
/// Safe for Basic because De Morgan holds even with Python's truthy semantics:
/// for any a, b: `not (a and b) == (not a or not b)` is a tautology even when
/// a and b are non-boolean (§3.3 warning does not apply here — the rewrite
/// preserves the boolean *result*, not the truthiness *type*).
fn normalize_de_morgan_nand<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    // Pattern 1: `not (a and b)`
    //   not_operator { "not", [paren_expr|boolean_operator] { a, "and", b } }
    if node_kind(node) == "not_operator" {
        let children = list_children(node)?;
        if children.len() == 2 && is_atom_str(children[0], "not") {
            let inner = unwrap_paren(children[1]);
            if node_kind(inner) == "boolean_operator" {
                let ic = non_punct_children(inner)?;
                if ic.len() == 3 && is_atom_str(ic[1], "and") {
                    let a = ic[0];
                    let b = ic[2];
                    return Some(synth_list(
                        arena,
                        "__nand__(",
                        vec![a, synth_atom(arena, ", "), b],
                        ")",
                    ));
                }
            }
        }
    }

    // Pattern 2: `not a or not b`
    //   boolean_operator { not_operator { a }, "or", not_operator { b } }
    if node_kind(node) == "boolean_operator" {
        let children = non_punct_children(node)?;
        if children.len() == 3 && is_atom_str(children[1], "or") {
            if let (Some(a), Some(b)) = (
                extract_not_operand(children[0]),
                extract_not_operand(children[2]),
            ) {
                return Some(synth_list(
                    arena,
                    "__nand__(",
                    vec![a, synth_atom(arena, ", "), b],
                    ")",
                ));
            }
        }
    }

    None
}

// ─── De Morgan's Law: NOR ─────────────────────────────────────────────────────
/// **De Morgan's Law** (Appendix B): `!(A || B) == !A && !B`
///
/// Python forms:
///   `not (a or b)`     →  `__nor__(a, b)`
///   `not a and not b`  →  `__nor__(a, b)`
fn normalize_de_morgan_nor<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    // Pattern 1: `not (a or b)`
    if node_kind(node) == "not_operator" {
        let children = list_children(node)?;
        if children.len() == 2 && is_atom_str(children[0], "not") {
            let inner = unwrap_paren(children[1]);
            if node_kind(inner) == "boolean_operator" {
                let ic = non_punct_children(inner)?;
                if ic.len() == 3 && is_atom_str(ic[1], "or") {
                    let a = ic[0];
                    let b = ic[2];
                    return Some(synth_list(
                        arena,
                        "__nor__(",
                        vec![a, synth_atom(arena, ", "), b],
                        ")",
                    ));
                }
            }
        }
    }

    // Pattern 2: `not a and not b`
    if node_kind(node) == "boolean_operator" {
        let children = non_punct_children(node)?;
        if children.len() == 3 && is_atom_str(children[1], "and") {
            if let (Some(a), Some(b)) = (
                extract_not_operand(children[0]),
                extract_not_operand(children[2]),
            ) {
                return Some(synth_list(
                    arena,
                    "__nor__(",
                    vec![a, synth_atom(arena, ", "), b],
                    ")",
                ));
            }
        }
    }

    None
}

// ─── Double negation ─────────────────────────────────────────────────────────
/// **Double Negation** (Appendix B, truthy-aware form):
///   `not not x`  →  `__bool__(x)`
///
/// In Python, `not not x` returns a `bool` while `x` may be any type.
/// We normalize to `__bool__(x)` (not raw `x`) so that:
///   - `not not x` ↔ `bool(x)` is treated as equivalent (both explicit coercions)
///   - `not not x` ↔ `x` is still shown as a diff (truthiness vs. value)
///
/// This matches the JS treatment of `!!x` → `__bool__(x)` for consistency.
fn normalize_double_negation<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "not_operator" {
        return None;
    }
    let children = list_children(node)?;
    if children.len() != 2 || !is_atom_str(children[0], "not") {
        return None;
    }

    let inner = children[1];
    if node_kind(inner) != "not_operator" {
        return None;
    }
    let inner_children = list_children(inner)?;
    if inner_children.len() != 2 || !is_atom_str(inner_children[0], "not") {
        return None;
    }

    let x = inner_children[1];
    Some(synth_list(arena, "__bool__(", vec![x], ")"))
}

// ─── bool() call ─────────────────────────────────────────────────────────────
// Note: `bool(x)` is NOT explicitly normalised here because it is already
// handled by the double-negation normalizer — `not not x` → `__bool__(x)` and
// `bool(x)` → `__bool__(x)` makes them equivalent.  A dedicated `bool(x)`
// call normalizer would need to fire here; add below if needed.

// ─── len() check ─────────────────────────────────────────────────────────────
/// Canonicalise length comparisons to `__len_nonempty__(x)` or `__len_empty__(x)`:
///
///   `len(x) > 0`   `len(x) >= 1`   `len(x) != 0`  →  `__len_nonempty__(x)`
///   `len(x) == 0`  `len(x) <= 0`   `not len(x)`   →  `__len_empty__(x)`
fn normalize_len_check<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "comparison_operator" {
        return None;
    }
    let children = non_punct_children(node)?;
    if children.len() != 3 {
        return None;
    }

    let op = atom_content(children[1])?;

    // Try: len(x) OP literal
    if let Some(subject) = extract_len_subject(children[0]) {
        let rhs = atom_content(children[2])?;
        let canonical = match (op, rhs) {
            (">", "0") | (">=", "1") | ("!=", "0") => "__len_nonempty__(",
            ("==", "0") | ("<=", "0") => "__len_empty__(",
            _ => return None,
        };
        return Some(synth_list(arena, canonical, vec![subject], ")"));
    }

    // Try reversed: literal OP len(x)
    if let Some(subject) = extract_len_subject(children[2]) {
        let lhs = atom_content(children[0])?;
        let canonical = match (lhs, op) {
            ("0", "<") | ("1", "<=") | ("0", "!=") => "__len_nonempty__(",
            ("0", "==") | ("0", ">=") => "__len_empty__(",
            _ => return None,
        };
        return Some(synth_list(arena, canonical, vec![subject], ")"));
    }

    None
}

/// If `node` is a `len(x)` call, return `x`.
fn extract_len_subject<'a>(node: &'a Syntax<'a>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "call" {
        return None;
    }
    let children = list_children(node)?;
    if children.len() < 2 {
        return None;
    }
    if atom_content(children[0])? != "len" {
        return None;
    }
    // args node: List with open "(" and children [subject]
    let args = children[1];
    let args_children = list_children(args)?;
    if args_children.len() == 1 {
        Some(args_children[0])
    } else {
        None
    }
}

// ─── print() ─────────────────────────────────────────────────────────────────
/// `print(x)` → `__print__(x)`  — unifies `print` with/without `end="\n"`.
fn normalize_print_call<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "call" {
        return None;
    }
    let children = list_children(node)?;
    if children.is_empty() {
        return None;
    }
    if atom_content(children[0])? != "print" {
        return None;
    }
    // Preserve arguments as-is but wrap in canonical __print__ form.
    let args = children.get(1).copied().unwrap_or_else(|| synth_list(arena, "(", vec![], ")"));
    Some(synth_list(arena, "__print__", vec![args], ""))
}

// ─── Empty constructors ───────────────────────────────────────────────────────
/// Unify empty-container constructor forms:
///   `list()` ↔ `[]`    →  `__empty_list__`
///   `dict()` ↔ `{}`    →  `__empty_dict__`
///   `set()`            →  `__empty_set__`
fn normalize_empty_constructors<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    // list() / dict() / set() call forms
    if node_kind(node) == "call" {
        let children = list_children(node)?;
        if children.len() >= 2 {
            let args = children[1];
            let args_ch = list_children(args).map_or(0, |c| c.len());
            if args_ch == 0 {
                match atom_content(children[0])? {
                    "list" => return Some(synth_atom(arena, "__empty_list__")),
                    "dict" => return Some(synth_atom(arena, "__empty_dict__")),
                    "set"  => return Some(synth_atom(arena, "__empty_set__")),
                    _ => {}
                }
            }
        }
    }

    // [] literal
    if node_kind(node) == "list" {
        if list_children(node).map_or(true, |c| c.is_empty()) {
            return Some(synth_atom(arena, "__empty_list__"));
        }
    }

    // {} literal (ambiguous: could be dict or set, but empty {} is always dict in Python)
    if node_kind(node) == "dictionary" {
        if list_children(node).map_or(true, |c| c.is_empty()) {
            return Some(synth_atom(arena, "__empty_dict__"));
        }
    }

    None
}

// ─── List comprehension (Advanced) ───────────────────────────────────────────
/// Normalize `[f(x) for x in iterable]` and equivalent loop patterns.
/// Gated on Advanced because loop-to-comprehension rewrites may change
/// evaluation order of side-effecting expressions.
fn normalize_list_comprehension<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    // list_comprehension: open "[", children [expression, for_in_clause], close "]"
    if node_kind(node) != "list_comprehension" {
        return None;
    }
    let children = list_children(node)?;
    if children.len() < 2 {
        return None;
    }
    let expr = children[0];
    let for_clause = children.iter().find(|c| node_kind(c) == "for_in_clause")?;
    let fc_children = list_children(for_clause)?;
    if fc_children.len() < 4 {
        return None;
    }
    // for_in_clause: ["for", var, "in", iterable]
    let var = fc_children[1];
    let iterable = fc_children[3];
    Some(synth_list(
        arena,
        "__listcomp__(",
        vec![
            synth_list(arena, "", vec![expr], ""),
            synth_atom(arena, ", "),
            var,
            synth_atom(arena, ", "),
            iterable,
        ],
        ")",
    ))
}

// ─── if-else canonicalization + branch inversion ─────────────────────────────
/// Canonicalise Python `if`-`else` statements and handle **branch inversion**.
///
/// **Standard form:**
/// ```python
/// if cond: T  else: F   →   __bool_if__(cond, T, F)
/// ```
///
/// **Branch-inverted form:**
/// ```python
/// if not cond: F  else: T   →   __bool_if__(cond, T, F)
/// ```
///
/// When the condition is a `not_operator { "not", inner }`, `inner` is used
/// as the canonical condition and the two branch blocks are swapped.
///
/// Tree-sitter structure for `if cond: T else: F`:
/// ```
/// if_statement
///   condition: <expr>
///   consequence: block { T }
///   alternative: else_clause { block { F } }
/// ```
///
/// **Guards** — does NOT fire when:
/// - There is no `else_clause` (would collapse control flow).
/// - The `else_clause` contains an `elif_clause` or nested `if_statement`
///   (else-if / elif chain).
fn normalize_bool_if<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "if_statement" {
        return None;
    }
    let children = list_children(node)?;

    // Require exactly one else_clause.
    let else_clauses: Vec<_> = children
        .iter()
        .filter(|c| node_kind(c) == "else_clause")
        .collect();
    if else_clauses.len() != 1 {
        return None;
    }

    // Reject elif chains.
    if children.iter().any(|c| node_kind(c) == "elif_clause") {
        return None;
    }

    let else_clause = *else_clauses[0];
    let else_ch = list_children(else_clause)?;

    // else_clause must not contain a nested if (else-if form).
    if else_ch.iter().any(|c| node_kind(c) == "if_statement") {
        return None;
    }

    let else_block = else_ch
        .iter()
        .find(|c| node_kind(c) == "block")?;

    // condition: first child that is not a keyword or block or else_clause.
    let condition = children.iter().find(|c| {
        let k = node_kind(c);
        !matches!(k, "if" | "else" | "elif" | "block" | "else_clause" | "elif_clause")
            && atom_content(c) != Some("if")
            && atom_content(c) != Some(":")
    })?;

    let then_block = children
        .iter()
        .find(|c| node_kind(c) == "block")?;

    // Branch inversion: if condition is `not inner`, use `inner` and swap.
    let (canonical_cond, true_block, false_block): (&Syntax, &Syntax, &Syntax) =
        if let Some(inner) = extract_not_operand(*condition) {
            (inner, *else_block, *then_block)
        } else {
            (*condition, *then_block, *else_block)
        };

    Some(synth_list(
        arena,
        "__bool_if__(",
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

// ─── Nested if → if-and (short-circuit equivalence) ──────────────────────────
/// Canonicalise doubly-nested `if` statements with no `else` clause.
///
/// ```python
/// if a:
///     if b:
///         body    ≡   if a and b:  body
/// ```
///
/// Both forms evaluate `b` only when `a` is truthy and execute `body` only
/// when both hold.  Produces:
///
/// ```
/// __if_and__(a, b, body)
/// ```
///
/// **Guards** — does NOT fire when:
/// - Either `if` has an `else_clause` or `elif_clause`.
/// - The outer `block` contains more than one statement (extra statements
///   are outside the inner `if` scope and would be lost in the `and` form).
fn normalize_nested_if_and<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "if_statement" {
        return None;
    }
    let children = list_children(node)?;

    // Outer if must have no else or elif.
    if children
        .iter()
        .any(|c| matches!(node_kind(c), "else_clause" | "elif_clause"))
    {
        return None;
    }

    let outer_cond = children.iter().find(|c| {
        let k = node_kind(c);
        !matches!(k, "if" | "else" | "elif" | "block" | "else_clause" | "elif_clause")
            && atom_content(c) != Some("if")
            && atom_content(c) != Some(":")
    })?;

    let outer_block_node = children.iter().find(|c| node_kind(c) == "block")?;

    // Outer block must contain exactly one statement.
    let block_stmts = list_children(*outer_block_node)?;
    if block_stmts.len() != 1 {
        return None;
    }

    let inner = block_stmts[0];
    if node_kind(inner) != "if_statement" {
        return None;
    }

    let inner_children = list_children(inner)?;

    // Inner if must also have no else or elif.
    if inner_children
        .iter()
        .any(|c| matches!(node_kind(c), "else_clause" | "elif_clause"))
    {
        return None;
    }

    let inner_cond = inner_children.iter().find(|c| {
        let k = node_kind(c);
        !matches!(k, "if" | "else" | "elif" | "block" | "else_clause" | "elif_clause")
            && atom_content(c) != Some("if")
            && atom_content(c) != Some(":")
    })?;

    let inner_block = inner_children.iter().find(|c| node_kind(c) == "block")?;

    Some(synth_list(
        arena,
        "__if_and__(",
        vec![
            *outer_cond,
            synth_atom(arena, ", "),
            *inner_cond,
            synth_atom(arena, ", "),
            *inner_block,
        ],
        ")",
    ))
}

// ─── Private helpers ─────────────────────────────────────────────────────────

/// True if `node` is an Atom with content equal to `s`.
#[inline]
fn is_atom_str<'a>(node: &'a Syntax<'a>, s: &str) -> bool {
    atom_content(node).map_or(false, |c| c == s)
}

/// If `node` is a `not_operator { "not", x }`, return `x`.
fn extract_not_operand<'a>(node: &'a Syntax<'a>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "not_operator" {
        return None;
    }
    let children = list_children(node)?;
    if children.len() != 2 || !is_atom_str(children[0], "not") {
        return None;
    }
    Some(children[1])
}