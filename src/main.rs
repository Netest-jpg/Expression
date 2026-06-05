mod lexer;
mod parser;

use std::io::IsTerminal;
use std::io::{BufRead, BufWriter, Write};

use zmij::Buffer as DtoaBuffer;

use lexer::{Token, Tokenizer};
use parser::{Node, NodeKind, Parser};

// #[global_allocator]
// static ALLOC: dhat::Alloc = dhat::Alloc;

/// Write the token list.
/// Compact: single horizontal line, no EndOfFile.
/// Verbose (--debug): one token per line with kind + resolved text.
fn write_tokens<W: Write>(
    out: &mut W,
    tokens: &[Token],
    src: &str,
    verbose: bool,
) -> std::io::Result<()> {
    write!(out, "Tokens:")?;
    for tok in tokens {
        match tok {
            Token::EndOfFile => {}
            Token::Number { start, end, .. } => {
                if verbose {
                    write!(out, "\n  Number({})", &src[*start as usize..*end as usize])?;
                } else {
                    write!(out, " {}", &src[*start as usize..*end as usize])?;
                }
            }
            Token::Identifier { start, end, .. } => {
                if verbose {
                    write!(
                        out,
                        "\n  Identifier({})",
                        &src[*start as usize..*end as usize]
                    )?;
                } else {
                    write!(out, " {}", &src[*start as usize..*end as usize])?;
                }
            }
            other => {
                let s = match other {
                    Token::Plus => "+",
                    Token::Minus => "-",
                    Token::Asterisk => "*",
                    Token::ForwardSlash => "/",
                    Token::Caret => "^",
                    Token::LeftParenthesis => "(",
                    Token::RightParenthesis => ")",
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

/// Write a single node kind in compact form.
fn write_node_compact<W: Write>(out: &mut W, kind: &NodeKind, src: &str) -> std::io::Result<()> {
    match kind {
        NodeKind::Number(v) => {
            if v.fract() == 0.0 && v.abs() < 1e15 {
                write!(out, "{}", *v as i64)
            } else {
                let mut buf = DtoaBuffer::new();
                write!(out, "{}", buf.format(*v))
            }
        }
        NodeKind::Constant(v) => {
            if (v - std::f64::consts::PI).abs() < 1e-14 {
                write!(out, "π")
            } else if (v - std::f64::consts::E).abs() < 1e-14 {
                write!(out, "e")
            } else {
                let mut buf = DtoaBuffer::new();
                write!(out, "{}", buf.format(*v))
            }
        }
        NodeKind::Variable(start, end) => write!(out, "{}", &src[*start as usize..*end as usize]),
        NodeKind::Neg(a) => write!(out, "-(n{a})"),
        NodeKind::Add(a, b) => write!(out, "n{a} + n{b}"),
        NodeKind::Sub(a, b) => write!(out, "n{a} - n{b}"),
        NodeKind::Mul(a, b) => write!(out, "n{a} * n{b}"),
        NodeKind::Div(a, b) => write!(out, "n{a} / n{b}"),
        NodeKind::Pow(a, b) => write!(out, "n{a} ^ n{b}"),
        NodeKind::Sin(a) => write!(out, "sin(n{a})"),
        NodeKind::Cos(a) => write!(out, "cos(n{a})"),
        NodeKind::Tan(a) => write!(out, "tan(n{a})"),
        NodeKind::Ln(a) => write!(out, "ln(n{a})"),
        NodeKind::Log(a) => write!(out, "log(n{a})"),
        NodeKind::Sqrt(a) => write!(out, "sqrt(n{a})"),
        NodeKind::Call { hash, arg } => write!(out, "call<{hash}>(n{arg})"),
    }
}

/// Write a single node kind in verbose form.
fn write_node_verbose<W: Write>(out: &mut W, kind: &NodeKind, src: &str) -> std::io::Result<()> {
    match kind {
        NodeKind::Number(v) => write!(out, "Number({v})"),
        NodeKind::Constant(v) => write!(out, "Constant({v})"),
        NodeKind::Variable(start, end) => {
            write!(out, "Variable({:?})", &src[*start as usize..*end as usize])
        }
        NodeKind::Neg(c) => write!(out, "Neg(n{c})"),
        NodeKind::Add(l, r) => write!(out, "Add(n{l}, n{r})"),
        NodeKind::Sub(l, r) => write!(out, "Sub(n{l}, n{r})"),
        NodeKind::Mul(l, r) => write!(out, "Mul(n{l}, n{r})"),
        NodeKind::Div(l, r) => write!(out, "Div(n{l}, n{r})"),
        NodeKind::Pow(l, r) => write!(out, "Pow(n{l}, n{r})"),
        NodeKind::Sin(c) => write!(out, "Sin(n{c})"),
        NodeKind::Cos(c) => write!(out, "Cos(n{c})"),
        NodeKind::Tan(c) => write!(out, "Tan(n{c})"),
        NodeKind::Ln(c) => write!(out, "Ln(n{c})"),
        NodeKind::Log(c) => write!(out, "Log(n{c})"),
        NodeKind::Sqrt(c) => write!(out, "Sqrt(n{c})"),
        NodeKind::Call { hash, arg } => write!(out, "Call(hash: {hash}, arg: n{arg})"),
    }
}

/// Write the full arena.
fn write_arena<W: Write>(
    out: &mut W,
    root: u32,
    arena: &[Node],
    src: &str,
    verbose: bool,
) -> std::io::Result<()> {
    write!(out, "AST root: n{root}\nAST arena:")?;
    for (i, node) in arena.iter().enumerate() {
        write!(out, "\n  n{i}: ")?;
        if verbose {
            write_node_verbose(out, &node.kind, src)?;
        } else {
            write_node_compact(out, &node.kind, src)?;
        }
    }
    Ok(())
}

fn main() {
    //let _profiler = dhat::Profiler::new_heap();
    let verbose = std::env::args().any(|a| a == "--debug");

    let stdout = std::io::stdout();
    let mut out = BufWriter::with_capacity(1 << 16, stdout.lock());

    let stdin = std::io::stdin();
    let mut stdin = std::io::BufReader::with_capacity(1 << 16, stdin.lock()); // lock ONCE

    let is_terminal = std::io::stdin().is_terminal();

    // When piped (not a terminal) and not in debug mode, skip all formatting
    // work — tokenize + parse only. This makes pipe-based benchmarks measure
    // actual parse throughput instead of write!/format overhead.
    let should_print = is_terminal || verbose;

    if verbose {
        writeln!(out, "Debug mode ON").ok();
    }
    if should_print {
        writeln!(out, "Enter a math expression:").ok();
        writeln!(out, "[ use 'quit' / 'exit' / ':q' to exit the program ]").ok();
        writeln!(out).ok();
    }

    let mut line = String::with_capacity(64);
    // tokens and arena live outside the loop: Vec capacity reused every iteration.
    let mut tokens: Vec<Token> = Vec::new();
    let mut arena: Vec<Node> = Vec::new();

    loop {
        if is_terminal {
            write!(out, "~ ").ok();
            out.flush().ok();
        }

        line.clear();

        match stdin.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: {e}");
                break;
            }
        }

        if line.capacity() > 512 {
            line.shrink_to(128);
        }

        let expression = line.trim();

        if matches!(expression, "quit" | "exit" | ":q") {
            if should_print {
                writeln!(out, "bye bye").ok();
            }
            break;
        }

        if expression.is_empty() {
            continue;
        }

        // Tokenize — tokens borrows nothing from `line` (offset-based).
        if let Err(e) = Tokenizer::new(expression).tokenize(&mut tokens) {
            eprintln!("Error: {e}");
            if should_print {
                writeln!(out).ok();
                out.flush().ok();
            }
            continue;
        }

        if should_print {
            write_tokens(&mut out, &tokens, expression, verbose).ok();
            writeln!(out).ok();
            writeln!(out).ok();
        }

        // Parse — arena is borrowed mutably; capacity is preserved on both
        // Ok and Err, no mem::take shuffling needed.
        arena.clear();
        match Parser::new(&tokens, expression, &mut arena).parse() {
            Err(e) => {
                eprintln!("Error: {e}");
                if should_print {
                    writeln!(out).ok();
                }
            }
            Ok(root) => {
                if should_print {
                    write_arena(&mut out, root, &arena, expression, verbose).ok();
                    writeln!(out).ok();
                }
            }
        }

        if is_terminal {
            writeln!(out).ok();
            out.flush().ok();
        }
    }
}