use std::io::IsTerminal;
use std::io::{BufRead, BufWriter, Write};

use expression::eval::{EvalError, EvalResultKind, eval, evaluate_pending, try_simple_assign};
use expression::lexer::{Token, Tokenizer};
use expression::parser::{Node, NodeKind, Parser};
use expression::vars::{VarStore, collect_vars};
use zmij::Buffer as DtoaBuffer;
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

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
            Token::Number { start, end } => {
                let s = &src[*start as usize..*end as usize];
                if verbose {
                    write!(out, "\n  Number({s})")?;
                } else {
                    write!(out, " {s}")?;
                }
            }
            Token::Identifier { start, end, .. } => {
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
                    Token::Asterisk => "*",
                    Token::ForwardSlash => "/",
                    Token::Caret => "^",
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
fn write_value<W: Write>(out: &mut W, v: f64) -> std::io::Result<()> {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        let mut buf = itoa::Buffer::new();
        out.write_all(buf.format(v as i64).as_bytes())
    } else {
        let mut buf = DtoaBuffer::new();
        out.write_all(buf.format(v).as_bytes())
    }
}

fn write_node_compact<W: Write>(out: &mut W, kind: &NodeKind, src: &str) -> std::io::Result<()> {
    match kind {
        NodeKind::Number(v) => write_value(out, *v),
        NodeKind::Constant(v) => {
            if (v - std::f64::consts::PI).abs() < 1e-14 {
                write!(out, "π")
            } else if (v - std::f64::consts::E).abs() < 1e-14 {
                write!(out, "e")
            } else {
                write_value(out, *v)
            }
        }
        NodeKind::Variable(s, e, _) => write!(out, "{}", &src[*s as usize..*e as usize]),
        NodeKind::Neg(a) => write!(out, "-(n{a})"),
        NodeKind::Equation(a, b) => write!(out, "n{a} = n{b}"),

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

        NodeKind::Sec(a) => write!(out, "sec(n{a})"),
        NodeKind::Csc(a) => write!(out, "csc(n{a})"),
        NodeKind::Cot(a) => write!(out, "cot(n{a})"),

        NodeKind::Asin(a) => write!(out, "asin(n{a})"),
        NodeKind::Acos(a) => write!(out, "acos(n{a})"),
        NodeKind::Atan(a) => write!(out, "atan(n{a})"),
        NodeKind::Acsc(a) => write!(out, "acsc(n{a})"),
        NodeKind::Asec(a) => write!(out, "asec(n{a})"),
        NodeKind::Acot(a) => write!(out, "acot(n{a})"),

        NodeKind::Sinh(a) => write!(out, "sinh(n{a})"),
        NodeKind::Cosh(a) => write!(out, "cosh(n{a})"),
        NodeKind::Tanh(a) => write!(out, "tanh(n{a})"),
        NodeKind::Sech(a) => write!(out, "sech(n{a})"),
        NodeKind::Csch(a) => write!(out, "csch(n{a})"),
        NodeKind::Coth(a) => write!(out, "coth(n{a})"),

        NodeKind::Asinh(a) => write!(out, "asinh(n{a})"),
        NodeKind::Acosh(a) => write!(out, "acosh(n{a})"),
        NodeKind::Atanh(a) => write!(out, "atanh(n{a})"),
        NodeKind::Asech(a) => write!(out, "asech(n{a})"),
        NodeKind::Acsch(a) => write!(out, "acsch(n{a})"),
        NodeKind::Acoth(a) => write!(out, "acoth(n{a})"),

        NodeKind::Call { hash, arg } => write!(out, "call<{hash}>(n{arg})"),
    }
}

fn write_node_verbose<W: Write>(out: &mut W, kind: &NodeKind, src: &str) -> std::io::Result<()> {
    match kind {
        NodeKind::Number(v) => {
            write!(out, "Number(")?;
            write_value(out, *v)?;
            write!(out, ")")
        }
        NodeKind::Constant(v) => {
            write!(out, "Constant(")?;
            write_value(out, *v)?;
            write!(out, ")")
        }
        NodeKind::Variable(s, e, _) => {
            write!(out, "Variable({:?})", &src[*s as usize..*e as usize])
        }
        NodeKind::Neg(c) => write!(out, "Neg(n{c})"),
        NodeKind::Equation(l, r) => write!(out, "Equation(n{l}, n{r})"),

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

        NodeKind::Sec(c) => write!(out, "Sec(n{c})"),
        NodeKind::Csc(c) => write!(out, "Csc(n{c})"),
        NodeKind::Cot(c) => write!(out, "Cot(n{c})"),

        NodeKind::Asin(c) => write!(out, "Asin(n{c})"),
        NodeKind::Acos(c) => write!(out, "Acos(n{c})"),
        NodeKind::Atan(c) => write!(out, "Atan(n{c})"),
        NodeKind::Acsc(c) => write!(out, "Acsc(n{c})"),
        NodeKind::Asec(c) => write!(out, "Asec(n{c})"),
        NodeKind::Acot(c) => write!(out, "Acot(n{c})"),

        NodeKind::Sinh(c) => write!(out, "Sinh(n{c})"),
        NodeKind::Cosh(c) => write!(out, "Cosh(n{c})"),
        NodeKind::Tanh(c) => write!(out, "Tanh(n{c})"),
        NodeKind::Sech(c) => write!(out, "Sech(n{c})"),
        NodeKind::Csch(c) => write!(out, "Csch(n{c})"),
        NodeKind::Coth(c) => write!(out, "Coth(n{c})"),

        NodeKind::Asinh(c) => write!(out, "Asinh(n{c})"),
        NodeKind::Acosh(c) => write!(out, "Acosh(n{c})"),
        NodeKind::Atanh(c) => write!(out, "Atanh(n{c})"),
        NodeKind::Asech(c) => write!(out, "Asech(n{c})"),
        NodeKind::Acsch(c) => write!(out, "Acsch(n{c})"),
        NodeKind::Acoth(c) => write!(out, "Acoth(n{c})"),

        NodeKind::Call { hash, arg } => write!(out, "Call(hash: {hash}, arg: n{arg})"),
    }
}

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

struct Pending {
    src: String,
    arena: Vec<Node>,
    root: u32,
}

impl Pending {
    /// Return the names of variables in this equation that are still unbound.
    fn free_var_names<'a>(&'a self, vars: &VarStore) -> Vec<&'a str> {
        collect_vars(&self.arena, self.root)
            .iter()
            .filter(|(h, _, _)| vars.get(*h).is_none())
            .map(|(_, s, e)| &self.src[*s as usize..*e as usize])
            .collect()
    }

    /// Print the current state of this pending equation relative to vars.
    fn print_status<W: Write>(&self, out: &mut W, vars: &VarStore) {
        let free = self.free_var_names(vars);
        if free.is_empty() {
            writeln!(
                out,
                "  pending \"{}\" — all variables bound, type 'evaluate'",
                self.src
            )
            .ok();
        } else {
            writeln!(
                out,
                "  pending \"{}\" — still need: {}",
                self.src,
                free.join(", ")
            )
            .ok();
        }
    }
}

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let verbose = std::env::args().any(|a| a == "--debug");

    let stdout = std::io::stdout();
    let mut out = BufWriter::with_capacity(1 << 16, stdout.lock());

    let stdin = std::io::stdin();
    let mut stdin = std::io::BufReader::with_capacity(1 << 16, stdin.lock());

    let is_terminal = std::io::stdin().is_terminal();
    let should_print = is_terminal || verbose;

    if verbose {
        out.write_all(b"Debug mode ON\n").ok();
    }
    if should_print {
        out.write_all(
            b"Enter a math expression:\n\
              [ use 'quit' / 'exit' / ':q' to exit ]\n\
              [ use 'evaluate' to solve the pending equation ]\n\
              [ use 'clear' to reset all variable bindings ]\n\
              \n",
        )
        .ok();
    }

    let mut line = String::with_capacity(64);
    let mut tokens: Vec<Token> = Vec::new();
    let mut arena: Vec<Node> = Vec::new();
    let mut vars = VarStore::new();
    let mut pending: Option<Pending> = None;
    // Hoisted buffer: reused for Pending.src each iteration instead of
    // calling expression.to_string() which allocates a fresh String every time.
    let mut pending_src = String::new();

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

        // ---- control words -------------------------------------------------
        if matches!(expression, "quit" | "exit" | ":q") {
            if should_print {
                writeln!(out, "program terminated").ok();
            }
            break;
        }

        if expression.is_empty() {
            continue;
        }

        if expression == "clear" {
            vars.clear();
            pending = None;
            if should_print {
                writeln!(out, "  (all variables and pending equation cleared)").ok();
            }
            if is_terminal {
                writeln!(out).ok();
                out.flush().ok();
            }
            continue;
        }

        if expression == "evaluate" {
            match &pending {
                None => {
                    eprintln!("Error: nothing to evaluate — enter an equation first.");
                }
                Some(p) => match evaluate_pending(&p.arena, p.root, &vars, &p.src) {
                    Err(e) => eprintln!("Error: {e}"),
                    Ok(result) => {
                        if should_print {
                            match result.kind {
                                EvalResultKind::Value(v) => {
                                    write!(out, "  = ").ok();
                                    write_value(&mut out, v).ok();
                                    writeln!(out).ok();
                                }
                                EvalResultKind::Verified { lhs, rhs } => {
                                    if (lhs - rhs).abs() < 1e-9 {
                                        write!(out, "  ✓  ").ok();
                                        write_value(&mut out, lhs).ok();
                                        write!(out, " = ").ok();
                                        write_value(&mut out, rhs).ok();
                                        writeln!(out, "  (true)").ok();
                                    } else {
                                        write!(out, "  ✗  ").ok();
                                        write_value(&mut out, lhs).ok();
                                        write!(out, " ≠ ").ok();
                                        write_value(&mut out, rhs).ok();
                                        writeln!(out, "  (false)").ok();
                                    }
                                }
                                EvalResultKind::Solved {
                                    name,
                                    values,
                                    count,
                                } => {
                                    if count == 2 && (values[0] + values[1]).abs() < 1e-9 {
                                        // Symmetric roots: show ±
                                        write!(out, "  {} = ±", name).ok();
                                        write_value(&mut out, values[0].abs()).ok();
                                        writeln!(out).ok();
                                    } else {
                                        for i in 0..count as usize {
                                            write!(out, "  {} = ", name).ok();
                                            write_value(&mut out, values[i]).ok();
                                            writeln!(out).ok();
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
            }
            if is_terminal {
                writeln!(out).ok();
                out.flush().ok();
            }
            continue;
        }

        if let Err(e) = Tokenizer::new(expression).tokenize(&mut tokens) {
            eprintln!("Error: {e}");
            if is_terminal {
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

        arena.clear();
        // Reserve before parsing: node count can never exceed token count, so
        // this single reserve eliminates all the repeated grow-reallocations
        // that dhat PP 1.1.1 flagged (6 allocs across recursive parse_expr calls).
        // On subsequent REPL iterations the Vec already has sufficient capacity
        // and reserve() becomes a no-op — no wasted work.
        arena.reserve(tokens.len());
        let root = match Parser::new(&tokens, expression, &mut arena).parse() {
            Err(e) => {
                eprintln!("Error: {e}");
                if is_terminal {
                    writeln!(out).ok();
                    out.flush().ok();
                }
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
            // -----------------------------------------------------------------
            // Equation node: try simple assignment first (x = <value>);
            // if the lhs is complex or rhs has free vars, store as pending.
            // -----------------------------------------------------------------
            NodeKind::Equation(_, _) => {
                match try_simple_assign(&arena, root, &mut vars) {
                    Err(e) => {
                        eprintln!("Error: {e}");
                    }

                    Ok(Some((start, end, value))) => {
                        // x = <number>: stored in VarStore.
                        // Then show how this affects the pending equation.
                        if should_print {
                            let name = &expression[start as usize..end as usize];
                            write!(out, "  {} = ", name).ok();
                            write_value(&mut out, value).ok();
                            writeln!(out).ok();
                            if let Some(p) = &pending {
                                p.print_status(&mut out, &vars);
                            }
                        }
                    }

                    Ok(None) => {
                        // Complex equation (e.g. x^2+2x = 3).
                        // Warn if we are replacing an existing pending equation,
                        // then store and report which variables are still free.
                        if should_print && let Some(prev) = &pending {
                            writeln!(out, "  (replacing pending: \"{}\")", prev.src).ok();
                        }

                        // Reclaim the old pending_src buffer (if any) so its heap
                        // allocation is reused by push_str below — no fresh alloc.
                        let mut pending_arena = match pending.take() {
                            Some(old) => {
                                pending_src = old.src; // reclaim String buffer
                                old.arena
                            }
                            None => Vec::new(),
                        };
                        std::mem::swap(&mut pending_arena, &mut arena);
                        pending_src.clear();
                        pending_src.push_str(expression); // reuse heap, no alloc if cap sufficient
                        // Safety: pending_src is not accessed while pending is live
                        // because pending_src is only mutated here, before the new
                        // Pending is constructed, and reclaimed from Pending above.
                        let src = std::mem::take(&mut pending_src);
                        pending = Some(Pending {
                            src,
                            arena: pending_arena,
                            root,
                        });

                        if should_print {
                            // Only collect vars for display; skip the allocation in non-interactive mode.
                            let all_vars =
                                collect_vars(pending.as_ref().unwrap().arena.as_slice(), root);
                            let free: Vec<&str> = all_vars
                                .iter()
                                .filter(|(h, _, _)| vars.get(*h).is_none())
                                .map(|(_, s, e)| &expression[*s as usize..*e as usize])
                                .collect();
                            if free.is_empty() {
                                writeln!(out, "  (all variables bound — type 'evaluate' to check)")
                                    .ok();
                            } else {
                                writeln!(
                                    out,
                                    "  (stored — free variable{}: {})",
                                    if free.len() == 1 { "" } else { "s" },
                                    free.join(", ")
                                )
                                .ok();
                                writeln!(
                                    out,
                                    "  assign value{} then type 'evaluate'",
                                    if free.len() == 1 { "" } else { "s" }
                                )
                                .ok();
                            }
                        }
                    }
                }
            }

            _ => {
                match eval(&arena, root, &vars) {
                    Ok(v) => {
                        if should_print {
                            write!(out, "  = ").ok();
                            write_value(&mut out, v).ok();
                            writeln!(out).ok();
                        }
                    }
                    // eval returns Err — match on the type to distinguish
                    // unbound variables (store as pending) from real errors.
                    Err(e) => {
                        match e {
                            EvalError::UnboundVariable => {
                                if should_print {
                                    if let Some(prev) = &pending {
                                        writeln!(out, "  (replacing pending: \"{}\")", prev.src)
                                            .ok();
                                    }
                                    // Only collect names for display.
                                    let all_vars = collect_vars(&arena, root);
                                    let free: Vec<&str> = all_vars
                                        .iter()
                                        .filter(|(h, _, _)| vars.get(*h).is_none())
                                        .map(|(_, s, e)| &expression[*s as usize..*e as usize])
                                        .collect();
                                    writeln!(
                                        out,
                                        "  (stored — free variable{}: {})",
                                        if free.len() == 1 { "" } else { "s" },
                                        free.join(", ")
                                    )
                                    .ok();
                                    writeln!(
                                        out,
                                        "  assign value{} then type 'evaluate'",
                                        if free.len() == 1 { "" } else { "s" }
                                    )
                                    .ok();
                                }

                                let mut pending_arena = match pending.take() {
                                    Some(old) => {
                                        pending_src = old.src; // reclaim String buffer
                                        old.arena
                                    }
                                    None => Vec::new(),
                                };
                                std::mem::swap(&mut pending_arena, &mut arena);
                                pending_src.clear();
                                pending_src.push_str(expression); // reuse heap, no alloc if cap sufficient
                                let src = std::mem::take(&mut pending_src);
                                pending = Some(Pending {
                                    src,
                                    arena: pending_arena,
                                    root,
                                });
                            }
                            other => {
                                eprintln!("Error: {}", other.to_string_msg());
                            }
                        }
                    }
                }
            }
        }

        if is_terminal {
            writeln!(out).ok();
            out.flush().ok();
        }
    }
}
