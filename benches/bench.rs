use criterion::{Criterion, black_box, criterion_group, criterion_main};
use expression::eval::{evaluate_pending, try_simple_assign};
use expression::lexer::Tokenizer;
use expression::parser::{Node, Parser};
use expression::simplify::simplify;
use expression::variables::{VariableStore, collect_variables};

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
        let mut vars = VariableStore::new();
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
    let vars = VariableStore::new();
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
    c.bench_function("collect_variables", |b| {
        b.iter(|| {
            let vars = collect_variables(&arena, root);
            black_box(vars.len());
        });
    });
}

fn push(arena: &mut Vec<Node>, kind: Node) -> u32 {
    let idx = arena.len() as u32;
    arena.push(kind);
    idx
}

fn bench_simplify_only(c: &mut Criterion) {
    let mut arena = Vec::new();
    let n3 = push(&mut arena, Node::Number(3.0));
    let n2 = push(&mut arena, Node::Number(2.0));
    let n0 = push(&mut arena, Node::Number(0.0));
    let n1 = push(&mut arena, Node::Number(1.0));
    let sum = push(&mut arena, Node::Add(n2, n0));
    let mul = push(&mut arena, Node::Mul(n3, sum));
    let root = push(&mut arena, Node::Div(mul, n1));

    c.bench_function("simplify only", |b| {
        b.iter(|| {
            let mut arena_copy = arena.clone();
            let new_root = simplify(&mut arena_copy, root);
            black_box(new_root);
        });
    });
}

fn bench_parse_and_simplify(c: &mut Criterion) {
    let src = "sin(0)^2 + cos(0)*sqrt(9) - ln(e)*tan(0) + x*1 + y*0";
    c.bench_function("parse+simplify", |b| {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        b.iter(|| {
            tokens.clear();
            arena.clear();
            Tokenizer::new(black_box(src))
                .tokenize(&mut tokens)
                .unwrap();
            let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
            let simplified = simplify(&mut arena, root);
            black_box(simplified);
        });
    });
}

fn bench_simplify_deep(c: &mut Criterion) {
    let src = "(((x+0)*1)^1 - 0)/1";
    c.bench_function("simplify deep", |b| {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        b.iter(|| {
            tokens.clear();
            arena.clear();
            Tokenizer::new(black_box(src))
                .tokenize(&mut tokens)
                .unwrap();
            let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
            let simplified = simplify(&mut arena, root);
            black_box(simplified);
        });
    });
}

criterion_group!(
    benches,
    bench_full_pipeline,
    bench_lexer,
    bench_assignment,
    bench_pending_solve,
    bench_collect_vars,
    bench_simplify_only,
    bench_parse_and_simplify,
    bench_simplify_deep
);
criterion_main!(benches);
