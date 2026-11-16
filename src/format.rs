use expression::lexer::{BP_ADD, BP_EXP, BP_MUL, Token};
use expression::parser::Node;
use std::io::Write;

pub fn write_tokens<W: Write>(out: &mut W, tokens: &[Token], src: &str) -> std::io::Result<()> {
    write!(out, "  Tokens:")?;
    for token in tokens {
        match token {
            Token::EndOfFile => {}
            Token::Number { start, end } => {
                let s = &src[*start as usize..*end as usize];
                write!(out, " {s}")?;
            }
            Token::Variable { start, end, .. } => {
                let s = &src[*start as usize..*end as usize];
                write!(out, " {s}")?;
            }
            other => {
                let s = match other {
                    Token::Plus => "+",
                    Token::Minus => "-",
                    Token::Multiply => "*",
                    Token::Divide => "/",
                    Token::Exponent => "^",
                    Token::LeftParenthesis => "(",
                    Token::RightParenthesis => ")",
                    Token::Factorial => "!",
                    Token::Equals => "=",
                    _ => unreachable!(),
                };
                write!(out, " {s}")?;
            }
        }
    }
    Ok(())
}

#[inline(always)]
pub fn write_value<W: Write>(out: &mut W, value: f64) -> std::io::Result<()> {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        let mut buf = core::fmt::NumBuffer::<i64>::new();
        out.write_all((value as i64).format_into(&mut buf).as_bytes())
    } else {
        let mut buf = zmij::Buffer::new();
        let s = buf.format(value);
        // print the exponent marker as `E` so output can be pasted back as input
        match s.split_once('e') {
            Some((mantissa, exponent)) => {
                out.write_all(mantissa.as_bytes())?;
                out.write_all(b"E")?;
                out.write_all(exponent.as_bytes())
            }
            None => out.write_all(s.as_bytes()),
        }
    }
}

pub fn write_node_recursive<W: Write>(
    out: &mut W,
    idx: u32,
    arena: &[Node],
    src: &str,
) -> std::io::Result<()> {
    match &arena[idx as usize] {
        Node::Number(v) => write_value(out, *v),
        Node::Constant(v) => {
            if (v - std::f64::consts::PI).abs() < 1e-14 {
                write!(out, "π")
            } else if (v - std::f64::consts::E).abs() < 1e-14 {
                write!(out, "e")
            } else {
                write_value(out, *v)
            }
        }
        Node::Variable(s, e, _) => write!(out, "{}", &src[*s as usize..*e as usize]),
        Node::Neg(a) => {
            let a = *a;
            write!(out, "-(")?;
            write_node_recursive(out, a, arena, src)?;
            write!(out, ")")
        }
        Node::Equation(a, b) => {
            let (a, b) = (*a, *b);
            write_node_recursive(out, a, arena, src)?;
            write!(out, " = ")?;
            write_node_recursive(out, b, arena, src)
        }
        Node::Add(a, b) => {
            let (a, b) = (*a, *b);
            write_child(out, a, arena, src, BP_ADD, Side::Left)?;
            write!(out, " + ")?;
            write_child(out, b, arena, src, BP_ADD, Side::Right)
        }
        Node::Sub(a, b) => {
            let (a, b) = (*a, *b);
            write_child(out, a, arena, src, BP_ADD, Side::Left)?;
            write!(out, " - ")?;
            write_child(out, b, arena, src, BP_ADD, Side::Right)
        }
        Node::Mul(a, b) => {
            let (a, b) = (*a, *b);
            write_child(out, a, arena, src, BP_MUL, Side::Left)?;
            write!(out, " * ")?;
            write_child(out, b, arena, src, BP_MUL, Side::Right)
        }
        Node::Div(a, b) => {
            let (a, b) = (*a, *b);
            write_child(out, a, arena, src, BP_MUL, Side::Left)?;
            write!(out, " / ")?;
            write_child(out, b, arena, src, BP_MUL, Side::Right)
        }
        Node::Pow(a, b) => {
            let (a, b) = (*a, *b);
            write_child(out, a, arena, src, BP_EXP, Side::Left)?;
            write!(out, " ^ ")?;
            write_child(out, b, arena, src, BP_EXP, Side::Right)
        }
        Node::Deg(a) => {
            write_node_recursive(out, *a, arena, src)?;
            write!(out, " Deg")
        }
        Node::Rad(a) => {
            write_node_recursive(out, *a, arena, src)?;
            write!(out, " Rad")
        }

        Node::Sin(a) => write_unary_recursive(out, "sin", *a, arena, src),
        Node::Cos(a) => write_unary_recursive(out, "cos", *a, arena, src),
        Node::Tan(a) => write_unary_recursive(out, "tan", *a, arena, src),

        Node::Ln(a) => write_unary_recursive(out, "ln", *a, arena, src),
        Node::Log(a) => write_unary_recursive(out, "log", *a, arena, src),
        Node::LogBase(base, arg) => write_subscript_recursive(out, "log_", *base, *arg, arena, src),

        Node::Sqrt(a) => write_unary_recursive(out, "sqrt", *a, arena, src),
        Node::Cbrt(a) => write_unary_recursive(out, "cbrt", *a, arena, src),
        Node::Root(n, x) => write_subscript_recursive(out, "root_", *n, *x, arena, src),

        Node::Factorial(a) => {
            let a = *a;
            let child = &arena[a as usize];
            // Parenthesize anything that would re-parse differently under a postfix `!`:
            // binary operators, `-(..)`, equations, and negative number literals.
            let needs_parens = node_bp(child).is_some()
                || matches!(child, Node::Neg(_) | Node::Equation(_, _))
                || matches!(child, Node::Number(v) if v.is_sign_negative());
            if needs_parens {
                write!(out, "(")?;
            }
            write_node_recursive(out, a, arena, src)?;
            if needs_parens {
                write!(out, ")")?;
            }
            write!(out, "!")
        }

        Node::Sec(a) => write_unary_recursive(out, "sec", *a, arena, src),
        Node::Csc(a) => write_unary_recursive(out, "csc", *a, arena, src),
        Node::Cot(a) => write_unary_recursive(out, "cot", *a, arena, src),

        Node::Asin(a) => write_unary_recursive(out, "asin", *a, arena, src),
        Node::Acos(a) => write_unary_recursive(out, "acos", *a, arena, src),
        Node::Atan(a) => write_unary_recursive(out, "atan", *a, arena, src),
        Node::Acsc(a) => write_unary_recursive(out, "acsc", *a, arena, src),
        Node::Asec(a) => write_unary_recursive(out, "asec", *a, arena, src),
        Node::Acot(a) => write_unary_recursive(out, "acot", *a, arena, src),

        Node::Sinh(a) => write_unary_recursive(out, "sinh", *a, arena, src),
        Node::Cosh(a) => write_unary_recursive(out, "cosh", *a, arena, src),
        Node::Tanh(a) => write_unary_recursive(out, "tanh", *a, arena, src),
        Node::Sech(a) => write_unary_recursive(out, "sech", *a, arena, src),
        Node::Csch(a) => write_unary_recursive(out, "csch", *a, arena, src),
        Node::Coth(a) => write_unary_recursive(out, "coth", *a, arena, src),

        Node::Asinh(a) => write_unary_recursive(out, "asinh", *a, arena, src),
        Node::Acosh(a) => write_unary_recursive(out, "acosh", *a, arena, src),
        Node::Atanh(a) => write_unary_recursive(out, "atanh", *a, arena, src),
        Node::Asech(a) => write_unary_recursive(out, "asech", *a, arena, src),
        Node::Acsch(a) => write_unary_recursive(out, "acsch", *a, arena, src),
        Node::Acoth(a) => write_unary_recursive(out, "acoth", *a, arena, src),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
}

fn node_bp(node: &Node) -> Option<u8> {
    match node {
        Node::Add(_, _) | Node::Sub(_, _) => Some(BP_ADD),
        Node::Mul(_, _) | Node::Div(_, _) => Some(BP_MUL),
        Node::Pow(_, _) => Some(BP_EXP),
        _ => None,
    }
}

fn write_child<W: Write>(
    out: &mut W,
    idx: u32,
    arena: &[Node],
    src: &str,
    parent_bp: u8,
    side: Side,
) -> std::io::Result<()> {
    let needs_parens = match node_bp(&arena[idx as usize]) {
        None => false,
        Some(child_bp) => {
            if child_bp < parent_bp {
                true
            } else if child_bp > parent_bp {
                false
            } else {
                if parent_bp == BP_EXP {
                    side == Side::Left
                } else {
                    // Still need parens for Sub/Div . Specifically on the right,
                    // since a - (b - c) != (a - b) - c and a / (b / c) != (a / b) / c, even though both share BP_ADD / BP_MUL with + and *.
                    let child_is_non_assoc =
                        matches!(arena[idx as usize], Node::Sub(_, _) | Node::Div(_, _));
                    child_is_non_assoc && side == Side::Right
                }
            }
        }
    };
    if needs_parens {
        write!(out, "(")?;
        write_node_recursive(out, idx, arena, src)?;
        write!(out, ")")
    } else {
        write_node_recursive(out, idx, arena, src)
    }
}

#[inline(always)]
fn write_unary_recursive<W: Write>(
    out: &mut W,
    name: &str,
    a: u32,
    arena: &[Node],
    src: &str,
) -> std::io::Result<()> {
    write!(out, "{name}(")?;
    write_node_recursive(out, a, arena, src)?;
    write!(out, ")")
}

fn write_subscript_recursive<W: Write>(
    out: &mut W,
    prefix: &str,
    base: u32,
    arg: u32,
    arena: &[Node],
    src: &str,
) -> std::io::Result<()> {
    write!(out, "{prefix}")?;
    if is_subscript_atomic(&arena[base as usize]) {
        write_node_recursive(out, base, arena, src)?;
    } else {
        write!(out, "(")?;
        write_node_recursive(out, base, arena, src)?;
        write!(out, ")")?;
    }
    write!(out, "(")?;
    write_node_recursive(out, arg, arena, src)?;
    write!(out, ")")
}

fn is_subscript_atomic(node: &Node) -> bool {
    matches!(
        node,
        Node::Number(_) | Node::Constant(_) | Node::Variable(_, _, _)
    )
}
