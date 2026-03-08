//! Semantic normalizer for JavaScript and TypeScript.
//!
//! # Logical foundations
//!
//! Normalizers are formal rewrite rules (`A == B`) from Wayne's
//! "Logic for Programmers".  Only sound bidirectional equivalences are applied.
//!
//! **Basic level rules:**
//!   - De Morgan NAND: `!(a && b)` == `!a || !b`  (Appendix B)
//!   - De Morgan NOR:  `!(a || b)` == `!a && !b`  (Appendix B)
//!   - Boolean coercion: `!!x` == `Boolean(x)`  (both force bool; `!!x` ≠ `x` for typing)
//!   - Null/undefined equivalences: `x == null` == `x == undefined`  (loose equality)
//!   - typeof-undefined: all forms canonicalized
//!   - console.log/info/warn/debug level differences are cosmetic
//!   - var/let/const declaration keyword  
//!   - Object/Array spread equivalences
//!   - **if-else canonicalization**: `if (c) { T } else { F }` → `__bool_if__(c, T, F)`
//!   - **Branch inversion**: `if (!c) { F } else { T }` → `__bool_if__(c, T, F)`
//!     Strips the negation and swaps branches so both forms are identical.
//!   - **Short-circuit equivalence**: `if (a) { if (b) { body } }` → `__if_and__(a, b, body)`
//!     A doubly-nested `if` with no `else` clauses has the same semantics as
//!     `if (a && b) { body }`.
//!
//! **Advanced level rules:**
//!   - Optional chaining: `x?.foo` ↔ guarded access  (structural, may hide intent)
//!   - TypeScript `interface` vs `type` (structural)
//!
//! **Note on De Morgan and JS truthiness (§3.3):**
//!   `!(a && b)` == `!a || !b` holds under boolean semantics.  Because `!`
//!   always coerces to bool in JS, the De Morgan equivalence is exact even for
//!   truthy operands.  This makes it safer than Python's `not` (which doesn't
//!   coerce its arguments, only its result) — but both are valid Basic rules.
//!
//! Tree-sitter node kinds used (tree-sitter-javascript / tree-sitter-typescript):
//!   lexical_declaration, variable_declaration, call_expression, unary_expression,
//!   binary_expression, member_expression, spread_element, object, array,
//!   interface_declaration, type_alias_declaration, as_expression,
//!   optional_chain, non_null_expression,
//!   if_statement, statement_block, else_clause, parenthesized_expression

use typed_arena::Arena;
use crate::options::SemanticLevel;
use crate::parse::guess_language as guess;
use crate::parse::syntax::Syntax;
use super::{
    atom_content, find_child_by_kind, is_atom_one_of,
    list_children, node_kind, non_punct_children, parent_kind, unwrap_paren,
    synth_atom, synth_list, SemanticNormalizer,
};

pub struct JavaScriptNormalizer;
pub struct TypeScriptNormalizer;

impl SemanticNormalizer for JavaScriptNormalizer {
    fn language(&self) -> guess::Language {
        guess::Language::JavaScript
    }

    fn normalize<'a>(
        &self,
        node: &'a Syntax<'a>,
        parent: Option<&'a Syntax<'a>>,
        arena: &'a Arena<Syntax<'a>>,
        level: SemanticLevel,
    ) -> Option<&'a Syntax<'a>> {
        shared_normalize(node, parent, arena, level)
    }
}

impl SemanticNormalizer for TypeScriptNormalizer {
    fn language(&self) -> guess::Language {
        guess::Language::TypeScript
    }

    fn normalize<'a>(
        &self,
        node: &'a Syntax<'a>,
        parent: Option<&'a Syntax<'a>>,
        arena: &'a Arena<Syntax<'a>>,
        level: SemanticLevel,
    ) -> Option<&'a Syntax<'a>> {
        shared_normalize(node, parent, arena, level)
            .or_else(|| ts_normalize_double_cast(node, arena))
            .or_else(|| {
                if level == SemanticLevel::Advanced {
                    // interface↔type is structural — may hide extensibility changes.
                    ts_normalize_interface_vs_type(node, arena)
                        // non-null assertion `x!` → `x`: technically no runtime
                        // effect, but removing it changes TypeScript type-checking
                        // behaviour at the call site, so Advanced-only is safer.
                        .or_else(|| ts_normalize_non_null(node, arena))
                } else {
                    None
                }
            })
    }
}

fn shared_normalize<'a>(
    node: &'a Syntax<'a>,
    parent: Option<&'a Syntax<'a>>,
    arena: &'a Arena<Syntax<'a>>,
    level: SemanticLevel,
) -> Option<&'a Syntax<'a>> {
    // Basic rules — sound logical tautologies.
    normalize_de_morgan_nand(node, arena)
        .or_else(|| normalize_de_morgan_nor(node, arena))
        .or_else(|| normalize_boolean_coercion(node, arena))
        .or_else(|| normalize_typeof_undefined(node, arena))
        .or_else(|| normalize_object_spread(node, arena))
        .or_else(|| normalize_array_spread(node, arena))
        .or_else(|| normalize_console_log(node, arena))
        .or_else(|| normalize_nullish_eq(node, arena))
        // if-else canonicalization + branch inversion + nested-if-and.
        .or_else(|| normalize_bool_if(node, arena))
        .or_else(|| normalize_nested_if_and(node, arena))
        // Advanced rules — structural equivalences that may hide real changes.
        .or_else(|| {
            if level == SemanticLevel::Advanced {
                // var/let/const unification is Advanced: the three keywords
                // have different scoping rules and `const` is immutable, so
                // treating them as equal is not a logical tautology (§3.3).
                normalize_var_keyword(node, parent, arena)
                    .or_else(|| normalize_optional_chain(node, arena))
            } else {
                None
            }
        })
}

// ─── De Morgan's Law: NAND ────────────────────────────────────────────────────
/// **De Morgan's Law** (Appendix B): `!(A && B) == !A || !B`
///
/// JavaScript forms:
///   `!(a && b)`  →  `__nand__(a, b)`
///   `!a || !b`   →  `__nand__(a, b)`
///
/// Because JS `!` always produces a boolean, the De Morgan equivalence is exact
/// even for truthy operands — this is the "forced coercion" that makes JS De
/// Morgan strictly safer than Python's `not`.
fn normalize_de_morgan_nand<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    // Pattern 1: !(a && b)
    //   unary_expression { "!", [paren_expr] { binary_expression { a, "&&", b } } }
    if node_kind(node) == "unary_expression" {
        let children = list_children(node)?;
        if children.len() == 2 && is_not_atom(children[0]) {
            let inner = unwrap_paren(children[1]);
            if node_kind(inner) == "binary_expression" {
                let bc = non_punct_children(inner)?;
                if bc.len() == 3 && atom_content(bc[1]) == Some("&&") {
                    return Some(nand_node(arena, bc[0], bc[2]));
                }
            }
        }
    }

    // Pattern 2: !a || !b
    //   binary_expression { unary("!",a), "||", unary("!",b) }
    if node_kind(node) == "binary_expression" {
        let children = non_punct_children(node)?;
        if children.len() == 3 && atom_content(children[1]) == Some("||") {
            if let (Some(a), Some(b)) = (
                extract_js_not_operand(children[0]),
                extract_js_not_operand(children[2]),
            ) {
                return Some(nand_node(arena, a, b));
            }
        }
    }

    None
}

// ─── De Morgan's Law: NOR ─────────────────────────────────────────────────────
/// **De Morgan's Law** (Appendix B): `!(A || B) == !A && !B`
///
/// JavaScript forms:
///   `!(a || b)`  →  `__nor__(a, b)`
///   `!a && !b`   →  `__nor__(a, b)`
fn normalize_de_morgan_nor<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    // Pattern 1: !(a || b)
    if node_kind(node) == "unary_expression" {
        let children = list_children(node)?;
        if children.len() == 2 && is_not_atom(children[0]) {
            let inner = unwrap_paren(children[1]);
            if node_kind(inner) == "binary_expression" {
                let bc = non_punct_children(inner)?;
                if bc.len() == 3 && atom_content(bc[1]) == Some("||") {
                    return Some(nor_node(arena, bc[0], bc[2]));
                }
            }
        }
    }

    // Pattern 2: !a && !b
    if node_kind(node) == "binary_expression" {
        let children = non_punct_children(node)?;
        if children.len() == 3 && atom_content(children[1]) == Some("&&") {
            if let (Some(a), Some(b)) = (
                extract_js_not_operand(children[0]),
                extract_js_not_operand(children[2]),
            ) {
                return Some(nor_node(arena, a, b));
            }
        }
    }

    None
}

// ─── var / let / const keyword normalisation ─────────────────────────────────
/// `var` / `let` / `const` → `__var__`
///
/// Gated on **Advanced** because these keywords are *not* semantically
/// equivalent:
///   - `var` has function scope; `let`/`const` have block scope.
///   - `const` is immutable.
/// Treating all three as identical at Basic would violate the §3.3 "Programs
/// are not Math" principle — it's not a logical tautology.
///
/// The parent check is strict: only fires when the parent is an actual
/// declaration node (`lexical_declaration` or `variable_declaration`).
/// The previous `|| pk == ""` fallback — which fired when there was *no*
/// parent — is removed because a top-level `var`/`let`/`const` atom that
/// isn't inside a declaration is either a parse artefact or an identifier
/// that happens to share the keyword name; normalising it would be wrong.
fn normalize_var_keyword<'a>(
    node: &'a Syntax<'a>,
    parent: Option<&'a Syntax<'a>>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    let pk = parent_kind(parent);
    // Strict parent check: no empty-string fallback.
    let is_decl_parent = pk == "lexical_declaration" || pk == "variable_declaration";
    if !is_decl_parent {
        return None;
    }

    let nk = node_kind(node);
    if nk == "var" || nk == "let" || nk == "const" {
        return Some(synth_atom(arena, "__var__"));
    }

    if is_atom_one_of(node, &["var", "let", "const"]) {
        return Some(synth_atom(arena, "__var__"));
    }
    None
}

// ─── Boolean coercion ────────────────────────────────────────────────────────
/// `!!x` → `__bool__(x)`   and   `Boolean(x)` → `__bool__(x)`
///
/// Note: `!!x` is normalized to `__bool__(x)`, NOT to `x`.  In JavaScript,
/// `!!x` forces a boolean type while `x` may be any truthy type.  Normalizing
/// to `__bool__` means `!!x ↔ Boolean(x)` is treated as equivalent (correct)
/// while `!!x ↔ x` is still shown as a diff (correct — they differ in type).
fn normalize_boolean_coercion<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "unary_expression" {
        let children = list_children(node)?;
        if children.len() == 2 && is_not_atom(children[0]) {
            if node_kind(children[1]) == "unary_expression" {
                let inner_ch = list_children(children[1])?;
                if inner_ch.len() == 2 && is_not_atom(inner_ch[0]) {
                    let x = inner_ch[1];
                    return Some(synth_list(arena, "__bool__(", vec![x], ")"));
                }
            }
        }
    }

    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name == "Boolean" {
                return Some(synth_list(
                    arena,
                    "__bool__(",
                    children[1..].to_vec(),
                    ")",
                ));
            }
        }
    }
    None
}

// ─── typeof undefined ────────────────────────────────────────────────────────
/// Canonicalise all `typeof` / `undefined` checks regardless of operand order
/// and strict vs. loose equality:
///
///   `typeof x === 'undefined'`   `'undefined' === typeof x`
///   `typeof x == 'undefined'`    `x === undefined`
///       → `__is_undefined__(x)`
fn normalize_typeof_undefined<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "binary_expression" {
        let children = non_punct_children(node)?;
        if children.len() == 3 {
            let op = atom_content(children[1])?;
            if op == "===" || op == "==" {
                if let Some(target) = extract_typeof_target(children[0]) {
                    let rhs_str = atom_content(children[2])?;
                    if is_undefined_token(rhs_str) {
                        return Some(synth_list(
                            arena,
                            "__is_undefined__(",
                            vec![target],
                            ")",
                        ));
                    }
                }
                if let Some(target) = extract_typeof_target(children[2]) {
                    let lhs_str = atom_content(children[0])?;
                    if is_undefined_token(lhs_str) {
                        return Some(synth_list(
                            arena,
                            "__is_undefined__(",
                            vec![target],
                            ")",
                        ));
                    }
                }
                // x === undefined (no typeof)
                let rhs_str = atom_content(children[2]);
                let lhs_str = atom_content(children[0]);
                if rhs_str == Some("undefined") {
                    return Some(synth_list(
                        arena,
                        "__is_undefined__(",
                        vec![children[0]],
                        ")",
                    ));
                }
                if lhs_str == Some("undefined") {
                    return Some(synth_list(
                        arena,
                        "__is_undefined__(",
                        vec![children[2]],
                        ")",
                    ));
                }
            }
        }
    }

    None
}

/// If `node` is a `typeof expr` unary expression, return `expr`.
fn extract_typeof_target<'a>(node: &'a Syntax<'a>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "unary_expression" {
        let ch = list_children(node)?;
        if ch.len() == 2 && atom_content(ch[0])? == "typeof" {
            return Some(ch[1]);
        }
    }
    None
}

fn is_undefined_token(s: &str) -> bool {
    s == "undefined" || s == "'undefined'" || s == "\"undefined\""
}

// ─── Object spread ───────────────────────────────────────────────────────────
/// Canonicalise object spread/merge forms to `__object_spread__(sources…)`:
///
///   `Object.assign({}, a)`       → `__object_spread__(a)`
///   `Object.assign({}, a, b)`    → `__object_spread__(a, b)`
///   `{ ...a }`                   → `__object_spread__(a)`
///   `{ ...a, ...b }`             → `__object_spread__(a, b)`
///
/// The previous object-literal branch only handled the single-spread case
/// (`non_punct.len() == 1`).  `{ ...a, ...b }` was silently skipped, so it
/// never matched `Object.assign({}, a, b)`.  The fix: accept any number of
/// spread elements, provided *all* non-punctuation children are spreads.
fn normalize_object_spread<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    // ── Object.assign({}, source…) ───────────────────────────────────────────
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name == "Object.assign" {
                if let Some(arg_list) = children.get(1) {
                    let args = non_punct_children(arg_list)?;
                    if !args.is_empty() && node_kind(args[0]) == "object" {
                        if list_children(args[0]).map_or(false, |c| c.is_empty()) {
                            let sources: Vec<_> = args[1..].iter().copied().collect();
                            return Some(synth_list(
                                arena,
                                "__object_spread__(",
                                sources,
                                ")",
                            ));
                        }
                    }
                }
            }
        }
    }

    // ── { ...a }  or  { ...a, ...b, … } ─────────────────────────────────────
    if node_kind(node) == "object" {
        let non_punct = non_punct_children(node)?;
        // Every non-punctuation child must be a spread_element.
        if !non_punct.is_empty()
            && non_punct.iter().all(|c| node_kind(*c) == "spread_element")
        {
            let mut targets: Vec<&'a Syntax<'a>> = Vec::with_capacity(non_punct.len());
            for c in &non_punct {
                // spread_element children: ["...", target]
                let spread_ch = list_children(*c)?;
                let target = spread_ch.last().copied()?;
                targets.push(target);
            }
            return Some(synth_list(arena, "__object_spread__(", targets, ")"));
        }
    }
    None
}

// ─── Array spread ────────────────────────────────────────────────────────────
/// `Array.from(x)` → `__array_spread__(x)`
/// `[...x]` → `__array_spread__(x)`
fn normalize_array_spread<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name == "Array.from" {
                if let Some(arg_list) = children.get(1) {
                    return Some(synth_list(
                        arena,
                        "__array_spread__(",
                        vec![arg_list],
                        ")",
                    ));
                }
            }
        }
    }

    if node_kind(node) == "array" {
        let non_punct = non_punct_children(node)?;
        if non_punct.len() == 1 && node_kind(non_punct[0]) == "spread_element" {
            let spread_ch = list_children(non_punct[0])?;
            let target = spread_ch.last().copied()?;
            return Some(synth_list(arena, "__array_spread__(", vec![target], ")"));
        }
    }
    None
}

// ─── console.log / info / warn / debug ───────────────────────────────────────
/// `console.log` / `console.info` / `console.warn` / `console.debug`
/// → `__console__(…)`   (log level is cosmetic)
fn normalize_console_log<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(callee) = children.first().copied() {
            if node_kind(callee) == "member_expression" {
                let callee_ch = list_children(callee)?;
                let obj = atom_content(callee_ch.first().copied()?)?;
                let method = atom_content(callee_ch.last().copied()?)?;
                if obj == "console"
                    && matches!(method, "log" | "info" | "warn" | "debug")
                {
                    let args = children.get(1).copied();
                    return Some(synth_list(
                        arena,
                        "__console__(",
                        args.map_or(vec![], |a| vec![a]),
                        ")",
                    ));
                }
            }

            if let Some(callee_str) = atom_content(callee) {
                if matches!(
                    callee_str,
                    "console.log" | "console.info" | "console.warn" | "console.debug"
                ) {
                    let args = children.get(1).copied();
                    return Some(synth_list(
                        arena,
                        "__console__(",
                        args.map_or(vec![], |a| vec![a]),
                        ")",
                    ));
                }
            }
        }
    }
    None
}

// ─── Nullish equality ────────────────────────────────────────────────────────
/// Normalise nullish checks regardless of operand order:
///
///   `x == null`  `null == x`  `x == undefined`  `undefined == x`
///       → `__nullish__(x)`
///
///   `x != null`  `null != x`  `x != undefined`  `undefined != x`
///       → `__not_nullish__(x)`
fn normalize_nullish_eq<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "binary_expression" {
        let children = non_punct_children(node)?;
        if children.len() == 3 {
            let op = atom_content(children[1])?;
            if op == "==" || op == "!=" {
                let lhs_str = atom_content(children[0]);
                let rhs_str = atom_content(children[2]);
                let is_null_token = |s: &str| s == "null" || s == "undefined";

                let (subject, negated) = if rhs_str.map_or(false, is_null_token) {
                    (children[0], op == "!=")
                } else if lhs_str.map_or(false, is_null_token) {
                    (children[2], op == "!=")
                } else {
                    return None;
                };

                let canonical = if negated {
                    "__not_nullish__("
                } else {
                    "__nullish__("
                };
                return Some(synth_list(arena, canonical, vec![subject], ")"));
            }
        }
    }

    None
}

// ─── Optional chaining (Advanced) ────────────────────────────────────────────
/// `x?.foo` → `__opt_chain__(x, foo)` — gated on Advanced because optional
/// chaining carries semantic intent (null-safety at call site vs. guarantee
/// from type system), and collapsing it can mask a real architectural change.
// ... (imports and struct definitions remain the same)

// ... (inside normalize_optional_chain function)

// ─── Optional chaining (Advanced) ────────────────────────────────────────────
/// `x?.foo` → `__opt_chain__(x, foo)` — gated on Advanced because optional
/// chaining carries semantic intent (null-safety at call site vs. guarantee
/// from type system), and collapsing it can mask a real architectural change.
fn normalize_optional_chain<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    // In tree-sitter-javascript, `x?.foo` is a `member_expression` whose
    // children include a `"?."` atom: [object, "?.", property].
    if node_kind(node) == "member_expression" {
        let children = list_children(node)?;
        if !children.iter().any(|c| atom_content(c) == Some("?.")) {
            return None;
        }
        // Object is the first child; property is the last non-"?." child.
        let obj = children.first().copied()?;
        let prop = children
            .iter()
            .rev()
            .find(|c| atom_content(*c) != Some("?."))
            .copied()?;
        
        // FIX: obj and prop are references, pass them directly to ptr::eq
        if std::ptr::eq(obj, prop) {
            // Only one meaningful child — can't split into obj/prop.
            return None;
        }
        return Some(synth_list(
            arena,
            "__opt_chain__(",
            vec![obj, synth_atom(arena, ", "), prop],
            ")",
        ));
    }

    // `optional_chain` wrapper node — produced by some tree-sitter grammar
    // versions for chained optional accesses.
    if node_kind(node) == "optional_chain" {
        let children = list_children(node)?;
        if !children.is_empty() {
            return Some(synth_list(arena, "__opt_chain__(", children.to_vec(), ")"))
        }
    }

    None
}

// ... (rest of the file remains the same)
// ─── TypeScript-only ─────────────────────────────────────────────────────────
/// `x as unknown as T` → `x as T`   (redundant double cast)
fn ts_normalize_double_cast<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "as_expression" {
        let children = list_children(node)?;
        if children.len() >= 3 && node_kind(children[0]) == "as_expression" {
            let inner_ch = list_children(children[0])?;
            if inner_ch.len() >= 3 {
                let mid_type = atom_content(inner_ch.last().copied()?)?;
                if mid_type == "unknown" || mid_type == "any" {
                    let expr = inner_ch.first().copied()?;
                    let target_type = children.last().copied()?;
                    return Some(Syntax::new_list(
                        arena,
                        "",
                        vec![],
                        vec![expr, synth_atom(arena, " as "), target_type],
                        " ",
                        vec![],
                        "as_expression",
                    ));
                }
            }
        }
    }

    None
}

/// `interface Foo { … }` ↔ `type Foo = { … }` → `__object_type__ Foo { … }`
/// Gated on Advanced: changing from `interface` to `type` can affect extensibility.
fn ts_normalize_interface_vs_type<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "interface_declaration" || node_kind(node) == "type_alias_declaration" {
        let name_node = find_child_by_kind(node, "type_identifier")?;
        let name = atom_content(name_node)?;
        let body_node = find_child_by_kind(node, "object_type")
            .or_else(|| find_child_by_kind(node, "type_literal"))?;
        let body_children = list_children(body_node).unwrap_or_default().to_vec();
        return Some(Syntax::new_list(
            arena,
            &format!("__object_type__ {name} {{ "),
            vec![],
            body_children,
            " }",
            vec![],
            "",
        ));
    }
    None
}

/// TypeScript non-null assertion `x!` → `x`
///
/// Gated on **Advanced**: while `x!` has no runtime effect, non-null assertions
/// exist precisely because the author knows something the type system cannot
/// prove.  Removing them changes whether TypeScript emits type errors in the
/// surrounding code, which is a meaningful semantic difference at the
/// type-system level even if it is invisible at runtime.  Basic level should
/// not silently suppress that signal.
fn ts_normalize_non_null<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "non_null_expression" {
        let children = list_children(node)?;
        let expr = children.first().copied()?;
        return Some(expr);
    }

    None
}

// ─── if-else canonicalization + branch inversion ─────────────────────────────
/// Canonicalise `if`-`else` statements to `__bool_if__(cond, T, F)` and
/// simultaneously handle **branch inversion**.
///
/// **Standard form:**
/// ```js
/// if (cond) { T } else { F }   →   __bool_if__(cond, T, F)
/// ```
///
/// **Branch-inverted form:**
/// ```js
/// if (!cond) { F } else { T }  →   __bool_if__(cond, T, F)
/// ```
///
/// When the condition inside the `parenthesized_expression` is a `!` negation,
/// the negation is stripped and the two branches are swapped, so both forms
/// produce the same canonical node.
///
/// Tree-sitter structure for `if (c) T else F`:
/// ```
/// if_statement
///   parenthesized_expression { c }
///   statement_block { T }
///   else_clause { statement_block { F } }
/// ```
///
/// **Guards** — does NOT fire when:
/// - There is no `else` clause (would collapse control flow).
/// - The `else_clause` contains another `if_statement` (else-if chain).
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

    let else_clause = *else_clauses[0];
    let else_ch = list_children(else_clause)?;

    // Reject `else if` chains.
    if else_ch.iter().any(|c| node_kind(c) == "if_statement") {
        return None;
    }

    // else_clause must contain a statement_block.
    let else_block = else_ch
        .iter()
        .find(|c| node_kind(c) == "statement_block")?;

    // condition is the parenthesized_expression child of the if_statement.
    let paren_node = children
        .iter()
        .find(|c| node_kind(c) == "parenthesized_expression")?;

    // consequence is the statement_block directly under the if_statement.
    let then_block = children
        .iter()
        .find(|c| node_kind(c) == "statement_block")?;

    // Unwrap the parenthesised condition to get the actual expression.
    let cond_expr = js_unwrap_parens(*paren_node);

    // Branch inversion: if cond_expr is `!inner`, use `inner` and swap blocks.
    let (canonical_cond, true_block, false_block): (&Syntax, &Syntax, &Syntax) =
        if let Some(inner) = extract_js_not_operand(cond_expr) {
            (inner, *else_block, *then_block)
        } else {
            (cond_expr, *then_block, *else_block)
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
/// ```js
/// if (a) { if (b) { body } }   ≡   if (a && b) { body }
/// ```
///
/// Both forms evaluate `b` only when `a` is truthy and execute `body` only
/// when both hold.  Produces the sentinel:
///
/// ```
/// __if_and__(a, b, body)
/// ```
///
/// **Guards** — does NOT fire when:
/// - Either `if` has an `else` clause.
/// - The outer block contains more than one statement (extra statements would
///   be unreachable in the `&&` form, so the structures are NOT equivalent).
/// - Either `if` is part of an `else if` chain.
fn normalize_nested_if_and<'a>(
    node: &'a Syntax<'a>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "if_statement" {
        return None;
    }
    let children = list_children(node)?;

    // Outer if must have NO else_clause.
    if children.iter().any(|c| node_kind(c) == "else_clause") {
        return None;
    }

    let outer_paren = children
        .iter()
        .find(|c| node_kind(c) == "parenthesized_expression")?;

    let outer_block_node = children
        .iter()
        .find(|c| node_kind(c) == "statement_block")?;

    // Outer block must contain exactly one statement — the inner if.
    let block_stmts = list_children(*outer_block_node)?;
    if block_stmts.len() != 1 {
        return None;
    }

    let inner = block_stmts[0];
    if node_kind(inner) != "if_statement" {
        return None;
    }

    let inner_children = list_children(inner)?;

    // Inner if must also have NO else_clause.
    if inner_children.iter().any(|c| node_kind(c) == "else_clause") {
        return None;
    }

    let inner_paren = inner_children
        .iter()
        .find(|c| node_kind(c) == "parenthesized_expression")?;

    let inner_block = inner_children
        .iter()
        .find(|c| node_kind(c) == "statement_block")?;

    let outer_cond = js_unwrap_parens(*outer_paren);
    let inner_cond = js_unwrap_parens(*inner_paren);

    Some(synth_list(
        arena,
        "__if_and__(",
        vec![
            outer_cond,
            synth_atom(arena, ", "),
            inner_cond,
            synth_atom(arena, ", "),
            *inner_block,
        ],
        ")",
    ))
}

/// Strip one layer of `parenthesized_expression` wrapping, returning the
/// single inner expression node.  Falls back to the node itself when the
/// parenthesised expression has a non-trivial shape (multiple children).
///
/// In tree-sitter-javascript a `parenthesized_expression` wraps the condition
/// of every `if`/`while`/etc. statement.  The actual condition is the sole
/// non-punctuation child.
fn js_unwrap_parens<'a>(node: &'a Syntax<'a>) -> &'a Syntax<'a> {
    if node_kind(node) != "parenthesized_expression" {
        return node;
    }
    let children = match list_children(node) {
        Some(ch) => ch,
        None => return node,
    };
    // Filter out bare "(" / ")" punctuation atoms if present.
    let exprs: Vec<_> = children
        .iter()
        .filter(|c| {
            !matches!(atom_content(*c), Some("(") | Some(")"))
        })
        .collect();
    if exprs.len() == 1 {
        *exprs[0]
    } else {
        node
    }
}

// ─── Private helpers ─────────────────────────────────────────────────────────

/// True if `node` is an Atom with content `"!"`.
#[inline]
fn is_not_atom<'a>(node: &'a Syntax<'a>) -> bool {
    atom_content(node).map_or(false, |c| c == "!")
}

/// If `node` is `unary_expression { "!", x }`, return `x`.
fn extract_js_not_operand<'a>(node: &'a Syntax<'a>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "unary_expression" {
        return None;
    }
    let children = list_children(node)?;
    if children.len() != 2 || !is_not_atom(children[0]) {
        return None;
    }
    Some(children[1])
}

/// Build `__nand__(a, b)` canonical node.
fn nand_node<'a>(
    arena: &'a Arena<Syntax<'a>>,
    a: &'a Syntax<'a>,
    b: &'a Syntax<'a>,
) -> &'a Syntax<'a> {
    synth_list(arena, "__nand__(", vec![a, synth_atom(arena, ", "), b], ")")
}

/// Build `__nor__(a, b)` canonical node.
fn nor_node<'a>(
    arena: &'a Arena<Syntax<'a>>,
    a: &'a Syntax<'a>,
    b: &'a Syntax<'a>,
) -> &'a Syntax<'a> {
    synth_list(arena, "__nor__(", vec![a, synth_atom(arena, ", "), b], ")")
}