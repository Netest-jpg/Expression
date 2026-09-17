#![allow(dead_code)]
use crate::parser::Node;
/// Recursively simplifies the expression subtree rooted at `root`: applies constant folding
/// and algebraic identities from bottom-up, and returns the arena index of the result.
///
/// New nodes are pushed onto the end of `arena` rather than overwriting old ones, so the
/// original subtree at `root` is left intact but unreachable. Clone the arena first if you
/// need to keep the pre-simplified tree around.
///
/// This is a single bottom-up pass, not a fixed-point loop — expressions needing more than
/// one round of folding may come out only partially simplified.
///
/// # Example
/// ```ignore
/// // "2*x*0+1" -> 1
/// let simplified_root = simplify(&mut arena, root);
/// assert_eq!(arena[simplified_root as usize], Node::Number(1.0));
/// ```
pub fn simplify(arena: &mut Vec<Node>, root: u32) -> u32 {
    let kind = arena[root as usize].clone();
    match kind {
        Node::Number(_) | Node::Constant(_) | Node::Variable(_, _, _) => root,

        Node::Neg(a) => {
            let a = simplify(arena, a);
            if let Node::Neg(inner) = arena[a as usize] {
                return inner; // -(-x) -> x
            }
            if let Some(v) = as_number(arena, a) {
                return push(arena, Node::Number(-v)); // -(n) -> -n
            }
            push(arena, Node::Neg(a))
        }

        Node::Add(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                return push(arena, Node::Number(x + y)); // 2 + 3 -> 5
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
                return push(arena, Node::Number(x - y)); // 3 - 2 -> 1
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
                return push(arena, Node::Number(x * y)); // 3 * 5 -> 15
            }
            if is_zero(arena, a) || is_zero(arena, b) {
                return push(arena, Node::Number(0.0)); // a * 0 -> 0 || 0 * b -> 0
            }
            if is_one(arena, a) {
                return b; // 1 * x -> x
            }
            if is_one(arena, b) {
                return a; // x * 1 -> x
            }
            push(arena, Node::Mul(a, b))
        }
        #[rustfmt::skip]
        Node::Div(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) && y != 0.0 {
                return push(arena, Node::Number(x / y)); // 15 / 5 -> 3
            }
            if is_one(arena, b) {
                return a; // x / 1 -> x
            }
            push(arena, Node::Div(a, b))
        }

        #[rustfmt::skip]
        Node::Pow(base, exponent) => {
            let base = simplify(arena, base);
            let exponent = simplify(arena, exponent);

            if let (Some(x), Some(y)) = (as_number(arena, base), as_number(arena, exponent)) {
                return push(arena, Node::Number(x.powf(y))); // 2^2 -> 4
            }
            if is_zero(arena, exponent) {
                return push(arena, Node::Number(1.0)); // x^0 -> 1
            }
            if is_one(arena, exponent) {
                return base; // x^1 -> x
            }
            if let Node::LogBase(log_base, arg) = arena[exponent as usize]
                && let (Node::Number(b1), Node::Number(b2)) = (&arena[base as usize], &arena[log_base as usize])
                && (*b2 > 0.0 && *b2 != 1.0 && b1 == b2) {
                    return arg; // b^log_b(k) = k where, b is a Number
                }
            push(arena, Node::Pow(base, exponent))
        }

        Node::Equation(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            push(arena, Node::Equation(a, b))
        }

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
        #[rustfmt::skip]
        Node::LogBase(base, arg) => {
            let base = simplify(arena, base);
            let arg = simplify(arena, arg);
            if let Node::Pow(inner_base, k) = arena[arg as usize]
                && let (Node::Number(b1), Node::Number(b2)) = (&arena[base as usize], &arena[inner_base as usize])
                && (*b1 > 0.0 && *b1 != 1.0 && b1 == b2){
                    return k; // log_b(b^k) -> k
                }

            if let (Some(base), Some(arg)) = (as_number(arena, base), as_number(arena, arg))
                && (base > 0.0 && base != 1.0) {
                    if arg == 1.0 {
                        return push(arena, Node::Number(0.0)); // log_2(1) -> 0
                    }
                    if arg == base {
                        return push(arena, Node::Number(1.0)); // log_2(2) -> 1
                    }
                    return push(arena, Node::Number(arg.log(base))); // log_2(8) -> 3
                }

            push(arena, Node::LogBase(base, arg))
        }
        Node::Sqrt(a) => simplify_unary(arena, a, Node::Sqrt, f64::sqrt),
    }
}

#[inline(always)]
fn push(arena: &mut Vec<Node>, node: Node) -> u32 {
    let idx = arena.len() as u32;
    arena.push(node);
    idx
}

/// Returns f64 if the node at `idx` is `NodeKind::Number`, else `None`
/// Only applies to literal Number nodes — not Constants
#[inline(always)]
fn as_number(arena: &[Node], idx: u32) -> Option<f64> {
    match arena[idx as usize] {
        Node::Number(v) => Some(v),
        _ => None,
    }
}

/// Shared helper for all unary function nodes: simplify the argument, constant-fold via `f` if it resolved to a Number, otherwise rebuild the node (via `ctor`) pointing at the simplified argument.
#[rustfmt::skip]
#[inline(always)]
fn simplify_unary(arena: &mut Vec<Node>, arg: u32, ctor: fn(u32) -> Node, f: fn(f64) -> f64) -> u32 {
    let arg = simplify(arena, arg);
    if let Some(v) = as_number(arena, arg) {
        return push(arena, Node::Number(f(v)));
    }
    push(arena, ctor(arg))
}

#[inline(always)]
fn is_zero(arena: &[Node], idx: u32) -> bool {
    matches!(arena[idx as usize], Node::Number(v) if v == 0.0)
}

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
    fn test_log_base_constant_fold() {
        let (arena, root) = run("log_2(8)");
        assert_number(&arena, root, 3.0);
    }

    #[test]
    fn test_symbolic_log_base_stays_symbolic() {
        let (arena, root) = run("log_b(k)");
        match arena[root as usize] {
            Node::LogBase(_, _) => {}
            ref other => panic!("expected LogBase, got {other:?}"),
        }
    }

    #[test]
    fn test_implicit_mul_with_paren_simplifies() {
        // x(0) parses as x*0 since our parser fix; should fold to 0 too.
        let (arena, root) = run("x(0)");
        assert_number(&arena, root, 0.0);
    }

    #[test]
    fn test_log_base_power_numeric_valid() {
        let (arena, root) = run("log_2(2^5)");
        assert_number(&arena, root, 5.0);
    }

    #[test]
    fn test_log_base_power_base_one_not_folded() {
        // 1^5 = 1, but log_1(1) is indeterminate — must NOT fold to 5.
        let (arena, root) = run("log_1(1^5)");
        match arena[root as usize] {
            Node::LogBase(_, _) => {}
            ref other => panic!("expected LogBase left unsimplified, got {other:?}"),
        }
    }

    #[test]
    fn test_log_base_power_symbolic_not_folded() {
        let (arena, root) = run("log_y(y^k)");
        match arena[root as usize] {
            Node::LogBase(_, _) => {}
            ref other => panic!("expected LogBase left unsimplified, got {other:?}"),
        }
    }

    #[test]
    fn test_pow_log_base_numeric_valid() {
        // 2^log_2(8) -> 8
        let (arena, root) = run("2^log_2(8)");
        assert_number(&arena, root, 8.0);
    }

    #[test]
    fn test_pow_log_base_symbolic_not_folded() {
        let (arena, root) = run("y^log_y(k)");
        match arena[root as usize] {
            Node::Pow(_, _) => {}
            ref other => panic!("expected Pow left unsimplified, got {other:?}"),
        }
    }
}
