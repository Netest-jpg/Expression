use ::expression::angles::AngleMode;
use expression::evaluation::{
    EvaluationError, EvaluationResult, evaluate, evaluate_or_solve, try_simple_assign,
};
use expression::lexer::{Token, Tokenizer};
use expression::parser::{Node, Parser};
use expression::simplification::simplify;
use expression::variables::{VariableBank, collect_variables};
use std::io::{BufRead, BufWriter, IsTerminal, Write};
mod format;
use format::{write_node_recursive, write_tokens, write_value};
mod settings;
use settings::Settings;
mod pending;
use pending::Pending;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let mut settings = Settings::new();
    let stdin = std::io::stdin();
    let mut input = std::io::BufReader::with_capacity(1 << 16, stdin.lock());
    let stdout = std::io::stdout();
    let mut out = BufWriter::with_capacity(1 << 16, stdout.lock());
    let is_terminal = std::io::stdin().is_terminal();
    let mut line = String::with_capacity(64);
    let mut tokens: Vec<Token> = Vec::with_capacity(32);
    let mut arena: Vec<Node> = Vec::with_capacity(32);
    let mut vars = VariableBank::new();
    let mut pending: Option<Pending> = None;
    let mut pending_src = String::new();

    if is_terminal {
        out.write_all(
            b"Enter a math expression:\n\
              [ use 'quit' or ':q' to exit ]\n\
              [ use 'evaluate' to solve the pending equation ]\n\
              [ use 'simplify' to simplify the pending equation ]\n\
              [ use 'clear' to reset all variable bindings ]\n\
              [ use 'tokens' to toggle token output ]\n\
              [ use 'angle' to toggle default angle unit (degrees/radians) ]\n\
              \n",
        )
        .ok();
    }

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

        if expression.is_empty() {
            continue;
        }
        match expression {
            "quit" | ":q" => {
                writeln!(out, "process terminated").ok();
                break;
            }
            "clear" => {
                vars.clear();
                pending = None;
                writeln!(out, "(All variables and pending equation cleared)\n").ok();
                out.flush().ok();
                continue;
            }
            "angle" => {
                settings.angle = match settings.angle {
                    AngleMode::Degrees => AngleMode::Radians,
                    AngleMode::Radians => AngleMode::Degrees,
                };
                writeln!(
                    out,
                    "(Changed default angle unit to: {:?})\n",
                    settings.angle
                )
                .ok();
                out.flush().ok();

                continue;
            }
            "tokens" => {
                settings.tokens.toggle();
                writeln!(
                    out,
                    "(Token output: {})\n",
                    ["off", "on"][settings.tokens as usize]
                )
                .ok();
                out.flush().ok();
                continue;
            }
            "evaluate" => {
                match &pending {
                    None => {
                        eprintln!("Error: Nothing to evaluate. Enter an equation.");
                    }
                    Some(p) => match evaluate_or_solve(&p.arena, p.root, &vars, &p.src) {
                        Err(e) => eprintln!("Error: {e}"),
                        Ok(result) => match result {
                            EvaluationResult::Value(v) => {
                                write_value(&mut out, v).ok();
                                writeln!(out).ok();
                            }
                            EvaluationResult::Verified { lhs, rhs } => {
                                if (lhs - rhs).abs() < 1e-9 {
                                    write_value(&mut out, lhs).ok();
                                    write!(out, " = ").ok();
                                    write_value(&mut out, rhs).ok();
                                    writeln!(out, "  (LHS = RHS)").ok();
                                } else {
                                    write_value(&mut out, lhs).ok();
                                    write!(out, " ≠ ").ok();
                                    write_value(&mut out, rhs).ok();
                                    writeln!(out, "  (LHS ≠ RHS)").ok();
                                }
                            }
                            #[rustfmt::skip]
                            EvaluationResult::Solved { name, values, count } => {
                                if count == 2 && (values[0] + values[1]).abs() < 1e-9 {
                                    write!(out, "{} = ±", name).ok();
                                    write_value(&mut out, values[0].abs()).ok();
                                    writeln!(out).ok();
                                } else {
                                        for value in values.iter().take(count as usize) {
                                        write!(out, "{} = ", name).ok();
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
            "simplify" => {
                match &pending {
                    None => {
                        eprintln!("Error: Nothing to simplify — enter an expression first.");
                    }
                    Some(p) => {
                        let mut work_arena = p.arena.clone();
                        let simplified_root = simplify(&mut work_arena, p.root);
                        write_node_recursive(&mut out, simplified_root, &work_arena, &p.src).ok();
                        writeln!(out).ok();
                    }
                }
                if is_terminal {
                    writeln!(out).ok();
                    out.flush().ok();
                }
                continue;
            }
            _ => {}
        }

        if let Err(e) = Tokenizer::new(expression).tokenize(&mut tokens) {
            eprintln!("Error: {e}");
            writeln!(out).ok();
            out.flush().ok();
            continue;
        }

        if is_terminal && settings.tokens {
            write_tokens(&mut out, &tokens, expression).ok();
            writeln!(out).ok();
            writeln!(out).ok();
        }

        arena.clear();
        arena.reserve(tokens.len());

        let root = match Parser::new(&tokens, expression, &mut arena)
            .with_angle(settings.angle)
            .parse()
        {
            Err(e) => {
                eprintln!("Error: {e}");
                writeln!(out).ok();
                out.flush().ok();
                continue;
            }
            Ok(root) => root,
        };

        match &arena[root as usize] {
            Node::Equation(_, _) => {
                match try_simple_assign(&arena, root, &mut vars) {
                    Err(e) => {
                        eprintln!("Error: {e}");
                    }
                    Ok(Some((start, end, value))) => {
                        let name = &expression[start as usize..end as usize];
                        write!(out, "{} = ", name).ok();
                        write_value(&mut out, value).ok();
                        writeln!(out).ok();
                        if is_terminal && let Some(p) = &pending {
                            p.print_status(&mut out, &vars);
                        }
                    }
                    Ok(None) => {
                        if is_terminal && let Some(prev) = &pending {
                            writeln!(out, "(replacing pending: \"{}\")", prev.src).ok();
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
                                writeln!(out, "(all variables bound — type 'evaluate' to check)")
                                    .ok();
                            } else {
                                writeln!(
                                    out,
                                    "(stored — free variable{}: {})",
                                    if free.len() == 1 { "" } else { "s" },
                                    free.join(", ")
                                )
                                .ok();
                                writeln!(
                                    out,
                                    "assign value{} then type 'evaluate'",
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
                        write_value(&mut out, v).ok();
                        writeln!(out).ok();
                    }
                    Err(e) => {
                        match e {
                            EvaluationError::UnboundVariable => {
                                if is_terminal {
                                    if let Some(prev) = &pending {
                                        writeln!(out, "(replacing pending: \"{}\")", prev.src).ok();
                                    }
                                    let all_vars = collect_variables(&arena, root);
                                    let free: Vec<&str> = all_vars
                                        .iter()
                                        .filter(|(h, _, _)| vars.get(*h).is_none())
                                        .map(|(_, s, e)| &expression[*s as usize..*e as usize])
                                        .collect();
                                    writeln!(
                                        out,
                                        "(stored — free variable{}: {})",
                                        if free.len() == 1 { "" } else { "s" },
                                        free.join(", ")
                                    )
                                    .ok();
                                    writeln!(
                                        out,
                                        "assign value{} then type 'evaluate'",
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
                                eprintln!("Error: {}", other);
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
