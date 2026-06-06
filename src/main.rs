mod lexer;
mod parser;

use std::io::IsTerminal;
use std::io::{BufRead, BufWriter, Write};

use zmij::Buffer as DtoaBuffer;

use lexer::{Token, Tokenizer};
use parser::{
    Node, NodeKind, Parser, VarStore,
    collect_vars, eval, evaluate_pending, try_simple_assign,
    EvalResultKind,
};

// -----------------------------------------------------------------------
// Token display
// -----------------------------------------------------------------------
fn write_tokens<W: Write>(
    out:     &mut W,
    tokens:  &[Token],
    src:     &str,
    verbose: bool,
) -> std::io::Result<()> {
    write!(out, "Tokens:")?;
    for tok in tokens {
        match tok {
            Token::EndOfFile => {}
            Token::Number { start, end, .. } => {
                let s = &src[*start as usize..*end as usize];
                if verbose { write!(out, "\n  Number({s})")?; } else { write!(out, " {s}")?; }
            }
            Token::Identifier { start, end, .. } => {
                let s = &src[*start as usize..*end as usize];
                if verbose { write!(out, "\n  Identifier({s})")?; } else { write!(out, " {s}")?; }
            }
            other => {
                let s = match other {
                    Token::Plus             => "+",
                    Token::Minus            => "-",
                    Token::Asterisk         => "*",
                    Token::ForwardSlash     => "/",
                    Token::Caret            => "^",
                    Token::LeftParenthesis  => "(",
                    Token::RightParenthesis => ")",
                    Token::Equals           => "=",
                    _ => unreachable!(),
                };
                if verbose { write!(out, "\n  {s}")?; } else { write!(out, " {s}")?; }
            }
        }
    }
    Ok(())
}

// -----------------------------------------------------------------------
// AST display
// -----------------------------------------------------------------------
fn fmt_value(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let mut buf = DtoaBuffer::new();
        buf.format(v).to_string()
    }
}

fn write_node_compact<W: Write>(out: &mut W, kind: &NodeKind, src: &str) -> std::io::Result<()> {
    match kind {
        NodeKind::Number(v)           => write!(out, "{}", fmt_value(*v)),
        NodeKind::Constant(v)         => {
            if (v - std::f64::consts::PI).abs() < 1e-14 { write!(out, "π") }
            else if (v - std::f64::consts::E).abs() < 1e-14 { write!(out, "e") }
            else { write!(out, "{}", fmt_value(*v)) }
        }
        NodeKind::Variable(s, e, _)   => write!(out, "{}", &src[*s as usize..*e as usize]),
        NodeKind::Neg(a)              => write!(out, "-(n{a})"),
        NodeKind::Equation(a, b)      => write!(out, "n{a} = n{b}"),
        NodeKind::Add(a, b)           => write!(out, "n{a} + n{b}"),
        NodeKind::Sub(a, b)           => write!(out, "n{a} - n{b}"),
        NodeKind::Mul(a, b)           => write!(out, "n{a} * n{b}"),
        NodeKind::Div(a, b)           => write!(out, "n{a} / n{b}"),
        NodeKind::Pow(a, b)           => write!(out, "n{a} ^ n{b}"),
        NodeKind::Sin(a)              => write!(out, "sin(n{a})"),
        NodeKind::Cos(a)              => write!(out, "cos(n{a})"),
        NodeKind::Tan(a)              => write!(out, "tan(n{a})"),
        NodeKind::Ln(a)               => write!(out, "ln(n{a})"),
        NodeKind::Log(a)              => write!(out, "log(n{a})"),
        NodeKind::Sqrt(a)             => write!(out, "sqrt(n{a})"),
        NodeKind::Call { hash, arg }  => write!(out, "call<{hash}>(n{arg})"),
    }
}

fn write_node_verbose<W: Write>(out: &mut W, kind: &NodeKind, src: &str) -> std::io::Result<()> {
    match kind {
        NodeKind::Number(v)           => write!(out, "Number({v})"),
        NodeKind::Constant(v)         => write!(out, "Constant({v})"),
        NodeKind::Variable(s, e, _)   => write!(out, "Variable({:?})", &src[*s as usize..*e as usize]),
        NodeKind::Neg(c)              => write!(out, "Neg(n{c})"),
        NodeKind::Equation(l, r)      => write!(out, "Equation(n{l}, n{r})"),
        NodeKind::Add(l, r)           => write!(out, "Add(n{l}, n{r})"),
        NodeKind::Sub(l, r)           => write!(out, "Sub(n{l}, n{r})"),
        NodeKind::Mul(l, r)           => write!(out, "Mul(n{l}, n{r})"),
        NodeKind::Div(l, r)           => write!(out, "Div(n{l}, n{r})"),
        NodeKind::Pow(l, r)           => write!(out, "Pow(n{l}, n{r})"),
        NodeKind::Sin(c)              => write!(out, "Sin(n{c})"),
        NodeKind::Cos(c)              => write!(out, "Cos(n{c})"),
        NodeKind::Tan(c)              => write!(out, "Tan(n{c})"),
        NodeKind::Ln(c)               => write!(out, "Ln(n{c})"),
        NodeKind::Log(c)              => write!(out, "Log(n{c})"),
        NodeKind::Sqrt(c)             => write!(out, "Sqrt(n{c})"),
        NodeKind::Call { hash, arg }  => write!(out, "Call(hash: {hash}, arg: n{arg})"),
    }
}

fn write_arena<W: Write>(
    out:     &mut W,
    root:    u32,
    arena:   &[Node],
    src:     &str,
    verbose: bool,
) -> std::io::Result<()> {
    write!(out, "AST root: n{root}\nAST arena:")?;
    for (i, node) in arena.iter().enumerate() {
        write!(out, "\n  n{i}: ")?;
        if verbose { write_node_verbose(out, &node.kind, src)?; }
        else       { write_node_compact(out, &node.kind, src)?; }
    }
    Ok(())
}

// -----------------------------------------------------------------------
// Pending expression state
// -----------------------------------------------------------------------
struct Pending {
    /// Stored source string (owns its allocation).
    src:  String,
    /// Arena for the pending expression (shared with main arena, swapped in).
    arena: Vec<Node>,
    root: u32,
}

// -----------------------------------------------------------------------
// main
// -----------------------------------------------------------------------
fn main() {
    let verbose = std::env::args().any(|a| a == "--debug");

    let stdout = std::io::stdout();
    let mut out = BufWriter::with_capacity(1 << 16, stdout.lock());

    let stdin = std::io::stdin();
    let mut stdin = std::io::BufReader::with_capacity(1 << 16, stdin.lock());

    let is_terminal = std::io::stdin().is_terminal();
    let should_print = is_terminal || verbose;

    if verbose { writeln!(out, "Debug mode ON").ok(); }
    if should_print {
        writeln!(out, "Enter a math expression:").ok();
        writeln!(out, "[ use 'quit' / 'exit' / ':q' to exit ]").ok();
        writeln!(out, "[ type 'evaluate' to solve the last equation ]").ok();
        writeln!(out, "[ type 'clear' to reset all variable bindings ]").ok();
        writeln!(out).ok();
    }

    let mut line       = String::with_capacity(64);
    let mut tokens:    Vec<Token> = Vec::new();
    let mut arena:     Vec<Node>  = Vec::new();
    let mut vars       = VarStore::new();
    let mut pending:   Option<Pending> = None;

    loop {
        if is_terminal {
            write!(out, "~ ").ok();
            out.flush().ok();
        }

        line.clear();
        match stdin.read_line(&mut line) {
            Ok(0)  => break,
            Ok(_)  => {}
            Err(e) => { eprintln!("Error: {e}"); break; }
        }
        if line.capacity() > 512 { line.shrink_to(128); }

        let expression = line.trim();

        // ---- control words -------------------------------------------------
        if matches!(expression, "quit" | "exit" | ":q") {
            if should_print { writeln!(out, "bye bye").ok(); }
            break;
        }

        if expression.is_empty() { continue; }

        if expression == "clear" {
            vars.clear();
            pending = None;
            if should_print { writeln!(out, "  (all variables cleared)").ok(); }
            if is_terminal { writeln!(out).ok(); out.flush().ok(); }
            continue;
        }

        // ---- 'evaluate' command --------------------------------------------
        if expression == "evaluate" {
            match &pending {
                None => {
                    eprintln!("Error: nothing to evaluate — enter an expression first.");
                }
                Some(p) => {
                    // Check how many free vars remain
                    let all_vars  = collect_vars(&p.arena, p.root);
                    let free_vars: Vec<_> = all_vars.iter()
                        .filter(|(h, _, _)| vars.get(*h).is_none())
                        .collect();

                    if free_vars.len() > 1 {
                        // Build name list from the pending source
                        let names: Vec<&str> = free_vars.iter()
                            .map(|(_, s, e)| &p.src[*s as usize..*e as usize])
                            .collect();
                        eprintln!(
                            "Error: cannot evaluate — {} variable{} still unassigned: {}.\n\
                             Assign with  name=value  or leave exactly one free to solve for.",
                            names.len(),
                            if names.len() == 1 { "" } else { "s" },
                            names.join(", ")
                        );
                    } else {
                        match evaluate_pending(&p.arena, p.root, &vars, &p.src) {
                            Err(e) => eprintln!("Error: {e}"),
                            Ok(result) => {
                                if should_print {
                                    match result.kind {
                                        EvalResultKind::Value(v) => {
                                            writeln!(out, "  = {}", fmt_value(v)).ok();
                                        }
                                        EvalResultKind::Verified { lhs, rhs } => {
                                            let tol = 1e-9;
                                            if (lhs - rhs).abs() < tol {
                                                writeln!(out, "  ✓  {} = {}  (true)", fmt_value(lhs), fmt_value(rhs)).ok();
                                            } else {
                                                writeln!(out, "  ✗  {} ≠ {}  (false)", fmt_value(lhs), fmt_value(rhs)).ok();
                                            }
                                        }
                                        EvalResultKind::Solved { name, value } => {
                                            writeln!(out, "  {} = {}", name, fmt_value(value)).ok();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if is_terminal { writeln!(out).ok(); out.flush().ok(); }
            continue;
        }

        // ---- tokenize ------------------------------------------------------
        if let Err(e) = Tokenizer::new(expression).tokenize(&mut tokens) {
            eprintln!("Error: {e}");
            if is_terminal { writeln!(out).ok(); out.flush().ok(); }
            continue;
        }

        if should_print {
            write_tokens(&mut out, &tokens, expression, verbose).ok();
            writeln!(out).ok();
            writeln!(out).ok();
        }

        // ---- parse ---------------------------------------------------------
        arena.clear();
        let root = match Parser::new(&tokens, expression, &mut arena).parse() {
            Err(e) => {
                eprintln!("Error: {e}");
                if is_terminal { writeln!(out).ok(); out.flush().ok(); }
                continue;
            }
            Ok(r) => r,
        };

        if should_print {
            write_arena(&mut out, root, &arena, expression, verbose).ok();
            writeln!(out).ok();
        }

        // ---- dispatch on root kind -----------------------------------------
        match &arena[root as usize].kind {

            // Equation node: try simple assignment first (x = <value>),
            // otherwise store as pending.
            NodeKind::Equation(_, _) => {
                match try_simple_assign(&arena, root, &mut vars, expression) {
                    Err(e) => {
                        eprintln!("Error: {e}");
                    }
                    Ok(Some((name, value))) => {
                        // Simple assignment succeeded.
                        if should_print {
                            writeln!(out, "  {} = {}", name, fmt_value(value)).ok();
                        }
                    }
                    Ok(None) => {
                        // Complex equation — store as pending, show free vars.
                        let all_vars = collect_vars(&arena, root);
                        let free: Vec<&str> = all_vars
                            .iter()
                            .filter(|(h, _, _)| vars.get(*h).is_none())
                            .map(|(_, s, e)| &expression[*s as usize..*e as usize])
                            .collect();

                        if should_print {
                            if free.is_empty() {
                                writeln!(out, "  (all variables bound — type 'evaluate' to check)").ok();
                            } else {
                                writeln!(
                                    out,
                                    "  (stored — free variable{}: {})",
                                    if free.len() == 1 { "" } else { "s" },
                                    free.join(", ")
                                ).ok();
                                writeln!(out, "  assign value{} then type 'evaluate'",
                                    if free.len() == 1 { "" } else { "s" }
                                ).ok();
                            }
                        }

                        // Move arena + src into pending, give main a fresh arena.
                        let mut pending_arena = Vec::new();
                        std::mem::swap(&mut pending_arena, &mut arena);
                        pending = Some(Pending {
                            src:   expression.to_string(),
                            arena: pending_arena,
                            root,
                        });
                        // arena is now empty and ready for the next iteration.
                    }
                }
            }

            // Plain expression — evaluate immediately if all vars bound,
            // otherwise store as pending.
            _ => {
                let all_vars = collect_vars(&arena, root);
                let free: Vec<&str> = all_vars
                    .iter()
                    .filter(|(h, _, _)| vars.get(*h).is_none())
                    .map(|(_, s, e)| &expression[*s as usize..*e as usize])
                    .collect();

                if free.is_empty() {
                    // No free variables — evaluate immediately.
                    match eval(&arena, root, &vars) {
                        Ok(v) => {
                            if should_print {
                                writeln!(out, "  = {}", fmt_value(v)).ok();
                            }
                        }
                        Err(e) => eprintln!("Error: {e}"),
                    }
                } else {
                    // Free variables present — store as pending.
                    if should_print {
                        writeln!(
                            out,
                            "  (stored — free variable{}: {})",
                            if free.len() == 1 { "" } else { "s" },
                            free.join(", ")
                        ).ok();
                        writeln!(out, "  assign value{} then type 'evaluate'",
                            if free.len() == 1 { "" } else { "s" }
                        ).ok();
                    }

                    let mut pending_arena = Vec::new();
                    std::mem::swap(&mut pending_arena, &mut arena);
                    pending = Some(Pending {
                        src:   expression.to_string(),
                        arena: pending_arena,
                        root,
                    });
                }
            }
        }

        if is_terminal { writeln!(out).ok(); out.flush().ok(); }
    }
}