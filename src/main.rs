use ::expression::angles::AngleMode;
use expression::evaluation::{EvaluationError, evaluate, evaluate_or_solve, try_assign};
use expression::lexer::{Token, Tokenizer};
use expression::parser::{Node, Parser};
use expression::simplification::simplify;
use expression::variables::VariableValues;
use std::io::{BufRead, BufWriter, IsTerminal, Write};
mod format;
use format::{write_node_recursive, write_result, write_tokens, write_value};
mod settings;
use settings::Settings;
mod pending;
use pending::{Pending, store_pending};

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
    let mut vars = VariableValues::new();
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
        }
        out.flush().ok();
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
                continue;
            }
            "evaluate" => {
                match &pending {
                    None => {
                        eprintln!("Error: Nothing to evaluate. Enter an equation.");
                        writeln!(out).ok();
                    }
                    Some(p) => match evaluate_or_solve(&p.arena, p.root, &vars, &p.src) {
                        Ok(result) => {
                            write_result(&mut out, result).ok();
                            writeln!(out).ok();
                        }
                        Err(e) => {
                            eprintln!("Error: {e}");
                            writeln!(out).ok();
                        }
                    },
                }
                continue;
            }
            "simplify" => {
                match &pending {
                    None => {
                        eprintln!("Error: Nothing to simplify. Enter an expression.");
                        writeln!(out).ok();
                    }
                    Some(p) => {
                        let mut work_arena = p.arena.clone();
                        let simplified_root = simplify(&mut work_arena, p.root);
                        write_node_recursive(&mut out, simplified_root, &work_arena, &p.src).ok();
                        writeln!(out).ok();
                        writeln!(out).ok();
                    }
                }
                continue;
            }
            _ => {}
        }

        if let Err(e) = Tokenizer::new(expression).tokenize(&mut tokens) {
            eprintln!("Error: {e}");
            writeln!(out).ok();
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
            .set_angle(settings.angle)
            .parse()
        {
            Err(e) => {
                eprintln!("Error: {e}");
                writeln!(out).ok();
                continue;
            }
            Ok(root) => root,
        };

        match &arena[root as usize] {
            Node::Equation(_, _) => match try_assign(&arena, root, &mut vars) {
                Err(e) => {
                    eprintln!("Error: {e}");
                }
                Ok(Some((start, end, value))) => {
                    let name = &expression[start as usize..end as usize];
                    write!(out, "{} = ", name).ok();
                    write_value(&mut out, value).ok();
                    writeln!(out).ok();
                    if is_terminal && let Some(p) = &pending {
                        p.print_pending(&mut out, &vars);
                    }
                }
                Ok(None) => {
                    if is_terminal && let Some(p) = &pending {
                        writeln!(out, "(Replacing pending: '{}')", p.src).ok();
                    }
                    pending = Some(store_pending(
                        pending.take(),
                        &mut pending_src,
                        &mut arena,
                        expression,
                        root,
                    ));

                    if is_terminal && let Some(p) = &pending {
                        let free = p.unbound_variables(&vars);

                        if free.is_empty() {
                            writeln!(out, "(All variables bound. Type 'evaluate' to check)").ok();
                        } else {
                            let plural = if free.len() == 1 { "" } else { "s" };

                            writeln!(out, "(Stored variable{plural}: {})", free.join(", ")).ok();
                            writeln!(out, "Assign value{plural} then type 'evaluate'").ok();
                        }
                    }
                }
            },
            _ => match evaluate(&arena, root, &vars) {
                Ok(v) => {
                    write_value(&mut out, v).ok();
                    writeln!(out).ok();
                }
                Err(e) => match e {
                    EvaluationError::UnboundVariable => {
                        if is_terminal && let Some(p) = &pending {
                            writeln!(out, "(Replacing pending: '{}')", p.src).ok();
                        }
                        pending = Some(store_pending(
                            pending.take(),
                            &mut pending_src,
                            &mut arena,
                            expression,
                            root,
                        ));
                        if is_terminal && let Some(p) = &pending {
                            let free = p.unbound_variables(&vars);
                            let plural = if free.len() == 1 { "" } else { "s" };

                            writeln!(out, "(Replacing pending: '{}')", p.src).ok();
                            writeln!(out, "(Stored variable{plural}: {})", free.join(", ")).ok();
                            writeln!(out, "Assign value{plural} then type 'evaluate'").ok();
                        }
                    }
                    other => {
                        eprintln!("Error: {}", other);
                    }
                },
            },
        }
        if is_terminal {
            writeln!(out).ok();
        }
    }
}
