mod lexer;
mod parser;

use lexer::Tokenizer;
use parser::{NodeKind, Parser};

fn format_node_kind(kind: &NodeKind<'_>) -> String {
    match kind {
        NodeKind::Number(value) => format!("Number({value})"),
        NodeKind::Constant(value) => format!("Constant({value})"),
        NodeKind::Variable(name) => format!("Variable({name:?})"),
        NodeKind::Neg(child) => format!("Neg(n{child})"),
        NodeKind::Add(left, right) => format!("Add(n{left}, n{right})"),
        NodeKind::Sub(left, right) => format!("Sub(n{left}, n{right})"),
        NodeKind::Mul(left, right) => format!("Mul(n{left}, n{right})"),
        NodeKind::Div(left, right) => format!("Div(n{left}, n{right})"),
        NodeKind::Pow(left, right) => format!("Pow(n{left}, n{right})"),
        NodeKind::Sin(child) => format!("Sin(n{child})"),
        NodeKind::Cos(child) => format!("Cos(n{child})"),
        NodeKind::Tan(child) => format!("Tan(n{child})"),
        NodeKind::Ln(child) => format!("Ln(n{child})"),
        NodeKind::Log(child) => format!("Log(n{child})"),
        NodeKind::Sqrt(child) => format!("Sqrt(n{child})"),
        NodeKind::Call { hash, arg } => format!("Call(hash: {hash}, arg: n{arg})"),
    }
}

fn main() {
    println!("Enter a math expression:");

    let mut expression = String::new();

    std::io::stdin().read_line(&mut expression).unwrap();

    let expression = expression.trim();

    let mut tokenizer = Tokenizer::new(expression);
    let tokens = tokenizer.tokenize();
    let (root, arena) = Parser::new(&tokens).parse();

    println!("Tokens:");
    for token in &tokens {
        println!("  {:?}", token);
    }

    println!("AST root: n{root}");
    println!("AST arena:");
    for (index, node) in arena.iter().enumerate() {
        println!("  n{index}: {}", format_node_kind(&node.kind));
    }
}
