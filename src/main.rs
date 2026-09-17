use std::io::IsTerminal;
use std::io::{BufRead, BufWriter, Write};

use expression::lexer::{Token, Tokenizer};
use expression::parser::{Node, Parser};
use expression::simplification::simplify;
use expression::variables::{VariableBank, collect_variables};
#[rustfmt::skip]
use expression::evaluation::{EvaluationMessages, EvaluationResult, evaluate, evaluate_pending, try_simple_assign};
mod format;
use format::{write_arena, write_node_recursive, write_tokens, write_value};

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

struct DebugFlags {
    tokens: bool,
    ast: bool,
    verbose: bool,
}

impl DebugFlags {
    fn new() -> Self {
        DebugFlags {
            tokens: true,   // on by default
            ast: false,     // off by default, toggle with 'show ast'
            verbose: false, // off by default, toggle with 'verbose'
        }
    }
}

struct Pending {
    src: String,
    arena: Vec<Node>,
    root: u32,
}

impl Pending {
    // TODO: improve the docstring and write a doctest
    /// Return the names of variables in this equation that are still unbound.
    fn free_var_names<'a>(&'a self, vars: &VariableBank) -> Vec<&'a str> {
        collect_variables(&self.arena, self.root)
            .iter()
            .filter(|(h, _, _)| vars.get(*h).is_none())
            .map(|(_, s, e)| &self.src[*s as usize..*e as usize])
            .collect()
    }

    // TODO: improve the docstring and write a doctest
    /// Print the current state of this pending equation relative to vars.
    #[rustfmt::skip]
    fn print_status<W: Write>(&self, out: &mut W, vars: &VariableBank) {
        let free = self.free_var_names(vars);
        if free.is_empty() {
            writeln!(out, "  pending \"{}\" — all variables bound, type 'evaluate'", self.src).ok();
        } else {
            writeln!(out, "  pending \"{}\" — still need: {}", self.src, free.join(", ")).ok();
        }
    }
}

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let mut flags = DebugFlags::new();

    let stdin = std::io::stdin();
    let mut input = std::io::BufReader::with_capacity(1 << 16, stdin.lock());

    let stdout = std::io::stdout();
    let mut out = BufWriter::with_capacity(1 << 16, stdout.lock());

    let is_terminal = std::io::stdin().is_terminal();

    if is_terminal {
        out.write_all(
            b"Enter a math expression:\n\
              [ use ':q' to exit ]\n\
              [ use 'evaluate' to solve the pending equation ]\n\
              [ use 'simplify' to simplify the pending equation ]\n\
              [ use 'clear' to reset all variable bindings ]\n\
              [ use 'show tokens' / 'show ast' / 'verbose' to toggle debug output ]\n\
              \n",
        )
        .ok();
    }

    let mut line = String::with_capacity(64);
    let mut tokens: Vec<Token> = Vec::with_capacity(32);
    let mut arena: Vec<Node> = Vec::with_capacity(32);
    let mut vars = VariableBank::new();
    let mut pending: Option<Pending> = None;
    // Hoisted buffer: reused for Pending.src each iteration instead of calling expression.to_string() which allocates a fresh String every time.
    let mut pending_src = String::new();

    loop {
        if is_terminal {
            write!(out, "~ ").ok();
            out.flush().ok();
        }

        line.clear();
        match input.read_line(&mut line) {
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

        // CONTROL WORDS
        if expression.is_empty() {
            continue;
        }

        if matches!(expression, ":q") {
            if is_terminal {
                writeln!(out, "process terminated").ok();
            }
            break;
        }

        if expression == "clear" {
            vars.clear();
            pending = None;
            if is_terminal {
                writeln!(out, "  (all variables and pending equation cleared)").ok();
                writeln!(out).ok();
                out.flush().ok();
            }
            continue;
        }

        if expression == "evaluate" {
            match &pending {
                None => {
                    eprintln!("Error: Nothing to evaluate — enter an equation first.");
                }
                Some(p) => match evaluate_pending(&p.arena, p.root, &vars, &p.src) {
                    Err(e) => eprintln!("Error: {e}"),
                    Ok(result) => match result {
                        EvaluationResult::Value(v) => {
                            write!(out, "  ").ok();
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
                        #[rustfmt::skip]
                        EvaluationResult::Solved { name, values, count } => {
                            if count == 2 && (values[0] + values[1]).abs() < 1e-9 {
                                write!(out, "  {} = ±", name).ok();
                                write_value(&mut out, values[0].abs()).ok();
                                writeln!(out).ok();
                            } else {
                                    for value in values.iter().take(count as usize) {
                                    write!(out, "  {} = ", name).ok();
                                    write_value(&mut out, *value).ok();
                                    writeln!(out).ok();
                                }
                            }
                        },
                    },
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
                    eprintln!("Error: Nothing to simplify — enter an expression first.");
                }
                #[rustfmt::skip]
                Some(p) => {
                    let mut work_arena = p.arena.clone();
                    let simplified_root = simplify(&mut work_arena, p.root);

                    write!(out, "  ").ok();
                    write_node_recursive(&mut out, simplified_root, &work_arena, &p.src).ok();
                    writeln!(out).ok();
                    // TODO: re-enable once write_arena (or a compaction pass) can print
                    // the simplified arena without showing orphaned/duplicate nodes.
                    // if flags.ast {
                    //     write_arena(&mut out, simplified_root, &work_arena, &p.src, flags.verbose).ok();
                    //     writeln!(out).ok();
                    // }
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
            if is_terminal {
                writeln!(
                    out,
                    "  (Token output {})",
                    if flags.tokens { "on" } else { "off" }
                )
                .ok();
                writeln!(out).ok();
                out.flush().ok();
            }
            continue;
        }

        if expression == "show ast" {
            flags.ast = !flags.ast;
            if is_terminal {
                writeln!(
                    out,
                    "  (AST output {})",
                    if flags.ast { "on" } else { "off" }
                )
                .ok();
                writeln!(out).ok();
                out.flush().ok();
            }
            continue;
        }

        if expression == "verbose" {
            flags.verbose = !flags.verbose;
            if is_terminal {
                writeln!(
                    out,
                    "  (Verbose mode {})",
                    if flags.verbose { "on" } else { "off" }
                )
                .ok();
                writeln!(out).ok();
                out.flush().ok();
            }
            continue;
        }

        if let Err(e) = Tokenizer::new(expression).tokenize(&mut tokens) {
            eprintln!("Error: {e}");
            writeln!(out).ok();
            out.flush().ok();
            continue;
        }

        if is_terminal && flags.tokens {
            write_tokens(&mut out, &tokens, expression, flags.verbose).ok();
            writeln!(out).ok();
            writeln!(out).ok();
        }

        arena.clear();
        arena.reserve(tokens.len());

        let root = match Parser::new(&tokens, expression, &mut arena).parse() {
            Err(e) => {
                eprintln!("Error: {e}");
                writeln!(out).ok();
                out.flush().ok();
                continue;
            }
            Ok(root) => root,
        };

        if is_terminal && flags.ast {
            write_arena(&mut out, root, &arena, expression, flags.verbose).ok();
            writeln!(out).ok();
        }

        match &arena[root as usize] {
            Node::Equation(_, _) => {
                match try_simple_assign(&arena, root, &mut vars) {
                    Err(e) => {
                        eprintln!("Error: {e}");
                    }
                    Ok(Some((start, end, value))) => {
                        let name = &expression[start as usize..end as usize];
                        write!(out, "  {} = ", name).ok();
                        write_value(&mut out, value).ok();
                        writeln!(out).ok();
                        if is_terminal && let Some(p) = &pending {
                            p.print_status(&mut out, &vars);
                        }
                    }
                    Ok(None) => {
                        if is_terminal && let Some(prev) = &pending {
                            writeln!(out, "  (replacing pending: \"{}\")", prev.src).ok();
                        }
                        let mut pending_arena = match pending.take() {
                            Some(old) => {
                                pending_src = old.src;
                                old.arena
                            }
                            None => Vec::with_capacity(32),
                        };
                        std::mem::swap(&mut pending_arena, &mut arena);
                        pending_src.clear();
                        pending_src.push_str(expression);
                        // Safety: pending_src is not accessed while pending is live because pending_src is only mutated here, before the new Pending is constructed, and reclaimed from Pending above.
                        let src = std::mem::take(&mut pending_src);
                        pending = Some(Pending {
                            src,
                            arena: pending_arena,
                            root,
                        });

                        if is_terminal {
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
                match evaluate(&arena, root, &vars) {
                    Ok(v) => {
                        write!(out, "  = ").ok();
                        write_value(&mut out, v).ok();
                        writeln!(out).ok();
                    }
                    Err(e) => {
                        match e {
                            EvaluationMessages::UnboundVariable => {
                                if is_terminal {
                                    if let Some(prev) = &pending {
                                        writeln!(out, "  (replacing pending: \"{}\")", prev.src)
                                            .ok();
                                    }
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
                                    None => Vec::with_capacity(32),
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
