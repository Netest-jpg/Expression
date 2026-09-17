use expression::lexer::{BP_ADD, BP_EXP, BP_MUL, Token};
use expression::parser::Node;
use std::io::Write;

#[rustfmt::skip]
pub fn write_tokens<W: Write>(out: &mut W, tokens: &[Token], src: &str, verbose: bool) -> std::io::Result<()> {
    write!(out, "Tokens:")?;
    for token in tokens {
        match token {
            Token::EndOfFile => {}
            Token::Number { start, end } => {
                let s = &src[*start as usize..*end as usize];
                if verbose {
                    write!(out, "\n  Number({s})")?;
                } else {
                    write!(out, " {s}")?;
                }
            }
            Token::Variable { start, end, .. } => {
                let s = &src[*start as usize..*end as usize];
                if verbose {
                    write!(out, "\n  Identifier({s})")?;
                } else {
                    write!(out, " {s}")?;
                }
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
                    Token::Equals => "=",
                    _ => unreachable!(),
                };
                if verbose {
                    write!(out, "\n  {s}")?;
                } else {
                    write!(out, " {s}")?;
                }
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
        out.write_all(buf.format(value).as_bytes())
    }
}

#[rustfmt::skip]
pub fn write_node_recursive<W: Write>(out: &mut W, idx: u32, arena: &[Node], src: &str) -> std::io::Result<()> {
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

        Node::Sin(a) => write_unary_recursive(out, "sin", *a, arena, src),
        Node::Cos(a) => write_unary_recursive(out, "cos", *a, arena, src),
        Node::Tan(a) => write_unary_recursive(out, "tan", *a, arena, src),

        Node::Ln(a) => write_unary_recursive(out, "ln", *a, arena, src),
        Node::Log(a) => write_unary_recursive(out, "log", *a, arena, src),
        Node::LogBase(base, arg) => write_log_base_recursive(out, *base, *arg, arena, src),
        Node::Sqrt(a) => write_unary_recursive(out, "sqrt", *a, arena, src),

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

#[rustfmt::skip]
pub fn write_arena<W: Write>(out: &mut W, root: u32, arena: &[Node], src: &str, verbose: bool) -> std::io::Result<()> {
    write!(out, "AST root: n{root}\nAST arena:")?;
    for (i, node) in arena.iter().enumerate() {
        write!(out, "\n  n{i}: ")?;
        if verbose {
            write_node_verbose(out, node, src)?;
        } else {
            write_node_compact(out, node, src)?;
        }
    }
    Ok(())
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

fn write_node_compact<W: Write>(out: &mut W, node: &Node, src: &str) -> std::io::Result<()> {
    match node {
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
        Node::Variable(start, end, _) => write!(out, "{}", &src[*start as usize..*end as usize]),
        Node::Neg(a) => write!(out, "-(n{a})"),
        Node::Equation(a, b) => write!(out, "n{a} = n{b}"),

        Node::Add(a, b) => write!(out, "n{a} + n{b}"),
        Node::Sub(a, b) => write!(out, "n{a} - n{b}"),
        Node::Mul(a, b) => write!(out, "n{a} * n{b}"),
        Node::Div(a, b) => write!(out, "n{a} / n{b}"),
        Node::Pow(a, b) => write!(out, "n{a} ^ n{b}"),

        Node::Sin(a) => write!(out, "sin(n{a})"),
        Node::Cos(a) => write!(out, "cos(n{a})"),
        Node::Tan(a) => write!(out, "tan(n{a})"),

        Node::Ln(a) => write!(out, "ln(n{a})"),
        Node::Log(a) => write!(out, "log(n{a})"),
        Node::LogBase(base, arg) => write!(out, "log_n{base}(n{arg})"),
        Node::Sqrt(a) => write!(out, "sqrt(n{a})"),

        Node::Sec(a) => write!(out, "sec(n{a})"),
        Node::Csc(a) => write!(out, "csc(n{a})"),
        Node::Cot(a) => write!(out, "cot(n{a})"),

        Node::Asin(a) => write!(out, "asin(n{a})"),
        Node::Acos(a) => write!(out, "acos(n{a})"),
        Node::Atan(a) => write!(out, "atan(n{a})"),
        Node::Acsc(a) => write!(out, "acsc(n{a})"),
        Node::Asec(a) => write!(out, "asec(n{a})"),
        Node::Acot(a) => write!(out, "acot(n{a})"),

        Node::Sinh(a) => write!(out, "sinh(n{a})"),
        Node::Cosh(a) => write!(out, "cosh(n{a})"),
        Node::Tanh(a) => write!(out, "tanh(n{a})"),
        Node::Sech(a) => write!(out, "sech(n{a})"),
        Node::Csch(a) => write!(out, "csch(n{a})"),
        Node::Coth(a) => write!(out, "coth(n{a})"),

        Node::Asinh(a) => write!(out, "asinh(n{a})"),
        Node::Acosh(a) => write!(out, "acosh(n{a})"),
        Node::Atanh(a) => write!(out, "atanh(n{a})"),
        Node::Asech(a) => write!(out, "asech(n{a})"),
        Node::Acsch(a) => write!(out, "acsch(n{a})"),
        Node::Acoth(a) => write!(out, "acoth(n{a})"),
    }
}

fn write_node_verbose<W: Write>(out: &mut W, node: &Node, src: &str) -> std::io::Result<()> {
    match node {
        Node::Number(v) => {
            write!(out, "Number(")?;
            write_value(out, *v)?;
            write!(out, ")")
        }
        Node::Constant(v) => {
            write!(out, "Constant(")?;
            write_value(out, *v)?;
            write!(out, ")")
        }
        Node::Variable(s, e, _) => {
            write!(out, "Variable({:?})", &src[*s as usize..*e as usize])
        }
        Node::Neg(c) => write!(out, "Neg(n{c})"),
        Node::Equation(l, r) => write!(out, "Equation(n{l}, n{r})"),

        Node::Add(l, r) => write!(out, "Add(n{l}, n{r})"),
        Node::Sub(l, r) => write!(out, "Sub(n{l}, n{r})"),
        Node::Mul(l, r) => write!(out, "Mul(n{l}, n{r})"),
        Node::Div(l, r) => write!(out, "Div(n{l}, n{r})"),
        Node::Pow(l, r) => write!(out, "Pow(n{l}, n{r})"),

        Node::Sin(c) => write!(out, "Sin(n{c})"),
        Node::Cos(c) => write!(out, "Cos(n{c})"),
        Node::Tan(c) => write!(out, "Tan(n{c})"),

        Node::Ln(c) => write!(out, "Ln(n{c})"),
        Node::Log(c) => write!(out, "Log(n{c})"),
        Node::LogBase(base, arg) => write!(out, "LogBase(n{base}, n{arg})"),
        Node::Sqrt(c) => write!(out, "Sqrt(n{c})"),

        Node::Sec(c) => write!(out, "Sec(n{c})"),
        Node::Csc(c) => write!(out, "Csc(n{c})"),
        Node::Cot(c) => write!(out, "Cot(n{c})"),

        Node::Asin(c) => write!(out, "Asin(n{c})"),
        Node::Acos(c) => write!(out, "Acos(n{c})"),
        Node::Atan(c) => write!(out, "Atan(n{c})"),
        Node::Acsc(c) => write!(out, "Acsc(n{c})"),
        Node::Asec(c) => write!(out, "Asec(n{c})"),
        Node::Acot(c) => write!(out, "Acot(n{c})"),

        Node::Sinh(c) => write!(out, "Sinh(n{c})"),
        Node::Cosh(c) => write!(out, "Cosh(n{c})"),
        Node::Tanh(c) => write!(out, "Tanh(n{c})"),
        Node::Sech(c) => write!(out, "Sech(n{c})"),
        Node::Csch(c) => write!(out, "Csch(n{c})"),
        Node::Coth(c) => write!(out, "Coth(n{c})"),

        Node::Asinh(c) => write!(out, "Asinh(n{c})"),
        Node::Acosh(c) => write!(out, "Acosh(n{c})"),
        Node::Atanh(c) => write!(out, "Atanh(n{c})"),
        Node::Asech(c) => write!(out, "Asech(n{c})"),
        Node::Acsch(c) => write!(out, "Acsch(n{c})"),
        Node::Acoth(c) => write!(out, "Acoth(n{c})"),
    }
}

#[rustfmt::skip]
#[inline(always)]
fn write_unary_recursive<W: Write>(out: &mut W, name: &str, a: u32, arena: &[Node], src: &str) -> std::io::Result<()> {
    write!(out, "{name}(")?;
    write_node_recursive(out, a, arena, src)?;
    write!(out, ")")
}

#[rustfmt::skip]
fn write_log_base_recursive<W: Write>(out: &mut W, base: u32, arg: u32, arena: &[Node], src: &str) -> std::io::Result<()> {
    write!(out, "log_")?;
    if is_log_base_atomic(&arena[base as usize]) {
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

fn is_log_base_atomic(node: &Node) -> bool {
    matches!(
        node,
        Node::Number(_) | Node::Constant(_) | Node::Variable(_, _, _)
    )
}
