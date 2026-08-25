#![allow(dead_code)]
use crate::parser::Node;
// TODO: read and understand this. Delete it later.
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

// TODO: write a docstring and doctest
pub fn simplify(arena: &mut Vec<Node>, root: u32) -> u32 {
    // Clone the kind up front: arena is about to be mutated by recursive
    // calls (which push new nodes), so we can't hold a borrow into it
    // across those calls. NodeKind is cheap to clone (u32/u64/f64 fields).
    let kind = arena[root as usize].clone();

    match kind {
        // returns as-is, since it's already in the simplest form.
        Node::Number(_) | Node::Constant(_) | Node::Variable(_, _, _) => root,

        Node::Neg(a) => {
            let a = simplify(arena, a);
            // Double negation: -(-x) -> x
            if let Node::Neg(inner) = arena[a as usize] {
                return inner;
            }
            // Constant fold: -(n) -> -n
            if let Some(v) = as_number(arena, a) {
                return push(arena, Node::Number(-v));
            }
            push(arena, Node::Neg(a))
        }

        Node::Add(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                return push(arena, Node::Number(x + y));
            }
            if is_zero(arena, a) {
                return b; // 0 + x -> x
            }
            if is_zero(arena, b) {
                return a; // x + 0 -> x
            }
            push(arena, Node::Add(a, b))
        }

        Node::Sub(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                return push(arena, Node::Number(x - y));
            }
            if is_zero(arena, b) {
                return a; // x - 0 -> x
            }
            push(arena, Node::Sub(a, b))
        }

        Node::Mul(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                return push(arena, Node::Number(x * y));
            }
            // Annihilator: anything * 0 -> 0. Checked before the identity
            // rule below since 0 takes priority over 1 if somehow both
            // matched (they can't both match the same operand, but this
            // keeps the precedence explicit).
            if is_zero(arena, a) || is_zero(arena, b) {
                return push(arena, Node::Number(0.0));
            }
            if is_one(arena, a) {
                return b; // 1 * x -> x
            }
            if is_one(arena, b) {
                return a; // x * 1 -> x
            }
            push(arena, Node::Mul(a, b))
        }

        Node::Div(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            // Deliberately NOT folding x/0 or n/0 here. Division by zero
            // has no single correct symbolic rewrite (sign-dependent
            // infinity, or undefined for 0/0) — leaving it unsimplified
            // is the mathematically honest choice. eval() already handles
            // the numeric case via plain f64 semantics.
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                if y != 0.0 {
                    return push(arena, Node::Number(x / y));
                }
            }
            if is_one(arena, b) {
                return a; // x / 1 -> x
            }
            push(arena, Node::Div(a, b))
        }

        Node::Pow(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                return push(arena, Node::Number(x.powf(y)));
            }
            if is_zero(arena, b) {
                return push(arena, Node::Number(1.0)); // x^0 -> 1
            }
            if is_one(arena, b) {
                return a; // x^1 -> x
            }
            push(arena, Node::Pow(a, b))
        }

        // Equation: simplify both sides independently. No identity rules
        // apply at this level — "lhs = rhs" doesn't fold further itself.
        Node::Equation(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            push(arena, Node::Equation(a, b))
        }

        // Unary functions: simplify the argument, constant-fold if it
        // resolved to a Number, otherwise rebuild with the simplified arg.
        // No special identities here yet (e.g. sin(0)=0) — left for a
        // later pass once the core loop is validated.
        Node::Sin(a) => simplify_unary(arena, a, Node::Sin, f64::sin),
        Node::Cos(a) => simplify_unary(arena, a, Node::Cos, f64::cos),
        Node::Tan(a) => simplify_unary(arena, a, Node::Tan, f64::tan),

        Node::Sec(a) => simplify_unary(arena, a, Node::Sec, |v| 1.0 / v.cos()),
        Node::Csc(a) => simplify_unary(arena, a, Node::Csc, |v| 1.0 / v.sin()),
        Node::Cot(a) => simplify_unary(arena, a, Node::Cot, |v| v.cos() / v.sin()),

        Node::Asin(a) => simplify_unary(arena, a, Node::Asin, f64::asin),
        Node::Acos(a) => simplify_unary(arena, a, Node::Acos, f64::acos),
        Node::Atan(a) => simplify_unary(arena, a, Node::Atan, f64::atan),

        Node::Acsc(a) => simplify_unary(arena, a, Node::Acsc, |v| (1.0 / v).asin()),
        Node::Asec(a) => simplify_unary(arena, a, Node::Asec, |v| (1.0 / v).acos()),
        Node::Acot(a) => simplify_unary(arena, a, Node::Acot, |v| (1.0 / v).atan()),

        Node::Sinh(a) => simplify_unary(arena, a, Node::Sinh, f64::sinh),
        Node::Cosh(a) => simplify_unary(arena, a, Node::Cosh, f64::cosh),
        Node::Tanh(a) => simplify_unary(arena, a, Node::Tanh, f64::tanh),
        Node::Sech(a) => simplify_unary(arena, a, Node::Sech, |v| 1.0 / v.cosh()),
        Node::Csch(a) => simplify_unary(arena, a, Node::Csch, |v| 1.0 / v.sinh()),
        Node::Coth(a) => simplify_unary(arena, a, Node::Coth, |v| v.cosh() / v.sinh()),

        Node::Asinh(a) => simplify_unary(arena, a, Node::Asinh, f64::asinh),
        Node::Acosh(a) => simplify_unary(arena, a, Node::Acosh, f64::acosh),
        Node::Atanh(a) => simplify_unary(arena, a, Node::Atanh, f64::atanh),
        Node::Asech(a) => simplify_unary(arena, a, Node::Asech, |v| (1.0 / v).acosh()),
        Node::Acsch(a) => simplify_unary(arena, a, Node::Acsch, |v| (1.0 / v).asinh()),
        Node::Acoth(a) => simplify_unary(arena, a, Node::Acoth, |v| (1.0 / v).atanh()),

        Node::Ln(a) => simplify_unary(arena, a, Node::Ln, f64::ln),
        Node::Log(a) => simplify_unary(arena, a, Node::Log, f64::log10),
        Node::Sqrt(a) => simplify_unary(arena, a, Node::Sqrt, f64::sqrt),
    }
}

// TODO: write a docstring and doctest
#[inline(always)]
fn push(arena: &mut Vec<Node>, node: Node) -> u32 {
    let idx = arena.len() as u32;
    arena.push(node);
    idx
}
// TODO: improve the docstring and write a doctest
/// Returns **Some(f64)** if the node at `idx` is **NodeKind::Number**, else **None**\
/// Only applies to literal Number nodes — not Constants
#[inline(always)]
fn as_number(arena: &[Node], idx: u32) -> Option<f64> {
    match arena[idx as usize] {
        Node::Number(v) => Some(v),
        _ => None,
    }
}

// TODO: improve the docstring and write a doctest
/// Shared helper for all unary function nodes: simplify the argument,
/// constant-fold via `f` if it resolved to a Number, otherwise rebuild
/// the node (via `ctor`) pointing at the simplified argument.
#[inline(always)]
fn simplify_unary(
    arena: &mut Vec<Node>,
    arg: u32,
    ctor: fn(u32) -> Node,
    f: fn(f64) -> f64,
) -> u32 {
    let arg = simplify(arena, arg);
    if let Some(v) = as_number(arena, arg) {
        return push(arena, Node::Number(f(v)));
    }
    push(arena, ctor(arg))
}

// TODO: write a docstring and doctest
#[inline(always)]
fn is_zero(arena: &[Node], idx: u32) -> bool {
    matches!(arena[idx as usize], Node::Number(v) if v == 0.0)
}

// TODO: write a docstring and doctest
#[inline(always)]
fn is_one(arena: &[Node], idx: u32) -> bool {
    matches!(arena[idx as usize], Node::Number(v) if v == 1.0)
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
        match arena[idx as usize] {
            Node::Number(v) => {
                assert!((v - expected).abs() < 1e-10, "expected {expected}, got {v}")
            }
            ref other => panic!("expected Number({expected}), got {other:?}"),
        }
    }

    #[test]
    fn test_add_zero() {
        let (arena, root) = run("x+0");
        match arena[root as usize] {
            Node::Variable(_, _, _) => {}
            ref other => panic!("expected Variable, got {other:?}"),
        }
    }

    #[test]
    fn test_zero_add() {
        let (arena, root) = run("0+x");
        match arena[root as usize] {
            Node::Variable(_, _, _) => {}
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
        match arena[root as usize] {
            Node::Variable(_, _, _) => {}
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
        match arena[root as usize] {
            Node::Variable(_, _, _) => {}
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
        match arena[root as usize] {
            Node::Div(_, _) => {}
            ref other => panic!("expected Div left unsimplified, got {other:?}"),
        }
    }

    #[test]
    fn test_double_negation() {
        let (arena, root) = run("-(-x)");
        match arena[root as usize] {
            Node::Variable(_, _, _) => {}
            ref other => panic!("expected Variable, got {other:?}"),
        }
    }

    #[test]
    fn test_division_by_one() {
        let (arena, root) = run("x/1");
        match arena[root as usize] {
            Node::Variable(_, _, _) => {}
            ref other => panic!("expected Variable, got {other:?}"),
        }
    }

    #[test]
    fn test_sub_zero() {
        let (arena, root) = run("x-0");
        match arena[root as usize] {
            Node::Variable(_, _, _) => {}
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
