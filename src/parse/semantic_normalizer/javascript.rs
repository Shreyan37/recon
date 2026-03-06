//! Semantic normalizer for JavaScript and TypeScript.
//!
//! Tree-sitter node kinds used (tree-sitter-javascript / tree-sitter-typescript):
//!   lexical_declaration, variable_declaration, call_expression, unary_expression,
//!   binary_expression, member_expression, spread_element, object, array,
//!   interface_declaration, type_alias_declaration, as_expression

use typed_arena::Arena;
use crate::parse::guess_language as guess;
use crate::parse::syntax::Syntax;
use super::{
    atom_content, find_child_by_kind, is_atom_one_of,
    list_children, node_kind, non_punct_children, parent_kind,
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
    ) -> Option<&'a Syntax<'a>> {
        shared_normalize(node, parent, arena)
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
    ) -> Option<&'a Syntax<'a>> {
        shared_normalize(node, parent, arena)
            .or_else(|| ts_normalize_double_cast(node, arena))
            .or_else(|| ts_normalize_interface_vs_type(node, arena))
    }
}

fn shared_normalize<'a>(
    node: &'a Syntax<'a>,
    parent: Option<&'a Syntax<'a>>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    normalize_var_keyword(node, parent, arena)
        .or_else(|| normalize_boolean_coercion(node, arena))
        .or_else(|| normalize_typeof_undefined(node, arena))
        .or_else(|| normalize_object_spread(node, arena))
        .or_else(|| normalize_array_spread(node, arena))
        .or_else(|| normalize_console_log(node, arena))
        .or_else(|| normalize_nullish_eq(node, arena))
}

// ─── var / let / const keyword normalisation ─────────────────────────────────
///  `var` / `let` / `const` (the keyword token, not the whole declaration)
/// → `__var__`
///
/// The keyword is an Atom with ts_node_kind == "var" / "let" / "const" in
/// tree-sitter-javascript.  We only fire when the parent is a variable
/// declaration node so we don't accidentally collapse these when they appear
/// as property names.
fn normalize_var_keyword<'a>(
    node: &'a Syntax<'a>,
    parent: Option<&'a Syntax<'a>>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    let pk = parent_kind(parent);
    let is_decl_parent = pk == "lexical_declaration"
        || pk == "variable_declaration"
        || pk == "";  // synthetic / top-level — allow through
    if !is_decl_parent {
        return None;
    }

    let nk = node_kind(node);
    if nk == "var" || nk == "let" || nk == "const" {
        return Some(synth_atom(arena, "__var__"));
    }

    // Also handle as a bare keyword Atom without ts_node_kind set
    if is_atom_one_of(node, &["var", "let", "const"]) {
        return Some(synth_atom(arena, "__var__"));
    }
    None
}

// ─── Boolean coercion ────────────────────────────────────────────────────────
///  `!!x` → `__bool__(x)`   and   `Boolean(x)` → `__bool__(x)`
fn normalize_boolean_coercion<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    // `!!x` — unary_expression with operator "!" applied twice
    if node_kind(node) == "unary_expression" {
        let children = list_children(node)?;
        // children: ["!", inner_unary]
        if children.len() == 2 {
            if atom_content(children[0])? == "!" {
                if node_kind(children[1]) == "unary_expression" {
                    // inner: ["!", x]
                    let inner_ch = list_children(children[1])?;
                    if inner_ch.len() == 2 && atom_content(inner_ch[0])? == "!" {
                        let x = inner_ch[1];
                        return Some(synth_list(arena, "__bool__(", vec![x], ")"));
                    }
                }
            }
        }
    }

    // Flat atom "!!x"
    if let Some(c) = atom_content(node) {
        if let Some(inner) = c.strip_prefix("!!") {
            return Some(synth_list(arena, "__bool__(", vec![synth_atom(arena, inner)], ")"));
        }
    }

    // `Boolean(x)` call
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name == "Boolean" {
                return Some(synth_list(arena, "__bool__(", children[1..].to_vec(), ")"));
            }
        }
    }
    None
}

// ─── typeof undefined ────────────────────────────────────────────────────────
///  `typeof x === 'undefined'` → `__is_undefined__(x)`
fn normalize_typeof_undefined<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    // binary_expression: [typeof_expr, "===", undefined_literal]
    if node_kind(node) == "binary_expression" {
        let children = non_punct_children(node)?;
        if children.len() == 3 {
            // operator in the middle
            if let Some(op) = atom_content(children[1]) {
                if op == "===" || op == "==" {
                    let lhs = children[0];
                    let rhs = children[2];
                    if node_kind(lhs) == "unary_expression" {
                        if let Some(lhs_ch) = list_children(lhs) {
                            if lhs_ch.len() == 2 && atom_content(lhs_ch[0])? == "typeof" {
                                let target = lhs_ch[1];
                                let rhs_str = atom_content(rhs)?;
                                if rhs_str == "undefined" || rhs_str == "'undefined'" || rhs_str == "\"undefined\"" {
                                    return Some(synth_list(arena, "__is_undefined__(", vec![target], ")"));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Flat atom form
    if let Some(c) = atom_content(node) {
        if let Some(rest) = c.strip_prefix("typeof ") {
            for suffix in &[" === 'undefined'", " === \"undefined\"", " == undefined"] {
                if let Some(var) = rest.strip_suffix(suffix) {
                    return Some(synth_list(arena, "__is_undefined__(", vec![synth_atom(arena, var)], ")"));
                }
            }
        }
    }
    None
}

// ─── Object spread ───────────────────────────────────────────────────────────
///  `Object.assign({}, x)` → `__object_spread__(x)`
///  `{ ...x }` → `__object_spread__(x)`
fn normalize_object_spread<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    // Object.assign({}, x)
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name == "Object.assign" {
                // args: [{}, x, ...]
                if let Some(arg_list) = children.get(1) {
                    let args = non_punct_children(arg_list)?;
                    if !args.is_empty() && node_kind(args[0]) == "object" {
                        if list_children(args[0]).map_or(false, |c| c.is_empty()) {
                            let sources: Vec<_> = args[1..].iter().map(|x| **x).collect();
                            return Some(synth_list(arena, "__object_spread__(", sources, ")"));
                        }
                    }
                }
            }
        }
    }

    // { ...x } — object with a single spread_element
    if node_kind(node) == "object" {
        let non_punct = non_punct_children(node)?;
        if non_punct.len() == 1 && node_kind(non_punct[0]) == "spread_element" {
            let spread_ch = list_children(non_punct[0])?;
            let target = spread_ch.last().copied()?;
            return Some(synth_list(arena, "__object_spread__(", vec![target], ")"));
        }
    }
    None
}

// ─── Array spread ────────────────────────────────────────────────────────────
///  `Array.from(x)` → `__array_spread__(x)`
///  `[...x]` → `__array_spread__(x)`
fn normalize_array_spread<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(fn_name) = atom_content(children.first().copied()?) {
            if fn_name == "Array.from" {
                if let Some(arg_list) = children.get(1) {
                    return Some(synth_list(arena, "__array_spread__(", vec![arg_list], ")"));
                }
            }
        }
    }

    // [...]
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
///  `console.log` / `console.info` / `console.warn` / `console.debug`
/// → `__console__`   (all write to stdout; log level is cosmetic)
fn normalize_console_log<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        if let Some(callee) = children.first().copied() {
            // callee is a member_expression like `console.log`
            if node_kind(callee) == "member_expression" {
                let callee_ch = list_children(callee)?;
                let obj = atom_content(callee_ch.first().copied()?)?;
                let method = atom_content(callee_ch.last().copied()?)?;
                if obj == "console" && matches!(method, "log" | "info" | "warn" | "debug") {
                    let args = children.get(1).copied();
                    return Some(synth_list(
                        arena, "__console__(",
                        args.map_or(vec![], |a| vec![a]),
                        ")",
                    ));
                }
            }

            // Flat atom "console.log" etc.
            if let Some(callee_str) = atom_content(callee) {
                if matches!(callee_str, "console.log" | "console.info" | "console.warn" | "console.debug") {
                    let args = children.get(1).copied();
                    return Some(synth_list(
                        arena, "__console__(",
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
///  `x == null` → `__nullish__(x)`
///  `x === null || x === undefined` → `__nullish__(x)`
fn normalize_nullish_eq<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    // binary_expression: [x, "==", null]
    if node_kind(node) == "binary_expression" {
        let children = non_punct_children(node)?;
        if children.len() == 3 {
            if let Some(op) = atom_content(children[1]) {
                if op == "==" {
                    let rhs_str = atom_content(children[2])?;
                    if rhs_str == "null" || rhs_str == "undefined" {
                        return Some(synth_list(arena, "__nullish__(", vec![children[0]], ")"));
                    }
                }
            }
        }
    }

    // Flat atom: "x == null"
    if let Some(c) = atom_content(node) {
        if let Some(lhs) = c.strip_suffix(" == null") {
            if !lhs.contains(' ') {
                return Some(synth_list(arena, "__nullish__(", vec![synth_atom(arena, lhs)], ")"));
            }
        }
    }
    None
}

// ─── TypeScript-only ─────────────────────────────────────────────────────────
///  `x as unknown as T` → `x as T`   (redundant double cast)
fn ts_normalize_double_cast<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "as_expression" {
        let children = list_children(node)?;
        // children: [inner_as_expr, "as", T]
        if children.len() >= 3 && node_kind(children[0]) == "as_expression" {
            let inner_ch = list_children(children[0])?;
            if inner_ch.len() >= 3 {
                let mid_type = atom_content(inner_ch.last().copied()?)?;
                if mid_type == "unknown" || mid_type == "any" {
                    let expr = inner_ch.first().copied()?;
                    let target_type = children.last().copied()?;
                    return Some(Syntax::new_list(
                        arena, "", vec![], vec![expr, synth_atom(arena, " as "), target_type],
                        " ", vec![], "as_expression",
                    ));
                }
            }
        }
    }

    // Flat atom "x as unknown as T"
    if let Some(c) = atom_content(node) {
        if let Some(pos) = c.find(" as unknown as ") {
            let expr = &c[..pos];
            let target = &c[pos + " as unknown as ".len()..];
            return Some(synth_atom(arena, &format!("{expr} as {target}")));
        }
    }
    None
}

///  `interface Foo { … }` ↔ `type Foo = { … }` → `__object_type__ Foo { … }`
fn ts_normalize_interface_vs_type<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "interface_declaration" || node_kind(node) == "type_alias_declaration" {
        // Find the name (type_identifier child) and body (object_type child)
        let name_node = find_child_by_kind(node, "type_identifier")?;
        let name = atom_content(name_node)?;
        let body_node = find_child_by_kind(node, "object_type")
            .or_else(|| find_child_by_kind(node, "type_literal"))?;
        let body_children = list_children(body_node).cloned().unwrap_or_default();
        return Some(Syntax::new_list(
            arena,
            &format!("__object_type__ {name} {{ "),
            vec![], body_children, " }", vec![], "",
        ));
    }
    None
}