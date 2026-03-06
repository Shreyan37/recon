//! Semantic normalizers for C and C++.
//!
//! Tree-sitter node kinds used:
//!   C:   call_expression, argument_list, unary_expression, sizeof_expression,
//!        null, pointer_declarator
//!   C++: call_expression, cast_expression, static_cast, new_expression,
//!        null_literal, using_declaration

use typed_arena::Arena;
use crate::parse::guess_language as guess;
use crate::parse::syntax::Syntax;
use super::{
    atom_content, is_atom_one_of,
    list_children, list_open, node_kind, non_punct_children,
    synth_atom, synth_list, SemanticNormalizer,
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
    ) -> Option<&'a Syntax<'a>> {
        c_shared(node, parent, arena)
            .or_else(|| cpp_null(node, arena))
            .or_else(|| cpp_cast(node, arena))
            .or_else(|| cpp_cout(node, arena))
            .or_else(|| cpp_cerr(node, arena))
            .or_else(|| cpp_new(node, arena))
            .or_else(|| cpp_string_ctor(node, arena))
            .or_else(|| cpp_move(node, arena))
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
}

// ─── NULL (C context) ────────────────────────────────────────────────────────
///  `NULL` / `((void*)0)` → `__null__`
///
///  `0` is only normalised when the parent is a pointer-typed context
/// (pointer_declarator, assignment with pointer type, argument_list, etc.)
/// to avoid false positives in arithmetic.
fn c_null<'a>(
    node: &'a Syntax<'a>,
    parent: Option<&'a Syntax<'a>>,
    arena: &'a Arena<Syntax<'a>>,
) -> Option<&'a Syntax<'a>> {
    // Match the `NULL` macro as a plain identifier atom.
    if let Some(c) = atom_content(node) {
        if c == "NULL" {
            return Some(synth_atom(arena, "__null__"));
        }
    }

    // Match tree-sitter's `null` node kind (some ts-c grammars use this).
    if node_kind(node) == "null" {
        return Some(synth_atom(arena, "__null__"));
    }

    // Match cast expressions: (void*)0 — the anonymous ( and ) in a
    // cast_expression are NOT hoisted to open/close by difftastic (unlike
    // parenthesized_expression), so they appear as atom children.
    // Use non_punct_children to skip them before indexing.
    if node_kind(node) == "cast_expression" {
        let children = non_punct_children(node)?;
        if children.len() >= 2 {
            let type_node = children[0];
            let val_node = children.last().copied()?;
            let is_void_ptr = atom_content(type_node)
                .map(|s| s == "void*" || s == "void *")
                .unwrap_or(false)
                || is_void_pointer_type(type_node);
            if is_void_ptr && atom_content(val_node) == Some("0") {
                return Some(synth_atom(arena, "__null__"));
            }
        }
    }

    // Match parenthesized null: ((void*)0)
    // Use non_punct_children so this works whether or not difftastic hoists
    // the parens into open/close_content.
    //
    // IMPORTANT: normalize_node is bottom-up, so by the time we see
    // parenthesized_expression its child may ALREADY have been rewritten to
    // the synthetic __null__ atom.  We must handle both cases:
    //   (a) child is still the raw cast_expression  → recurse into c_null
    //   (b) child is already __null__               → pass it straight through
    if node_kind(node) == "parenthesized_expression" {
        let children = non_punct_children(node)?;
        if children.len() == 1 {
            let child = children[0];
            // Case (b): already normalized by the bottom-up pass.
            if atom_content(child) == Some("__null__") {
                return Some(synth_atom(arena, "__null__"));
            }
            // Case (a): raw subtree – try to normalize it now.
            if let Some(normalized) = c_null(child, parent, arena) {
                return Some(normalized);
            }
        }
    }

    None
}

/// Returns true if `node` represents a `void *` type.
///
/// tree-sitter-c encodes `(void*)` as a `type_descriptor` list whose
/// children are `["void", pointer_declarator | abstract_pointer_declarator | "*"]`.
fn is_void_pointer_type(node: &Syntax<'_>) -> bool {
    let children = match list_children(node) {
        Some(c) => c,
        None => return false,
    };
    let mut has_void = false;
    let mut has_ptr = false;
    for child in children {
        if let Some(s) = atom_content(child) {
            if s == "void" { has_void = true; }
            if s == "*"   { has_ptr  = true; }
        }
        let k = node_kind(child);
        if k == "abstract_pointer_declarator" || k == "pointer_declarator" {
            has_ptr = true;
        }
    }
    has_void && has_ptr
}
// ─── sizeof ──────────────────────────────────────────────────────────────────
///  `sizeof(T)` / `sizeof T` → `__sizeof__(T)`
fn c_sizeof<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "sizeof_expression" {
        let children = non_punct_children(node)?;
        // children: ["sizeof", operand] (keyword may be filtered out already)
        let operand = children.last().copied()?;
        return Some(synth_list(arena, "__sizeof__(", vec![operand], ")"));
    }

    // Flat atom "sizeof(T)" or "sizeof T"
    if let Some(c) = atom_content(node) {
        if let Some(inner) = c.strip_prefix("sizeof(").and_then(|s| s.strip_suffix(')')) {
            return Some(synth_list(arena, "__sizeof__(", vec![synth_atom(arena, inner)], ")"));
        }
        if let Some(inner) = c.strip_prefix("sizeof ") {
            return Some(synth_list(arena, "__sizeof__(", vec![synth_atom(arena, inner)], ")"));
        }
    }
    None
}

// ─── printf / fprintf ────────────────────────────────────────────────────────
///  `printf(…)` / `fprintf(stdout, …)` → `__print__(…)`
///  `fprintf(stderr, …)` → `__log_err__(…)`
fn c_printf<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "call_expression" {
        return None;
    }
    let children = list_children(node)?;
    let fn_name = atom_content(children.first().copied()?)?;

    if fn_name == "printf" {
        let args = children.get(1).copied();
        return Some(synth_list(arena, "__print__(", args.map_or(vec![], |a| vec![a]), ")"));
    }

    if fn_name == "fprintf" {
        if let Some(arg_list) = children.get(1) {
            let args = non_punct_children(arg_list)?;
            if !args.is_empty() {
                let dest = atom_content(args[0])?;
                let rest: Vec<_> = args[1..].iter().map(|x| **x).collect();
                let canonical = if dest == "stderr" { "__log_err__(" } else { "__print__(" };
                return Some(synth_list(arena, canonical, rest, ")"));
            }
        }
    }
    None
}

// ─── assert ──────────────────────────────────────────────────────────────────
///  `assert(cond && "msg")` → `__assert__(cond)`   (strip message string)
fn c_assert<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "call_expression" {
        return None;
    }
    let children = list_children(node)?;
    let fn_name = atom_content(children.first().copied()?)?;
    if fn_name != "assert" {
        return None;
    }

    if let Some(arg_list) = children.get(1) {
        let args = non_punct_children(arg_list)?;
        if args.len() == 1 {
            // Check if the argument is a `&&` expression with a string on the right
            let arg = args[0];
            if node_kind(arg) == "binary_expression" {
                let bin_ch = non_punct_children(arg)?;
                if bin_ch.len() == 3 && atom_content(bin_ch[1])? == "&&" {
                    let rhs_str = atom_content(bin_ch[2])?;
                    if rhs_str.starts_with('"') {
                        // Strip the && "msg" — keep only the condition
                        return Some(synth_list(arena, "__assert__(", vec![bin_ch[0]], ")"));
                    }
                }
            }
            // No message — still canonicalise
            return Some(synth_list(arena, "__assert__(", vec![arg], ")"));
        }
    }
    None
}

// ─── memset zero-init ────────────────────────────────────────────────────────
///  `memset(p, 0, sizeof(*p))` → `__zero_init__(p)`
fn c_memset_zero<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) != "call_expression" {
        return None;
    }
    let children = list_children(node)?;
    if atom_content(children.first().copied()?)? != "memset" {
        return None;
    }

    if let Some(arg_list) = children.get(1) {
        let args = non_punct_children(arg_list)?;
        if args.len() == 3 {
            let is_zero = atom_content(args[1]).map_or(false, |s| s == "0");
            let is_sizeof = node_kind(args[2]) == "sizeof_expression"
                || atom_content(args[2]).map_or(false, |s: &str| s.starts_with("sizeof") || s.starts_with("__sizeof__"));
            if is_zero && is_sizeof {
                return Some(synth_list(arena, "__zero_init__(", vec![args[0]], ")"));
            }
        }
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// C++ only
// ─────────────────────────────────────────────────────────────────────────────

///  `nullptr` / `NULL` → `__null__`
fn cpp_null<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "null_literal" || is_atom_one_of(node, &["nullptr", "NULL"]) {
        return Some(synth_atom(arena, "__null__"));
    }
    None
}

///  `static_cast<T>(x)` / `reinterpret_cast<T>(x)` / `(T)x` → `__cast__(T, x)`
fn cpp_cast<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    // C++ named cast: cast_expression with type + value
    if node_kind(node) == "cast_expression" {
        let children = non_punct_children(node)?;
        if children.len() >= 2 {
            let type_node = children[0];
            let val_node = children.last().copied()?;
            return Some(synth_list(
                arena, "__cast__(",
                vec![type_node, synth_atom(arena, ", "), val_node], ")",
            ));
        }
    }

    // template cast: List open "static_cast<" or "reinterpret_cast<" etc.
    if let Some(open) = list_open(node) {
        for kw in &["static_cast<", "reinterpret_cast<", "const_cast<", "dynamic_cast<"] {
            if open.starts_with(kw) {
                let type_str = &open[kw.len()..].trim_end_matches('>');
                let children = list_children(node)?;
                let mut canonical = vec![synth_atom(arena, type_str)];
                canonical.extend_from_slice(children);
                return Some(synth_list(arena, "__cast__(", canonical, ")"));
            }
        }
    }
    None
}

///  `std::cout << x << std::endl` / `cout << x << "\n"` → `__print__(x)`
fn cpp_cout<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    // binary_expression tree: [cout, "<<", payload] possibly nested
    if node_kind(node) == "binary_expression" {
        if let Some((stream, payload)) = extract_stream_payload(node, "<<") {
            let stream_str = atom_content(stream)?;
            if stream_str == "std::cout" || stream_str == "cout" {
                // Strip endl / "\n" from rightmost payload
                let payload_clean = strip_endl(payload);
                return Some(synth_list(arena, "__print__(", vec![payload_clean], ")"));
            }
        }
    }

    // Flat atom
    if let Some(c) = atom_content(node) {
        if let Some(rest) = c.strip_prefix("std::cout << ").or_else(|| c.strip_prefix("cout << ")) {
            let payload = rest
                .trim_end_matches(" << std::endl")
                .trim_end_matches(" << endl")
                .trim_end_matches(" << \"\\n\"");
            return Some(synth_list(arena, "__print__(", vec![synth_atom(arena, payload)], ")"));
        }
    }
    None
}

///  `std::cerr << x` → `__log_err__(x)`
fn cpp_cerr<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "binary_expression" {
        if let Some((stream, payload)) = extract_stream_payload(node, "<<") {
            let stream_str = atom_content(stream)?;
            if stream_str == "std::cerr" || stream_str == "cerr" {
                let payload_clean = strip_endl(payload);
                return Some(synth_list(arena, "__log_err__(", vec![payload_clean], ")"));
            }
        }
    }

    if let Some(c) = atom_content(node) {
        if let Some(rest) = c.strip_prefix("std::cerr << ").or_else(|| c.strip_prefix("cerr << ")) {
            let payload = rest.trim_end_matches(" << std::endl").trim_end_matches(" << endl");
            return Some(synth_list(arena, "__log_err__(", vec![synth_atom(arena, payload)], ")"));
        }
    }
    None
}

///  `new T()` / `new T` → `__new__(T)`
fn cpp_new<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "new_expression" {
        let children = non_punct_children(node)?;
        // children: ["new", type, optional_args]
        // skip the "new" keyword atom
        let type_children: Vec<_> = children.iter()
            .filter(|c| atom_content(c).map_or(true, |s| s != "new"))
            .copied()
            .collect();
        if !type_children.is_empty() {
            let type_node = type_children[0];
            return Some(synth_list(arena, "__new__(", vec![type_node], ")"));
        }
    }

    // Flat atom "new T" / "new T()"
    if let Some(c) = atom_content(node) {
        if let Some(rest) = c.strip_prefix("new ") {
            let type_name = rest.trim_end_matches("()").trim_end_matches("{}").trim();
            return Some(synth_list(arena, "__new__(", vec![synth_atom(arena, type_name)], ")"));
        }
    }
    None
}

///  `std::string(s)` / `string(s)` → `__string__(s)`
fn cpp_string_ctor<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        let fn_name = atom_content(children.first().copied()?)?;
        if fn_name == "std::string" || fn_name == "string" {
            let args = children.get(1).copied();
            return Some(synth_list(arena, "__string__(", args.map_or(vec![], |a| vec![a]), ")"));
        }
    }
    None
}

///  `std::move(x)` / `move(x)` → `__move__(x)`
fn cpp_move<'a>(node: &'a Syntax<'a>, arena: &'a Arena<Syntax<'a>>) -> Option<&'a Syntax<'a>> {
    if node_kind(node) == "call_expression" {
        let children = list_children(node)?;
        let fn_name = atom_content(children.first().copied()?)?;
        if fn_name == "std::move" || fn_name == "move" {
            let args = children.get(1).copied();
            return Some(synth_list(arena, "__move__(", args.map_or(vec![], |a| vec![a]), ")"));
        }
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// For a `<<`-chained binary expression, extract the leftmost stream and
/// rightmost payload.  Returns `None` if the operator isn't `op`.
fn extract_stream_payload<'a, 'b>(
    node: &'b Syntax<'a>,
    op: &str,
) -> Option<(&'b &'a Syntax<'a>, &'b &'a Syntax<'a>)> {
    if node_kind(node) != "binary_expression" {
        return None;
    }
    let children = list_children(node)?;
    // children: [lhs, operator, rhs]
    if children.len() != 3 {
        return None;
    }
    if atom_content(children[1])? != op {
        return None;
    }

    // Walk left to find the stream root
    let mut stream = &children[0];
    loop {
        if node_kind(stream) == "binary_expression" {
            if let Some(ch) = list_children(stream) {
                if ch.len() == 3 && atom_content(ch[1]).map_or(false, |s| s == op) {
                    stream = &ch[0];
                    continue;
                }
            }
        }
        break;
    }
    Some((stream, &children[2]))
}

/// Strip a trailing `endl` / `"\n"` node from a stream payload.
fn strip_endl<'a>(node: &'a Syntax<'a>) -> &'a Syntax<'a> {
    // If it's a << chain whose RHS is endl/ "\n", return the LHS.
    if node_kind(node) == "binary_expression" {
        if let Some(children) = list_children(node) {
            if children.len() == 3 {
                if let Some(op) = atom_content(children[1]) {
                    if op == "<<" {
                        let rhs_str = atom_content(children[2]).unwrap_or("");
                        if rhs_str == "std::endl" || rhs_str == "endl" || rhs_str == "\"\\n\"" || rhs_str == "'\\n'" {
                            return children[0];
                        }
                    }
                }
            }
        }
    }
    node
}