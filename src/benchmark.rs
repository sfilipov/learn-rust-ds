use clap::Parser;
use learn_rust_ds::{avl_hashmap, avl_unsafe, avl_vec, tree};
use std::time::Instant;

#[derive(Parser)]
#[command(name = "tree-benchmark")]
#[command(about = "A tree performance testing tool")]
struct Args {
    #[arg(long, default_value = "100000")]
    size: usize,

    #[arg(long, default_value = "unsafe")]
    tree: String,
}

fn main() {
    let args = Args::parse();
    let mut tree: Box<dyn tree::TreeOps<usize>> = match args.tree.as_str() {
        "hashmap" => Box::new(avl_hashmap::Tree::new()),
        "unsafe" => Box::new(avl_unsafe::Tree::new()),
        "vec" => Box::new(avl_vec::Tree::new()),
        _ => panic!("Unexpected value for tree: {}", args.tree),
    };

    println!(
        "Running with {} tree and {} node count",
        args.tree, args.size
    );

    let size = args.size;
    let start = Instant::now();
    for i in 0..size {
        tree.insert(i);
    }
    let inserted = Instant::now();
    for i in 0..size {
        assert!(tree.contains(&i));
    }
    let checked_contains = Instant::now();
    for i in 0..size {
        tree.remove(&i);
    }
    let end = Instant::now();

    println!(
        "Inserts took {} ms",
        inserted.saturating_duration_since(start).as_micros() as f32 / 1000.0
    );
    println!(
        "Checking contains took {} ms",
        checked_contains
            .saturating_duration_since(inserted)
            .as_micros() as f32
            / 1000.0
    );
    println!(
        "Removals took {} ms",
        end.saturating_duration_since(checked_contains).as_micros() as f32 / 1000.0
    );
    println!(
        "Total {} ms",
        end.saturating_duration_since(start).as_micros() as f32 / 1000.0
    );
}
