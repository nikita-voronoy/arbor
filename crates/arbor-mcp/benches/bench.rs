use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use arbor_analyzers::AnalyzerRegistry;
use arbor_core::palace::Palace;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn analyze_fixture(name: &str) -> Palace {
    let mut palace = Palace::new();
    let registry = AnalyzerRegistry::new();
    registry.analyze_project(&fixture(name), &mut palace).unwrap();
    palace
}

// ============================================================
//  Indexing speed — how fast we can parse a project
// ============================================================

fn bench_indexing(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexing");

    for (name, label) in [
        ("rust-project", "Rust (3 files)"),
        ("c-project", "C (3 files)"),
        ("python-project", "Python (3 files)"),
        ("ts-project", "TypeScript (3 files)"),
        ("go-project", "Go (1 file)"),
        ("ansible-project", "Ansible"),
        ("terraform-project", "Terraform"),
        ("docs-project", "Markdown"),
        ("schema-project", "Schema (SQL+Proto)"),
    ] {
        group.bench_with_input(BenchmarkId::new("analyze", label), &name, |b, &name| {
            b.iter(|| {
                let mut palace = Palace::new();
                let registry = AnalyzerRegistry::new();
                registry.analyze_project(&fixture(name), &mut palace).unwrap();
                black_box(&palace);
            });
        });
    }

    group.finish();
}

// Self-analysis: arbor indexes itself
fn bench_self_indexing(c: &mut Criterion) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    c.bench_function("indexing/self (arbor ~9kLOC)", |b| {
        b.iter(|| {
            let mut palace = Palace::new();
            let registry = AnalyzerRegistry::new();
            registry.analyze_project(&root, &mut palace).unwrap();
            black_box(&palace);
        });
    });
}

// ============================================================
//  Query speed — operations on an already-indexed project
// ============================================================

fn bench_queries(c: &mut Criterion) {
    // Index arbor itself for realistic query benchmarks
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut palace = Palace::new();
    let registry = AnalyzerRegistry::new();
    registry.analyze_project(&root, &mut palace).unwrap();

    let mut group = c.benchmark_group("query");

    group.bench_function("boot", |b| {
        b.iter(|| black_box(palace.boot("arbor", "rust")));
    });

    group.bench_function("skeleton (full)", |b| {
        b.iter(|| black_box(palace.skeleton(None, 3)));
    });

    group.bench_function("compact_skeleton (full)", |b| {
        b.iter(|| black_box(palace.compact_skeleton(None, 500, true)));
    });

    group.bench_function("search (exact)", |b| {
        b.iter(|| black_box(palace.search("Palace")));
    });

    group.bench_function("search (substring)", |b| {
        b.iter(|| black_box(palace.search("analyz")));
    });

    group.bench_function("references", |b| {
        b.iter(|| black_box(palace.references("Palace")));
    });

    group.bench_function("find_primary", |b| {
        b.iter(|| black_box(palace.find_primary("Palace")));
    });

    if let Some(idx) = palace.find_primary("Palace") {
        group.bench_function("dependencies (depth 5)", |b| {
            b.iter(|| black_box(palace.dependencies(idx, 5)));
        });

        group.bench_function("impact (depth 5)", |b| {
            b.iter(|| black_box(palace.impact(idx, 5)));
        });

        group.bench_function("impact (depth 10)", |b| {
            b.iter(|| black_box(palace.impact(idx, 10)));
        });
    }

    group.finish();
}

// ============================================================
//  Queries on C project — struct-heavy with type refs
// ============================================================

fn bench_c_queries(c: &mut Criterion) {
    let palace = analyze_fixture("c-project");

    let mut group = c.benchmark_group("query_c");

    group.bench_function("search (Connection)", |b| {
        b.iter(|| black_box(palace.search("Connection")));
    });

    group.bench_function("references (Connection)", |b| {
        b.iter(|| black_box(palace.references("Connection")));
    });

    group.bench_function("search (server)", |b| {
        b.iter(|| black_box(palace.search("server")));
    });

    group.bench_function("references (find_connection)", |b| {
        b.iter(|| black_box(palace.references("find_connection")));
    });

    if let Some(idx) = palace.find_primary("server_disconnect") {
        group.bench_function("dependencies (server_disconnect)", |b| {
            b.iter(|| black_box(palace.dependencies(idx, 5)));
        });
    }

    group.finish();
}

// ============================================================
//  Persistence — save/load speed
// ============================================================

fn bench_persistence(c: &mut Criterion) {
    let palace = analyze_fixture("rust-project");

    let mut group = c.benchmark_group("persistence");

    group.bench_function("save (rust-project)", |b| {
        let dir = tempfile::tempdir().unwrap();
        b.iter(|| {
            arbor_persist::store::save(&palace, dir.path()).unwrap();
            black_box(());
        });
    });

    group.bench_function("load (rust-project)", |b| {
        let dir = tempfile::tempdir().unwrap();
        arbor_persist::store::save(&palace, dir.path()).unwrap();
        b.iter(|| {
            let loaded = arbor_persist::store::load(dir.path()).unwrap();
            black_box(loaded);
        });
    });

    // Larger: self
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut big_palace = Palace::new();
    let registry = AnalyzerRegistry::new();
    registry.analyze_project(&root, &mut big_palace).unwrap();

    group.bench_function("save (self ~9kLOC)", |b| {
        let dir = tempfile::tempdir().unwrap();
        b.iter(|| {
            arbor_persist::store::save(&big_palace, dir.path()).unwrap();
            black_box(());
        });
    });

    group.bench_function("load (self ~9kLOC)", |b| {
        let dir = tempfile::tempdir().unwrap();
        arbor_persist::store::save(&big_palace, dir.path()).unwrap();
        b.iter(|| {
            let loaded = arbor_persist::store::load(dir.path()).unwrap();
            black_box(loaded);
        });
    });

    group.finish();
}

// ============================================================
//  Incremental — remove + re-analyze single file
// ============================================================

fn bench_incremental(c: &mut Criterion) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    c.bench_function("incremental/remove_file", |b| {
        b.iter_batched(
            || {
                let mut palace = Palace::new();
                let registry = AnalyzerRegistry::new();
                registry.analyze_project(&root, &mut palace).unwrap();
                palace
            },
            |mut palace| {
                let file = root.join("crates/arbor-core/src/palace.rs");
                palace.remove_file(&file);
                black_box(&palace);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("incremental/re-analyze_single_file", |b| {
        let file = root.join("crates/arbor-core/src/palace.rs");
        let source = std::fs::read_to_string(&file).unwrap();
        let analyzer = arbor_analyzers::code::CodeAnalyzer::new();

        b.iter_batched(
            || {
                let mut palace = Palace::new();
                let registry = AnalyzerRegistry::new();
                registry.analyze_project(&root, &mut palace).unwrap();
                palace
            },
            |mut palace| {
                palace.remove_file(&file);
                use arbor_analyzers::Analyzer;
                analyzer.analyze_file(&file, &source, &mut palace).unwrap();
                black_box(&palace);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

// ============================================================
//  Detection speed
// ============================================================

fn bench_detect(c: &mut Criterion) {
    let mut group = c.benchmark_group("detect");

    for (name, label) in [
        ("rust-project", "Rust"),
        ("c-project", "C"),
        ("python-project", "Python"),
        ("ansible-project", "Ansible"),
    ] {
        group.bench_with_input(BenchmarkId::new("detect", label), &name, |b, &name| {
            b.iter(|| black_box(arbor_detect::detect(&fixture(name))));
        });
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_millis(500))
        .measurement_time(std::time::Duration::from_secs(2))
        .sample_size(20);
    targets =
        bench_indexing,
        bench_self_indexing,
        bench_queries,
        bench_c_queries,
        bench_persistence,
        bench_incremental,
        bench_detect,
}
criterion_main!(benches);
