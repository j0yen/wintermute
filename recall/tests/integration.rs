//! End-to-end: open a store, write a memory through the store + index,
//! query it back, list it, touch it, delete it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use recall::embeddings::{Embedder, HashEmbedder};
use recall::index::Index;
use recall::memory::{Kind, Memory, Subject};
use recall::paths;
use recall::retrieval;
use recall::store::FileStore;

#[test]
fn end_to_end_write_query_delete() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let store = FileStore::open(root.clone()).unwrap();
    let idx = Index::open(&paths::index_db(&root)).unwrap();
    let embedder = HashEmbedder::new();

    let m1 = Memory::new(
        Kind::Semantic,
        Subject::user(),
        "user prefers pnpm for typescript, cargo and uv for python",
    );
    let p1 = store.write(&m1).unwrap();
    let v1 = embedder.embed(&m1.body).unwrap();
    idx.upsert(&m1, &p1, Some((embedder.id(), &v1))).unwrap();

    let m2 = Memory::new(
        Kind::Procedural,
        Subject::project("recall"),
        "build with `cargo build --release` after sourcing ~/.cargo/env",
    );
    let p2 = store.write(&m2).unwrap();
    let v2 = embedder.embed(&m2.body).unwrap();
    idx.upsert(&m2, &p2, Some((embedder.id(), &v2))).unwrap();

    let hits = retrieval::search(&idx, "pnpm typescript", 5).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].hit.id, m1.front.id);

    let hybrid = retrieval::hybrid_search(&idx, &embedder, "pnpm typescript", 5).unwrap();
    assert!(!hybrid.is_empty());
    assert_eq!(hybrid[0].hit.id, m1.front.id);

    let proj_hits = idx.list(Some("project:"), 10).unwrap();
    assert_eq!(proj_hits.len(), 1);
    assert_eq!(proj_hits[0].subject, "project:recall");

    idx.touch_recall(&m1.front.id).unwrap();
    let after = idx.list(Some("user"), 10).unwrap();
    assert_eq!(after[0].recall_count, 1);

    let (found, fpath) = store.find_by_id(&m1.front.id).unwrap();
    assert_eq!(found.front.id, m1.front.id);
    store.delete(&fpath).unwrap();
    idx.remove(&m1.front.id).unwrap();
    assert_eq!(idx.count().unwrap(), 1);
}

#[test]
fn reindex_recovers_from_empty_db() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let store = FileStore::open(root.clone()).unwrap();
    for i in 0..3 {
        let m = Memory::new(
            Kind::Semantic,
            Subject::user(),
            format!("memory number {i}"),
        );
        store.write(&m).unwrap();
    }
    let idx = Index::open(&paths::index_db(&root)).unwrap();
    assert_eq!(idx.count().unwrap(), 0);
    let it = store.iter_all().filter_map(Result::ok);
    let embedder = HashEmbedder::new();
    let n = idx.rebuild_from(it, Some(&embedder)).unwrap();
    assert_eq!(n, 3);
    assert_eq!(idx.count().unwrap(), 3);
}
