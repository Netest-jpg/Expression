use criterion::{Criterion, black_box, criterion_group, criterion_main};
use expression::eval::{evaluate_pending, try_simple_assign};
use expression::lexer::Tokenizer;
use expression::parser::Parser;
use expression::vars::{VarStore, collect_vars};

fn bench_full_pipeline(c: &mut Criterion) {
    let src = "sin(x)^2 + cos(x)^2 + sqrt(9) * ln(e)";
    c.bench_function("lex+parse", |b| {
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
fn bench_assignment(c: &mut Criterion) {
    let src = "x=42";
    let mut tokens = Vec::new();
    let mut arena = Vec::new();
    Tokenizer::new(src).tokenize(&mut tokens).unwrap();
    c.bench_function("assignment", |b| {
        let mut vars = VarStore::new();
        b.iter(|| {
            vars.clear();
            arena.clear();
            let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
            let result = try_simple_assign(&arena, root, &mut vars).unwrap();
            black_box(result);
        });
    });
}
fn bench_pending_solve(c: &mut Criterion) {
    let src = "x+2=5";
    let mut tokens = Vec::new();
    let mut arena = Vec::new();
    let vars = VarStore::new();
    Tokenizer::new(src).tokenize(&mut tokens).unwrap();
    let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
    c.bench_function("pending solve", |b| {
        b.iter(|| {
            let result = evaluate_pending(&arena, root, &vars, src).unwrap();
            black_box(result);
        });
    });
}
fn bench_collect_vars(c: &mut Criterion) {
    let src = "a+b+c+d+sin(e)+sqrt(f)+g*h+i/j+k^l";
    let mut tokens = Vec::new();
    let mut arena = Vec::new();
    Tokenizer::new(src).tokenize(&mut tokens).unwrap();
    let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
    c.bench_function("collect_vars", |b| {
        b.iter(|| {
            let vars = collect_vars(&arena, root);
            black_box(vars.len());
        });
    });
}
criterion_group!(
    benches,
    bench_full_pipeline,
    bench_lexer,
    bench_assignment,
    bench_pending_solve,
    bench_collect_vars
);
criterion_main!(benches);
