use criterion::{Criterion, black_box, criterion_group, criterion_main};
use expression::lexer::Tokenizer;
use expression::parser::Parser;

fn bench_full_pipeline(c: &mut Criterion) {
    let src = "sin(x)^2 + cos(x)^2 + sqrt(9) * ln(e)";

    c.bench_function("lex+parse+eval", |b| {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        b.iter(|| {
            tokens.clear();
            arena.clear();
            Tokenizer::new(black_box(src))
                .tokenize(&mut tokens)
                .unwrap();
            let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
            black_box(root);
        });
    });
}
fn bench_lexer(c: &mut Criterion) {
    let src = "sin(x)^2 + cos(x)^2 + sqrt(9) * ln(e)";

    c.bench_function("lex only", |b| {
        let mut tokens = Vec::new();
        b.iter(|| {
            tokens.clear();
            Tokenizer::new(black_box(src))
                .tokenize(&mut tokens)
                .unwrap();
            black_box(&tokens);
        });
    });
}
criterion_group!(benches, bench_full_pipeline, bench_lexer);
criterion_main!(benches);
