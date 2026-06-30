#![allow(dead_code)]
use crate::parser::{Node, NodeKind};

// -----------------------------------------------------------------------
// Simplify — bottom-up rewrite of the AST arena.
//
// Mirrors eval.rs structurally: same recursive shape, same arena-walking
// pattern. The difference is what gets returned at each node — eval()
// returns a computed f64, simplify() returns a u32 (the index of the
// simplified subtree's root), and may push new nodes onto the arena
// rather than just reading existing ones.
//
// Traversal order: children are simplified first (recurse to leaves),
// then the current node is checked against the rule set using the
// *already-simplified* children. This means a rule never has to look
// through an unsimplified subtree — by the time a parent checks
// "is my child the Number 0?", that child has already been folded as
// far as it can go.
//
// Append-only: simplify never mutates or removes existing arena entries.
// A rewrite is just "push a new node, return its index." This keeps the
// function safe to call independently of eval/parser and avoids any
// need to track liveness or free old nodes.
// -----------------------------------------------------------------------

#[inline(always)]
fn push(arena: &mut Vec<Node>, kind: NodeKind) -> u32 {
    let idx = arena.len() as u32;
    arena.push(Node { kind });
    idx
}

/// Returns Some(value) if the node at `idx` is a plain Number, else None.
/// Constant folding only applies to literal Number nodes — not Constant
/// (pi/e), since folding those away would lose the symbolic distinction
/// that lets later passes recognize "this came from pi" if ever needed.
#[inline(always)]
fn as_number(arena: &[Node], idx: u32) -> Option<f64> {
    match arena[idx as usize].kind {
        NodeKind::Number(v) => Some(v),
        _ => None,
    }
}

pub fn simplify(arena: &mut Vec<Node>, root: u32) -> u32 {
    // Clone the kind up front: arena is about to be mutated by recursive
    // calls (which push new nodes), so we can't hold a borrow into it
    // across those calls. NodeKind is cheap to clone (u32/u64/f64 fields).
    let kind = arena[root as usize].kind.clone();

    match kind {
        // Leaves: nothing to simplify, return as-is. No need to push a
        // new node since the original is already in its simplest form.
        NodeKind::Number(_) | NodeKind::Constant(_) | NodeKind::Variable(_, _, _) => root,

        NodeKind::Neg(a) => {
            let a = simplify(arena, a);
            // Double negation: -(-x) -> x
            if let NodeKind::Neg(inner) = arena[a as usize].kind {
                return inner;
            }
            // Constant fold: -(n) -> -n
            if let Some(v) = as_number(arena, a) {
                return push(arena, NodeKind::Number(-v));
            }
            push(arena, NodeKind::Neg(a))
        }

        NodeKind::Add(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                return push(arena, NodeKind::Number(x + y));
            }
            if is_zero(arena, a) {
                return b; // 0 + x -> x
            }
            if is_zero(arena, b) {
                return a; // x + 0 -> x
            }
            push(arena, NodeKind::Add(a, b))
        }

        NodeKind::Sub(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                return push(arena, NodeKind::Number(x - y));
            }
            if is_zero(arena, b) {
                return a; // x - 0 -> x
            }
            push(arena, NodeKind::Sub(a, b))
        }

        NodeKind::Mul(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                return push(arena, NodeKind::Number(x * y));
            }
            // Annihilator: anything * 0 -> 0. Checked before the identity
            // rule below since 0 takes priority over 1 if somehow both
            // matched (they can't both match the same operand, but this
            // keeps the precedence explicit).
            if is_zero(arena, a) || is_zero(arena, b) {
                return push(arena, NodeKind::Number(0.0));
            }
            if is_one(arena, a) {
                return b; // 1 * x -> x
            }
            if is_one(arena, b) {
                return a; // x * 1 -> x
            }
            push(arena, NodeKind::Mul(a, b))
        }

        NodeKind::Div(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            // Deliberately NOT folding x/0 or n/0 here. Division by zero
            // has no single correct symbolic rewrite (sign-dependent
            // infinity, or undefined for 0/0) — leaving it unsimplified
            // is the mathematically honest choice. eval() already handles
            // the numeric case via plain f64 semantics.
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                if y != 0.0 {
                    return push(arena, NodeKind::Number(x / y));
                }
            }
            if is_one(arena, b) {
                return a; // x / 1 -> x
            }
            push(arena, NodeKind::Div(a, b))
        }

        NodeKind::Pow(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                return push(arena, NodeKind::Number(x.powf(y)));
            }
            if is_zero(arena, b) {
                return push(arena, NodeKind::Number(1.0)); // x^0 -> 1
            }
            if is_one(arena, b) {
                return a; // x^1 -> x
            }
            push(arena, NodeKind::Pow(a, b))
        }

        // Equation: simplify both sides independently. No identity rules
        // apply at this level — "lhs = rhs" doesn't fold further itself.
        NodeKind::Equation(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            push(arena, NodeKind::Equation(a, b))
        }

        // Unary functions: simplify the argument, constant-fold if it
        // resolved to a Number, otherwise rebuild with the simplified arg.
        // No special identities here yet (e.g. sin(0)=0) — left for a
        // later pass once the core loop is validated.
        NodeKind::Sin(a) => simplify_unary(arena, a, NodeKind::Sin, f64::sin),
        NodeKind::Cos(a) => simplify_unary(arena, a, NodeKind::Cos, f64::cos),
        NodeKind::Tan(a) => simplify_unary(arena, a, NodeKind::Tan, f64::tan),

        NodeKind::Sec(a) => simplify_unary(arena, a, NodeKind::Sec, |v| 1.0 / v.cos()),
        NodeKind::Csc(a) => simplify_unary(arena, a, NodeKind::Csc, |v| 1.0 / v.sin()),
        NodeKind::Cot(a) => simplify_unary(arena, a, NodeKind::Cot, |v| v.cos() / v.sin()),

        NodeKind::Asin(a) => simplify_unary(arena, a, NodeKind::Asin, f64::asin),
        NodeKind::Acos(a) => simplify_unary(arena, a, NodeKind::Acos, f64::acos),
        NodeKind::Atan(a) => simplify_unary(arena, a, NodeKind::Atan, f64::atan),

        NodeKind::Acsc(a) => simplify_unary(arena, a, NodeKind::Acsc, |v| (1.0 / v).asin()),
        NodeKind::Asec(a) => simplify_unary(arena, a, NodeKind::Asec, |v| (1.0 / v).acos()),
        NodeKind::Acot(a) => simplify_unary(arena, a, NodeKind::Acot, |v| (1.0 / v).atan()),

        NodeKind::Sinh(a) => simplify_unary(arena, a, NodeKind::Sinh, f64::sinh),
        NodeKind::Cosh(a) => simplify_unary(arena, a, NodeKind::Cosh, f64::cosh),
        NodeKind::Tanh(a) => simplify_unary(arena, a, NodeKind::Tanh, f64::tanh),
        NodeKind::Sech(a) => simplify_unary(arena, a, NodeKind::Sech, |v| 1.0 / v.cosh()),
        NodeKind::Csch(a) => simplify_unary(arena, a, NodeKind::Csch, |v| 1.0 / v.sinh()),
        NodeKind::Coth(a) => simplify_unary(arena, a, NodeKind::Coth, |v| v.cosh() / v.sinh()),

        NodeKind::Asinh(a) => simplify_unary(arena, a, NodeKind::Asinh, f64::asinh),
        NodeKind::Acosh(a) => simplify_unary(arena, a, NodeKind::Acosh, f64::acosh),
        NodeKind::Atanh(a) => simplify_unary(arena, a, NodeKind::Atanh, f64::atanh),
        NodeKind::Asech(a) => simplify_unary(arena, a, NodeKind::Asech, |v| (1.0 / v).acosh()),
        NodeKind::Acsch(a) => simplify_unary(arena, a, NodeKind::Acsch, |v| (1.0 / v).asinh()),
        NodeKind::Acoth(a) => simplify_unary(arena, a, NodeKind::Acoth, |v| (1.0 / v).atanh()),

        NodeKind::Ln(a) => simplify_unary(arena, a, NodeKind::Ln, f64::ln),
        NodeKind::Log(a) => simplify_unary(arena, a, NodeKind::Log, f64::log10),
        NodeKind::Sqrt(a) => simplify_unary(arena, a, NodeKind::Sqrt, f64::sqrt),
    }
}

/// Shared helper for all unary function nodes: simplify the argument,
/// constant-fold via `f` if it resolved to a Number, otherwise rebuild
/// the node (via `ctor`) pointing at the simplified argument.
#[inline(always)]
fn simplify_unary(
    arena: &mut Vec<Node>,
    arg: u32,
    ctor: fn(u32) -> NodeKind,
    f: fn(f64) -> f64,
) -> u32 {
    let arg = simplify(arena, arg);
    if let Some(v) = as_number(arena, arg) {
        return push(arena, NodeKind::Number(f(v)));
    }
    push(arena, ctor(arg))
}

#[inline(always)]
fn is_zero(arena: &[Node], idx: u32) -> bool {
    matches!(arena[idx as usize].kind, NodeKind::Number(v) if v == 0.0)
}

#[inline(always)]
fn is_one(arena: &[Node], idx: u32) -> bool {
    matches!(arena[idx as usize].kind, NodeKind::Number(v) if v == 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Tokenizer;
    use crate::parser::Parser;

    fn run(src: &str) -> (Vec<Node>, u32) {
        let mut tokens = Vec::new();
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let mut arena = Vec::new();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        let simplified = simplify(&mut arena, root);
        (arena, simplified)
    }

    fn assert_number(arena: &[Node], idx: u32, expected: f64) {
        match arena[idx as usize].kind {
            NodeKind::Number(v) => {
                assert!((v - expected).abs() < 1e-10, "expected {expected}, got {v}")
            }
            ref other => panic!("expected Number({expected}), got {other:?}"),
        }
    }

    #[test]
    fn test_add_zero() {
        let (arena, root) = run("x+0");
        match arena[root as usize].kind {
            NodeKind::Variable(_, _, _) => {}
            ref other => panic!("expected Variable, got {other:?}"),
        }
    }

    #[test]
    fn test_zero_add() {
        let (arena, root) = run("0+x");
        match arena[root as usize].kind {
            NodeKind::Variable(_, _, _) => {}
            ref other => panic!("expected Variable, got {other:?}"),
        }
    }

    #[test]
    fn test_mul_zero() {
        let (arena, root) = run("x*0");
        assert_number(&arena, root, 0.0);
    }

    #[test]
    fn test_mul_one() {
        let (arena, root) = run("x*1");
        match arena[root as usize].kind {
            NodeKind::Variable(_, _, _) => {}
            ref other => panic!("expected Variable, got {other:?}"),
        }
    }

    #[test]
    fn test_pow_zero() {
        let (arena, root) = run("x^0");
        assert_number(&arena, root, 1.0);
    }

    #[test]
    fn test_pow_one() {
        let (arena, root) = run("x^1");
        match arena[root as usize].kind {
            NodeKind::Variable(_, _, _) => {}
            ref other => panic!("expected Variable, got {other:?}"),
        }
    }

    #[test]
    fn test_constant_folding() {
        let (arena, root) = run("2+3");
        assert_number(&arena, root, 5.0);
    }

    #[test]
    fn test_nested_example_from_spec() {
        // 2*x*0 + 1 -> 1, the worked example from the design discussion.
        let (arena, root) = run("2*x*0+1");
        assert_number(&arena, root, 1.0);
    }

    #[test]
    fn test_div_by_zero_not_simplified() {
        // x/0 should NOT be folded — left as-is per design decision.
        let (arena, root) = run("x/0");
        match arena[root as usize].kind {
            NodeKind::Div(_, _) => {}
            ref other => panic!("expected Div left unsimplified, got {other:?}"),
        }
    }

    #[test]
    fn test_double_negation() {
        let (arena, root) = run("-(-x)");
        match arena[root as usize].kind {
            NodeKind::Variable(_, _, _) => {}
            ref other => panic!("expected Variable, got {other:?}"),
        }
    }

    #[test]
    fn test_division_by_one() {
        let (arena, root) = run("x/1");
        match arena[root as usize].kind {
            NodeKind::Variable(_, _, _) => {}
            ref other => panic!("expected Variable, got {other:?}"),
        }
    }

    #[test]
    fn test_sub_zero() {
        let (arena, root) = run("x-0");
        match arena[root as usize].kind {
            NodeKind::Variable(_, _, _) => {}
            ref other => panic!("expected Variable, got {other:?}"),
        }
    }

    #[test]
    fn test_unary_constant_fold() {
        let (arena, root) = run("sin(0)");
        assert_number(&arena, root, 0.0);
    }

    #[test]
    fn test_implicit_mul_with_paren_simplifies() {
        // x(0) parses as x*0 since our parser fix; should fold to 0 too.
        let (arena, root) = run("x(0)");
        assert_number(&arena, root, 0.0);
    }
}
