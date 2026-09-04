use crate::parser::Node;
use crate::variables::{VARIABLE_LIMIT, VariableBank, collect_variables};

/// Zero-allocation error type for evaluate.  Only converted to String at the
/// display boundary, so the Newton hot path never heap-allocates on errors.
#[derive(Debug)]
pub enum EvaluationError {
    /// A variable was looked up but had no binding in VariableBank.
    UnboundVariable,
    /// The root node is an Equation — use evaluate_pending instead.
    IsEquation,
}

impl EvaluationError {
    // TODO: write a docstring and doctest
    pub fn to_string_msg(&self) -> String {
        match self {
            EvaluationError::UnboundVariable => "unbound variable".to_string(),
            EvaluationError::IsEquation => "use 'evaluate' to evaluate an equation".to_string(),
        }
    }
}
// TODO: write a docstring and doctest
pub fn evaluate(arena: &[Node], idx: u32, vars: &VariableBank) -> Result<f64, EvaluationError> {
    match unsafe { arena.get_unchecked(idx as usize) } {
        Node::Number(v) => Ok(*v),
        Node::Constant(v) => Ok(*v),

        Node::Variable(_, _, hash) => vars.get(*hash).ok_or(EvaluationError::UnboundVariable),

        // Equation nodes are not evaluated by plain evaluate; use evaluate_pending.
        Node::Equation(_, _) => Err(EvaluationError::IsEquation),
        Node::Neg(a) => Ok(-evaluate(arena, *a, vars)?),
        Node::Add(a, b) => Ok(evaluate(arena, *a, vars)? + evaluate(arena, *b, vars)?),
        Node::Sub(a, b) => Ok(evaluate(arena, *a, vars)? - evaluate(arena, *b, vars)?),
        Node::Mul(a, b) => Ok(evaluate(arena, *a, vars)? * evaluate(arena, *b, vars)?),
        Node::Div(a, b) => Ok(evaluate(arena, *a, vars)? / evaluate(arena, *b, vars)?),
        Node::Pow(a, b) => Ok(evaluate(arena, *a, vars)?.powf(evaluate(arena, *b, vars)?)),

        Node::Sin(a) => Ok(evaluate(arena, *a, vars)?.sin()),
        Node::Cos(a) => Ok(evaluate(arena, *a, vars)?.cos()),
        Node::Tan(a) => Ok(evaluate(arena, *a, vars)?.tan()),

        Node::Sec(a) => Ok(1.0 / evaluate(arena, *a, vars)?.cos()),
        Node::Csc(a) => Ok(1.0 / evaluate(arena, *a, vars)?.sin()),
        Node::Cot(a) => {
            let v = evaluate(arena, *a, vars)?;
            Ok(v.cos() / v.sin())
        }

        Node::Asin(a) => Ok(evaluate(arena, *a, vars)?.asin()),
        Node::Acos(a) => Ok(evaluate(arena, *a, vars)?.acos()),
        Node::Atan(a) => Ok(evaluate(arena, *a, vars)?.atan()),

        Node::Asec(a) => Ok((1.0 / evaluate(arena, *a, vars)?).acos()),
        Node::Acsc(a) => Ok((1.0 / evaluate(arena, *a, vars)?).asin()),
        Node::Acot(a) => Ok((1.0 / evaluate(arena, *a, vars)?).atan()),

        Node::Sinh(a) => Ok(evaluate(arena, *a, vars)?.sinh()),
        Node::Cosh(a) => Ok(evaluate(arena, *a, vars)?.cosh()),
        Node::Tanh(a) => Ok(evaluate(arena, *a, vars)?.tanh()),
        Node::Sech(a) => Ok(1.0 / evaluate(arena, *a, vars)?.cosh()),
        Node::Csch(a) => Ok(1.0 / evaluate(arena, *a, vars)?.sinh()),
        Node::Coth(a) => {
            let v = evaluate(arena, *a, vars)?;
            Ok(v.cosh() / v.sinh())
        }

        Node::Asinh(a) => Ok(evaluate(arena, *a, vars)?.asinh()),
        Node::Acosh(a) => Ok(evaluate(arena, *a, vars)?.acosh()),
        Node::Atanh(a) => Ok(evaluate(arena, *a, vars)?.atanh()),
        Node::Asech(a) => Ok((1.0 / evaluate(arena, *a, vars)?).acosh()),
        Node::Acsch(a) => Ok((1.0 / evaluate(arena, *a, vars)?).asinh()),
        Node::Acoth(a) => Ok((1.0 / evaluate(arena, *a, vars)?).atanh()),

        Node::Ln(a) => Ok(evaluate(arena, *a, vars)?.ln()),
        Node::Log(a) => Ok(evaluate(arena, *a, vars)?.log10()),
        Node::LogBase(base, arg) => {
            Ok(evaluate(arena, *arg, vars)?.log(evaluate(arena, *base, vars)?))
        }
        Node::Sqrt(a) => Ok(evaluate(arena, *a, vars)?.sqrt()),
    }
}

// TODO: write a docstring and doctest
pub fn try_simple_assign(
    arena: &[Node],
    root: u32,
    vars: &mut VariableBank,
) -> Result<Option<(u32, u32, f64)>, String> {
    let Node::Equation(lhs, rhs) = &arena[root as usize] else {
        return Ok(None); // not an equation at all
    };
    let (lhs, rhs) = (*lhs, *rhs);

    // lhs must be a plain variable
    let (var_start, var_end, var_hash) = match &arena[lhs as usize] {
        Node::Variable(start, end, hash) => (*start, *end, *hash),
        _ => return Ok(None), // complex lhs → treat as equation
    };

    // rhs must be fully evaluable right now
    let value = evaluate(arena, rhs, vars).map_err(|e| e.to_string_msg())?;
    vars.set(var_hash, value)?;
    Ok(Some((var_start, var_end, value)))
}

// TODO: write a better version of below:
// -----------------------------------------------------------------------
// Evaluate a pending equation against the VariableBank.
//
// The root must be an Equation node.  Three cases:
//
//   • 0 free vars → verify lhs == rhs (within tolerance), print both sides.
//   • 1 free var  → solve f(x) = lhs(x) - rhs(x) = 0 numerically (Newton),
//                   print  `unknown = value`.
//   • >1 free vars → return Err listing which vars still need values.
// -----------------------------------------------------------------------
pub enum EvaluationResult {
    /// All vars bound: lhs value and rhs value (should be equal for an equation).
    Verified { lhs: f64, rhs: f64 },
    /// One free var solved numerically.
    /// `values` holds 1 or 2 roots; a second root is included when `-solution`
    /// also satisfies the equation (e.g. x^2 = 4 → [2, -2]).
    Solved {
        name: String,
        values: [f64; 2],
        count: u8,
    },
    /// Expression (no `=`) evaluated to a single value.
    Value(f64),
}

// TODO: write a docstring and doctest

pub fn evaluate_pending(
    arena: &[Node],
    root: u32,
    vars: &VariableBank,
    src: &str,
) -> Result<EvaluationResult, String> {
    // Plain expression (no Equation node at root)
    let (lhs_idx, rhs_idx) = match &arena[root as usize] {
        Node::Equation(l, r) => (*l, *r),
        _ => {
            let v = evaluate(arena, root, vars).map_err(|e| e.to_string_msg())?;
            return Ok(EvaluationResult::Value(v));
        }
    };

    // Collect free variables (unbound in VariableBank).
    let mut free = collect_variables(arena, root);
    if free.is_full() {
        return Err(format!(
            "cannot evaluate: variable limit ({VARIABLE_LIMIT}) exceeded"
        ));
    }
    free.retain(|(hash, _, _)| vars.get(*hash).is_none());

    match free.len() {
        0 => {
            // All bound — evaluate both sides.
            let lhs_val = evaluate(arena, lhs_idx, vars).map_err(|e| e.to_string_msg())?;
            let rhs_val = evaluate(arena, rhs_idx, vars).map_err(|e| e.to_string_msg())?;
            Ok(EvaluationResult::Verified {
                lhs: lhs_val,
                rhs: rhs_val,
            })
        }

        1 => {
            let (unknown_hash, start, end) = free[0];
            let name = src[start as usize..end as usize].to_string();

            // f(x) = lhs(x) - rhs(x); we want f(x) = 0.
            //
            // Build a probe VariableBank once (one clone of entries + one push),
            // then update only the unknown's slot each Newton step via set_last.
            // Zero heap allocations inside the Newton loop.
            let mut probe = vars.clone_for_probe(unknown_hash);
            let mut f = |x: f64| -> Result<f64, String> {
                probe.set_last(x);
                let l = evaluate(arena, lhs_idx, &probe).map_err(|e| e.to_string_msg())?;
                let r = evaluate(arena, rhs_idx, &probe).map_err(|e| e.to_string_msg())?;
                Ok(l - r)
            };

            // Newton's method, max 64 iterations, starting at x=1.0 then x=0.0 if needed.
            let solution = newton(&mut f, 1.0)
                .or_else(|_| newton(&mut f, 0.0))
                .or_else(|_| newton(&mut f, -1.0))
                .or_else(|_| newton(&mut f, 10.0))
                .map_err(|_| {
                    format!("could not solve for '{name}'; try assigning an initial guess manually")
                })?;

            // Check whether -solution is also a root (e.g. x^2 = 4 → ±2).
            // Two extra evaluations; no extra Newton runs.
            const ROOT_TOL: f64 = 1e-6;
            let neg = -solution;
            let has_neg_root =
                solution.abs() > ROOT_TOL && f(neg).map(|v| v.abs() < ROOT_TOL).unwrap_or(false);

            let (values, count) = if has_neg_root {
                ([solution, neg], 2u8)
            } else {
                ([solution, 0.0], 1u8)
            };

            Ok(EvaluationResult::Solved {
                name,
                values,
                count,
            })
        }

        _ => {
            // Build error message without a Vec<String> intermediate.
            let mut msg = format!(
                "cannot evaluate: {} variable{} still unassigned: ",
                free.len(),
                if free.len() == 1 { "" } else { "s" },
            );
            for (i, (_, start, end)) in free.iter().enumerate() {
                if i > 0 {
                    msg.push_str(", ");
                }
                msg.push_str(&src[*start as usize..*end as usize]);
            }
            msg.push_str(
                ".\nAssign values with  name=value  or leave exactly one free for solving.",
            );
            Err(msg)
        }
    }
}

// TODO: write a docstring and doctest
fn newton(f: &mut impl FnMut(f64) -> Result<f64, String>, x0: f64) -> Result<f64, String> {
    const MAX_ITER: usize = 64;
    const TOL: f64 = 1e-10;
    const H: f64 = 1e-7;

    let mut x = x0;
    let mut fx = f(x)?;
    for _ in 0..MAX_ITER {
        if fx.abs() < TOL {
            return Ok(x);
        }
        let fpx = (f(x + H)? - f(x - H)?) / (2.0 * H);
        if fpx.abs() < 1e-14 {
            return Err("derivative too small".to_string());
        }
        let x_new = x - fx / fpx;
        if (x_new - x).abs() < TOL {
            return Ok(x_new);
        }
        x = x_new;
        fx = f(x)?;
    }
    Err("did not converge".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Tokenizer;
    use crate::parser::Parser;
    use crate::variables::{VARIABLE_LIMIT, VariableBank};

    fn parse_and_eval(src: &str) -> f64 {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let vars = VariableBank::new();
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        evaluate(&arena, root, &vars).unwrap()
    }

    #[test]
    fn test_addition() {
        assert!((parse_and_eval("1+2") - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_precedence() {
        assert!((parse_and_eval("2+3*4") - 14.0).abs() < 1e-10);
    }

    #[test]
    fn test_right_assoc_pow() {
        assert!((parse_and_eval("2^3^2") - 512.0).abs() < 1e-10);
    }

    #[test]
    fn test_parentheses() {
        assert!((parse_and_eval("(2+3)*4") - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_division() {
        assert!((parse_and_eval("10.0/4.0") - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_decimal_edges() {
        assert!((parse_and_eval(".5+.25") - 0.75).abs() < 1e-10);
        assert!((parse_and_eval("5.+.5") - 5.5).abs() < 1e-10);
    }

    #[test]
    fn test_unary_minus() {
        assert!((parse_and_eval("-3+5") - 2.0).abs() < 1e-10);
        assert!((parse_and_eval("-(2+3)") - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_constants() {
        assert!((parse_and_eval("pi") - std::f64::consts::PI).abs() < 1e-12);
        assert!((parse_and_eval("e") - std::f64::consts::E).abs() < 1e-12);
    }

    #[test]
    fn test_sin() {
        assert!(parse_and_eval("sin(0)").abs() < 1e-10);
    }

    #[test]
    fn test_cos() {
        assert!((parse_and_eval("cos(0)") - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_sqrt() {
        assert!((parse_and_eval("sqrt(9)") - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_ln() {
        assert!((parse_and_eval("ln(e)") - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_plain_log_is_base_ten() {
        assert!((parse_and_eval("log(100)") - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_log_base_two() {
        assert!((parse_and_eval("log_2(8)") - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_log_base_ten() {
        assert!((parse_and_eval("log_10(100)") - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_nested_calls() {
        assert!((parse_and_eval("sqrt(sin(0)^2+cos(0)^2)") - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_complex() {
        assert!((parse_and_eval("2+3*(4-1)^2") - 29.0).abs() < 1e-10);
    }

    #[test]
    fn test_simple_assign() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let mut vars = VariableBank::new();
        let src = "x=42";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        let result = try_simple_assign(&arena, root, &mut vars).unwrap();
        assert!(result.is_some());
        let (start, end, val) = result.unwrap();
        assert_eq!(&src[start as usize..end as usize], "x");
        assert!((val - 42.0).abs() < 1e-10);
    }

    #[test]
    fn test_assign_then_use() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let mut vars = VariableBank::new();
        // Assign x=3
        let src = "x=3";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        try_simple_assign(&arena, root, &mut vars).unwrap();
        // evaluate x*x
        arena.clear();
        let src2 = "x*x";
        Tokenizer::new(src2).tokenize(&mut tokens).unwrap();
        let root2 = Parser::new(&tokens, src2, &mut arena).parse().unwrap();
        assert!((evaluate(&arena, root2, &vars).unwrap() - 9.0).abs() < 1e-10);
    }

    #[test]
    fn test_solve_linear() {
        // x+2=5  →  x=3
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let vars = VariableBank::new();
        let src = "x+2=5";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        let result = evaluate_pending(&arena, root, &vars, src).unwrap();
        if let EvaluationResult::Solved { name, values, .. } = result {
            assert_eq!(name, "x");
            assert!((values[0] - 3.0).abs() < 1e-8);
        } else {
            panic!("expected Solved");
        }
    }

    #[test]
    fn test_solve_quadratic() {
        // x^2=9  →  x=3 or x=-3 (Newton from x0=1 → 3)
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let vars = VariableBank::new();
        let src = "x^2=9";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        let result = evaluate_pending(&arena, root, &vars, src).unwrap();
        if let EvaluationResult::Solved { values, count, .. } = result {
            assert_eq!(count, 2, "expected two roots for x^2=9");
            for i in 0..count as usize {
                assert!(
                    (values[i].abs() - 3.0).abs() < 1e-8,
                    "root {} = {} not ≈ ±3",
                    i,
                    values[i]
                );
            }
        } else {
            panic!("expected Solved");
        }
    }

    #[test]
    fn test_too_many_free_vars_error() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let vars = VariableBank::new();
        let src = "x+y=5";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        assert!(evaluate_pending(&arena, root, &vars, src).is_err());
    }

    #[test]
    fn test_varstore_limit() {
        let mut vars = VariableBank::new();
        // Fill all VARIABLE_LIMIT slots with distinct hashes.
        for i in 0..VARIABLE_LIMIT {
            vars.set(i as u64 + 1, i as f64).unwrap(); // hash 0 is the zero-init sentinel; use 1..=64
        }
        // One more new key must be rejected.
        assert!(vars.set(VARIABLE_LIMIT as u64 + 1, 1.0).is_err());
        // Updating an existing key is always ok.
        vars.set(1u64, 99.0).unwrap();
    }
}
