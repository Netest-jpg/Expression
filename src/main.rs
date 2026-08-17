use std::io::IsTerminal;
use std::io::{BufRead, BufWriter, Write};

use expression::eval::{
    EvaluationError, EvaluationResult, eval, evaluate_pending, try_simple_assign,
};
use expression::lexer::{Token, Tokenizer};
use expression::parser::{Node, Parser};
use expression::simplify::simplify;
use expression::variables::{VariableStore, collect_variables};

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
        Node::Variable(s, e, _) => write!(out, "{}", &src[*s as usize..*e as usize]),
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

/// Recursively renders the subtree rooted at `idx` into actual expression
/// text. Unlike `write_node_compact`, which formats a single node and
/// prints child indices as literal `n{idx}` text (fine for the `show ast`
/// arena dump, where each line is meant to reference others by index),
/// this walks the whole subtree so callers get a real, human-readable
/// expression rather than leaked internal arena indices.
fn write_node_recursive<W: Write>(
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
            write_node_recursive(out, a, arena, src)?;
            write!(out, " + ")?;
            write_node_recursive(out, b, arena, src)
        }
        Node::Sub(a, b) => {
            let (a, b) = (*a, *b);
            write_node_recursive(out, a, arena, src)?;
            write!(out, " - ")?;
            write_node_recursive(out, b, arena, src)
        }
        Node::Mul(a, b) => {
            let (a, b) = (*a, *b);
            write_node_recursive(out, a, arena, src)?;
            write!(out, " * ")?;
            write_node_recursive(out, b, arena, src)
        }
        Node::Div(a, b) => {
            let (a, b) = (*a, *b);
            write_node_recursive(out, a, arena, src)?;
            write!(out, " / ")?;
            write_node_recursive(out, b, arena, src)
        }
        Node::Pow(a, b) => {
            let (a, b) = (*a, *b);
            write_node_recursive(out, a, arena, src)?;
            write!(out, " ^ ")?;
            write_node_recursive(out, b, arena, src)
        }

        Node::Sin(a) => write_unary_recursive(out, "sin", *a, arena, src),
        Node::Cos(a) => write_unary_recursive(out, "cos", *a, arena, src),
        Node::Tan(a) => write_unary_recursive(out, "tan", *a, arena, src),

        Node::Ln(a) => write_unary_recursive(out, "ln", *a, arena, src),
        Node::Log(a) => write_unary_recursive(out, "log", *a, arena, src),
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
            write_node_verbose(out, &node, src)?;
        } else {
            write_node_compact(out, &node, src)?;
        }
    }
    Ok(())
}

struct DebugFlags {
    tokens: bool,
    ast: bool,
}

impl DebugFlags {
    fn new(verbose: bool) -> Self {
        DebugFlags {
            tokens: true, // on by default
            ast: verbose, // off by default unless --debug
        }
    }
}

struct Pending {
    src: String,
    arena: Vec<Node>,
    root: u32,
}

impl Pending {
    /// Return the names of variables in this equation that are still unbound.
    fn free_var_names<'a>(&'a self, vars: &VariableStore) -> Vec<&'a str> {
        collect_variables(&self.arena, self.root)
            .iter()
            .filter(|(h, _, _)| vars.get(*h).is_none())
            .map(|(_, s, e)| &self.src[*s as usize..*e as usize])
            .collect()
    }

    /// Print the current state of this pending equation relative to vars.
    fn print_status<W: Write>(&self, out: &mut W, vars: &VariableStore) {
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

    let mut flags = DebugFlags::new(verbose);

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
              [ use 'simplify' to simplify the pending equation ]\n\
              [ use 'clear' to reset all variable bindings ]\n\
              [ use 'show tokens' / 'show ast' to toggle debug output ]\n\
              \n",
        )
        .ok();
    }

    let mut line = String::with_capacity(64);
    let mut tokens: Vec<Token> = Vec::with_capacity(32);
    let mut arena: Vec<Node> = Vec::with_capacity(32);
    let mut vars = VariableStore::new();
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
                            match result {
                                EvaluationResult::Value(v) => {
                                    write!(out, "  = ").ok();
                                    write_value(&mut out, v).ok();
                                    writeln!(out).ok();
                                }
                                EvaluationResult::Verified { lhs, rhs } => {
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
                                EvaluationResult::Solved {
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

        if expression == "simplify" {
            match &pending {
                None => {
                    eprintln!("Error: nothing to simplify — enter an expression first.");
                }
                Some(p) => {
                    // simplify() appends new nodes onto a clone of the pending
                    // arena rather than the original — keeps Pending's stored
                    // arena untouched in case the user wants to keep building
                    // on the unsimplified version (e.g. assigning a variable
                    // then evaluating normally afterwards).
                    let mut work_arena = p.arena.clone();
                    let simplified_root = simplify(&mut work_arena, p.root);

                    if should_print {
                        write!(out, "  = ").ok();
                        write_node_recursive(&mut out, simplified_root, &work_arena, &p.src).ok();
                        writeln!(out).ok();

                        if flags.ast {
                            write_arena(&mut out, simplified_root, &work_arena, &p.src, verbose)
                                .ok();
                            writeln!(out).ok();
                        }
                    }
                }
            }
            if is_terminal {
                writeln!(out).ok();
                out.flush().ok();
            }
            continue;
        }

        if expression == "show tokens" {
            flags.tokens = !flags.tokens;
            if should_print {
                writeln!(
                    out,
                    "  (token output {})",
                    if flags.tokens { "on" } else { "off" }
                )
                .ok();
            }
            if is_terminal {
                writeln!(out).ok();
                out.flush().ok();
            }
            continue;
        }

        if expression == "show ast" {
            flags.ast = !flags.ast;
            if should_print {
                writeln!(
                    out,
                    "  (AST output {})",
                    if flags.ast { "on" } else { "off" }
                )
                .ok();
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

        if should_print && flags.tokens {
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

        if should_print && flags.ast {
            write_arena(&mut out, root, &arena, expression, verbose).ok();
            writeln!(out).ok();
        }

        // ---- dispatch on root kind -----------------------------------------
        match &arena[root as usize] {
            // -----------------------------------------------------------------
            // Equation node: try simple assignment first (x = <value>);
            // if the lhs is complex or rhs has free vars, store as pending.
            // -----------------------------------------------------------------
            Node::Equation(_, _) => {
                match try_simple_assign(&arena, root, &mut vars) {
                    Err(e) => {
                        eprintln!("Error: {e}");
                    }

                    Ok(Some((start, end, value))) => {
                        // x = <number>: stored in VariableStore.
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
                                collect_variables(pending.as_ref().unwrap().arena.as_slice(), root);
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
                            EvaluationError::UnboundVariable => {
                                if should_print {
                                    if let Some(prev) = &pending {
                                        writeln!(out, "  (replacing pending: \"{}\")", prev.src)
                                            .ok();
                                    }
                                    // Only collect names for display.
                                    let all_vars = collect_variables(&arena, root);
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
