mod lexer;
mod parser;

use lexer::{Token, Tokenizer};
use parser::{Node, NodeKind, Parser};

// -----------------------------------------------------------------------
// Display helpers
// -----------------------------------------------------------------------

/// Compact single-character display for operator tokens.
fn token_display(tok: &Token<'_>) -> Option<&'static str> {
    Some(match tok {
        Token::Plus             => "+",
        Token::Minus            => "-",
        Token::Asterisk         => "*",
        Token::ForwardSlash     => "/",
        Token::Caret            => "^",
        Token::LeftParenthesis  => "(",
        Token::RightParenthesis => ")",
        Token::EndOfFile        => return None,
        _                       => return None, // Number/Identifier handled below
    })
}

/// Build the compact display string for one token (None = skip).
fn token_compact(tok: &Token<'_>) -> Option<String> {
    match tok {
        Token::Number { raw, .. }     => Some(raw.to_string()),
        Token::Identifier { name, .. } => Some(name.to_string()),
        Token::EndOfFile              => None,
        other => token_display(other).map(|s| s.to_string()),
    }
}

/// Format the token list as a String.
/// Compact: single horizontal line, no EndOfFile.
/// Verbose (--debug): one token per line with Debug repr.
fn format_tokens(tokens: &[Token<'_>], verbose: bool) -> String {
    if verbose {
        let mut out = String::from("Tokens:");
        for tok in tokens {
            if !matches!(tok, Token::EndOfFile) {
                out.push_str(&format!("\n  {:?}", tok));
            }
        }
        out
    } else {
        let parts: Vec<String> = tokens.iter().filter_map(token_compact).collect();
        format!("Tokens: {}", parts.join(" "))
    }
}

/// Compact display for one arena node kind.
fn node_compact(kind: &NodeKind<'_>) -> String {
    match kind {
        NodeKind::Number(v) => {
            if v.fract() == 0.0 && v.abs() < 1e15 {
                format!("{}", *v as i64)
            } else {
                format!("{v}")
            }
        }
        NodeKind::Constant(v) => {
            if (v - std::f64::consts::PI).abs() < 1e-14 {
                "π".to_string()
            } else if (v - std::f64::consts::E).abs() < 1e-14 {
                "e".to_string()
            } else {
                format!("{v}")
            }
        }
        NodeKind::Variable(name)     => name.to_string(),
        NodeKind::Neg(a)             => format!("-(n{a})"),
        NodeKind::Add(a, b)          => format!("n{a} + n{b}"),
        NodeKind::Sub(a, b)          => format!("n{a} - n{b}"),
        NodeKind::Mul(a, b)          => format!("n{a} * n{b}"),
        NodeKind::Div(a, b)          => format!("n{a} / n{b}"),
        NodeKind::Pow(a, b)          => format!("n{a} ^ n{b}"),
        NodeKind::Sin(a)             => format!("sin(n{a})"),
        NodeKind::Cos(a)             => format!("cos(n{a})"),
        NodeKind::Tan(a)             => format!("tan(n{a})"),
        NodeKind::Ln(a)              => format!("ln(n{a})"),
        NodeKind::Log(a)             => format!("log(n{a})"),
        NodeKind::Sqrt(a)            => format!("sqrt(n{a})"),
        NodeKind::Call { hash, arg } => format!("call<{hash}>(n{arg})"),
    }
}

/// Verbose display for one arena node (original style).
fn node_verbose(kind: &NodeKind<'_>) -> String {
    match kind {
        NodeKind::Number(value)      => format!("Number({value})"),
        NodeKind::Constant(value)    => format!("Constant({value})"),
        NodeKind::Variable(name)     => format!("Variable({name:?})"),
        NodeKind::Neg(child)         => format!("Neg(n{child})"),
        NodeKind::Add(left, right)   => format!("Add(n{left}, n{right})"),
        NodeKind::Sub(left, right)   => format!("Sub(n{left}, n{right})"),
        NodeKind::Mul(left, right)   => format!("Mul(n{left}, n{right})"),
        NodeKind::Div(left, right)   => format!("Div(n{left}, n{right})"),
        NodeKind::Pow(left, right)   => format!("Pow(n{left}, n{right})"),
        NodeKind::Sin(child)         => format!("Sin(n{child})"),
        NodeKind::Cos(child)         => format!("Cos(n{child})"),
        NodeKind::Tan(child)         => format!("Tan(n{child})"),
        NodeKind::Ln(child)          => format!("Ln(n{child})"),
        NodeKind::Log(child)         => format!("Log(n{child})"),
        NodeKind::Sqrt(child)        => format!("Sqrt(n{child})"),
        NodeKind::Call { hash, arg } => format!("Call(hash: {hash}, arg: n{arg})"),
    }
}

/// Format the arena as a String.
fn format_arena(root: u32, arena: &[Node<'_>], verbose: bool) -> String {
    let mut out = format!("AST root: n{root}\nAST arena:");
    for (i, node) in arena.iter().enumerate() {
        if verbose {
            out.push_str(&format!("\n  n{i}: {}", node_verbose(&node.kind)));
        } else {
            out.push_str(&format!("\n  n{i}: {}", node_compact(&node.kind)));
        }
    }
    out
}

// -----------------------------------------------------------------------
// REPL
// -----------------------------------------------------------------------

fn main() {
    let verbose = std::env::args().any(|a| a == "--debug");

    if verbose {
        println!("Debug mode ON");
    }
    println!("Enter a math expression:");
    println!("[ use 'quit' / 'exit' / ':q' to exit the program ]");
    println!();

    loop {
        // Prompt
        print!("- ");
        {
            use std::io::Write;
            std::io::stdout().flush().ok();
        }

        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) => break, // EOF (e.g. piped input finished)
            Ok(_) => {}
            Err(e) => {
                eprintln!("Read error: {e}");
                break;
            }
        }

        let expression = line.trim();

        // Exit commands
        if matches!(expression, "quit" | "exit" | ":q") {
            println!("bye bye");
            break;
        }

        // Skip blank lines
        if expression.is_empty() {
            continue;
        }

        // Tokenize + parse — all formatting happens *inside* the closure so
        // that arena's lifetime (which borrows from tokens) stays fully
        // contained; we return only owned Strings across the closure boundary.
        let result = std::panic::catch_unwind(|| {
            let tokens = Tokenizer::new(expression).tokenize();
            let token_line = format_tokens(&tokens, verbose);

            let (root, arena) = Parser::new(&tokens).parse();
            let arena_text = format_arena(root, &arena, verbose);

            (token_line, arena_text)
        });

        match result {
            Err(e) => {
                let msg = if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "unknown error".to_string()
                };
                eprintln!("Error: {msg}");
            }
            Ok((token_line, arena_text)) => {
                println!("{token_line}");
                println!();
                println!("{arena_text}");
            }
        }

        println!();
    }
}