use std::io::IsTerminal;
use std::io::{BufRead, BufWriter, Write};

use expression::lexer::{Token, Tokenizer};
use expression::parser::{Node, Parser};
use expression::simplification::simplify;
use expression::variables::{VariableBank, collect_variables};
#[rustfmt::skip]
use expression::evaluation::{EvaluationError, EvaluationResult, evaluate, evaluate_pending, try_simple_assign};
mod format;
use format::{write_arena, write_node_recursive, write_tokens, write_value};

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

// TODO: write a docstring
struct DebugFlags {
    tokens: bool,
    ast: bool,
}

impl DebugFlags {
    // TODO: write a docstring
    fn new(verbose: bool) -> Self {
        DebugFlags {
            tokens: true, // on by default
            ast: verbose, // off by default unless --debug
        }
    }
}

// TODO: write a docstring
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
    // TODO: sort things out, group similar things together. TL;DR: make things less messy
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
    let mut vars = VariableBank::new();
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
                    eprintln!("Error: Nothing to evaluate — enter an equation first.");
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
                                #[rustfmt::skip]
                                EvaluationResult::Solved { name, values, count } => {
                                    if count == 2 && (values[0] + values[1]).abs() < 1e-9 {
                                        // Symmetric roots: show ±
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
        // TODO: shorten the following comment
        // Reserve before parsing: node count can never exceed token count, so this single reserve eliminates all the repeated grow-reallocations that dhat PP 1.1.1 flagged (6 allocs across recursive parse_expr calls). On subsequent REPL iterations the Vec already has sufficient capacity and reserve() becomes a no-op — no wasted work.
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
            Ok(root) => root,
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
                        // x = <number>: stored in VariableBank.
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
                            #[rustfmt::skip]
                            let all_vars = collect_variables(pending.as_ref().unwrap().arena.as_slice(), root);
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
                        if should_print {
                            write!(out, "  = ").ok();
                            write_value(&mut out, v).ok();
                            writeln!(out).ok();
                        }
                    }
                    // evaluate returns Err — match on the type to distinguish
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
