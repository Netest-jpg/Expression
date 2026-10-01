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
                return push_node(arena, Node::Number(-v)); // -(n) -> -n
            }
            push_node(arena, Node::Neg(a))
        }

        Node::Add(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                return push_node(arena, Node::Number(x + y)); // 2 + 3 -> 5
            }
            if is_zero(arena, a) {
                return b; // 0 + x -> x
            }
            if is_zero(arena, b) {
                return a; // x + 0 -> x
            }
            push_node(arena, Node::Add(a, b))
        }

        Node::Sub(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                return push_node(arena, Node::Number(x - y)); // 3 - 2 -> 1
            }
            if is_zero(arena, b) {
                return a; // x - 0 -> x
            }
            push_node(arena, Node::Sub(a, b))
        }

        Node::Mul(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) {
                return push_node(arena, Node::Number(x * y)); // 3 * 5 -> 15
            }
            if is_zero(arena, a) || is_zero(arena, b) {
                return push_node(arena, Node::Number(0.0)); // a * 0 -> 0 || 0 * b -> 0
            }
            if is_one(arena, a) {
                return b; // 1 * x -> x
            }
            if is_one(arena, b) {
                return a; // x * 1 -> x
            }
            push_node(arena, Node::Mul(a, b))
        }
        #[rustfmt::skip]
        Node::Div(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            if let (Some(x), Some(y)) = (as_number(arena, a), as_number(arena, b)) && y != 0.0 {
                return push_node(arena, Node::Number(x / y)); // 15 / 5 -> 3
            }
            if is_one(arena, b) {
                return a; // x / 1 -> x
            }
            push_node(arena, Node::Div(a, b))
        }

        #[rustfmt::skip]
        Node::Pow(base, exponent) => {
            let base = simplify(arena, base);
            let exponent = simplify(arena, exponent);

            if let (Some(x), Some(y)) = (as_number(arena, base), as_number(arena, exponent)) {
                return push_node(arena, Node::Number(x.powf(y))); // 2^2 -> 4
            }
            if is_zero(arena, exponent) {
                return push_node(arena, Node::Number(1.0)); // x^0 -> 1
            }
            if is_one(arena, exponent) {
                return base; // x^1 -> x
            }
            if let Node::LogBase(log_base, arg) = arena[exponent as usize]
                && let (Node::Number(b1), Node::Number(b2)) = (&arena[base as usize], &arena[log_base as usize])
                && (*b2 > 0.0 && *b2 != 1.0 && b1 == b2) {
                    return arg; // b^log_b(k) = k where, b is a Number
                }
            push_node(arena, Node::Pow(base, exponent))
        }

        Node::Equation(a, b) => {
            let a = simplify(arena, a);
            let b = simplify(arena, b);
            push_node(arena, Node::Equation(a, b))
        }

        Node::Deg(a) => {
            let a = simplify(arena, a);
            push_node(arena, Node::Deg(a))
        }
        Node::Rad(a) => {
            let a = simplify(arena, a);
            push_node(arena, Node::Rad(a))
        }

        Node::Sin(a) => simplify_trig(arena, a, Node::Sin, f64::sin, crate::angles::sin_deg),
        Node::Cos(a) => simplify_trig(arena, a, Node::Cos, f64::cos, crate::angles::cos_deg),
        Node::Tan(a) => simplify_trig(arena, a, Node::Tan, f64::tan, crate::angles::tan_deg),

        Node::Sec(a) => simplify_trig(
            arena,
            a,
            Node::Sec,
            |v| 1.0 / v.cos(),
            |v| 1.0 / crate::angles::cos_deg(v),
        ),
        Node::Csc(a) => simplify_trig(
            arena,
            a,
            Node::Csc,
            |v| 1.0 / v.sin(),
            |v| 1.0 / crate::angles::sin_deg(v),
        ),
        Node::Cot(a) => simplify_trig(
            arena,
            a,
            Node::Cot,
            |v| v.cos() / v.sin(),
            |v| crate::angles::cos_deg(v) / crate::angles::sin_deg(v),
        ),

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
                        return push_node(arena, Node::Number(0.0)); // log_2(1) -> 0
                    }
                    if arg == base {
                        return push_node(arena, Node::Number(1.0)); // log_2(2) -> 1
                    }
                    return push_node(arena, Node::Number(arg.log(base))); // log_2(8) -> 3
                }

            push_node(arena, Node::LogBase(base, arg))
        }
        Node::Sqrt(a) => simplify_unary(arena, a, Node::Sqrt, f64::sqrt),
        Node::Cbrt(a) => simplify_unary(arena, a, Node::Cbrt, f64::cbrt),
        Node::Root(n, x) => {
            let n = simplify(arena, n);
            let x = simplify(arena, x);
            if is_one(arena, n) {
                return x; // root_1(x) -> x
            }
            if let (Some(nv), Some(xv)) = (as_number(arena, n), as_number(arena, x)) {
                let r = crate::evaluation::nth_root(nv, xv);
                if !r.is_nan() {
                    return push_node(arena, Node::Number(r)); // root_3(27) -> 3
                }
            }
            push_node(arena, Node::Root(n, x))
        }
    }
}

#[inline(always)]
fn push_node(arena: &mut Vec<Node>, node: Node) -> u32 {
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
        return push_node(arena, Node::Number(f(v)));
    }
    push_node(arena, ctor(arg))
}

/// Like `simplify_unary`, but for forward trig: the argument is a `Deg`/`Rad` wrapper.
/// Folds through `f_deg`/`f_rad` when the wrapped value is a Number, otherwise rebuilds.
fn simplify_trig(
    arena: &mut Vec<Node>,
    arg: u32,
    ctor: fn(u32) -> Node,
    f_rad: fn(f64) -> f64,
    f_deg: fn(f64) -> f64,
) -> u32 {
    let arg = simplify(arena, arg); // simplifies the inner expression, keeps the wrapper
    match arena[arg as usize] {
        Node::Deg(inner) => {
            if let Some(v) = as_number(arena, inner) {
                return push_node(arena, Node::Number(f_deg(v)));
            }
        }
        Node::Rad(inner) => {
            if let Some(v) = as_number(arena, inner) {
                return push_node(arena, Node::Number(f_rad(v)));
            }
        }
        _ => {}
    }
    push_node(arena, ctor(arg))
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

    #[test]
    fn test_trig_degrees_fold() {
        let (arena, root) = run("sin(30)");
        assert_number(&arena, root, 0.5);
        let (arena, root) = run("cos(90)");
        assert_number(&arena, root, 0.0);
    }

    #[test]
    fn test_symbolic_trig_keeps_unit() {
        let (arena, root) = run("sin(x Rad)");
        let Node::Sin(a) = arena[root as usize] else {
            panic!("expected Sin")
        };
        assert!(matches!(arena[a as usize], Node::Rad(_)));
    }

    #[test]
    fn test_root_fold_and_symbolic() {
        let (arena, root) = run("root_3(27)");
        assert_number(&arena, root, 3.0);

        let (arena, root) = run("root_2(-4)"); // NaN, so left unfolded
        assert!(matches!(arena[root as usize], Node::Root(_, _)));

        let (arena, root) = run("root_n(x)");
        assert!(matches!(arena[root as usize], Node::Root(_, _)));
    }

    #[test]
    fn test_cbrt_fold() {
        let (arena, root) = run("cbrt(27)");
        assert_number(&arena, root, 3.0);
        let (arena, root) = run("cbrt(-8)");
        assert_number(&arena, root, -2.0);
    }
}
