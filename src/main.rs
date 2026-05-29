mod lexer;
use lexer::lexer::Tokenizer;

fn main() {
    println!("Enter a math expression:");

    let mut expression = String::new();

    std::io::stdin()
        .read_line(&mut expression)
        .unwrap();

    let expression = expression.trim();

    println!("Tokens:");
    let mut tokenizer = Tokenizer::new(expression);
    let tokens = tokenizer.tokenize();

    for token in &tokens {
        println!("  {:?}", token);
    }
}