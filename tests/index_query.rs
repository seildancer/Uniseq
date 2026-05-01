mod common;

use std::fs;

use uniseq::Engine;

#[test]
fn builds_queries_for_journals_pages_tasks_timeline_and_search() {
    let root = common::copy_fixture("mixed").expect("copy fixture");
    let engine = Engine::open_workspace(&root).expect("open workspace");

    assert_eq!(engine.journal_dates().len(), 2);
    assert_eq!(engine.tasks(Some("areas/work/client-a")).len(), 1);
    assert!(!engine.search("budget").is_empty());
    assert!(!engine.timeline(Some("areas/work/client-a")).is_empty());

    let page = engine
        .get_page_view("areas/work/client-a")
        .expect("page view");
    assert!(!page.incoming.is_empty());
    assert!(!page.open_tasks.is_empty());
}

#[test]
fn cache_is_disposable_and_rebuildable() {
    let root = common::copy_fixture("mixed").expect("copy fixture");
    let mut engine = Engine::open_workspace(&root).expect("open workspace");
    let cache_path = engine.workspace.paths.cache.join("index/summary.txt");
    assert!(cache_path.exists());

    engine.clear_cache().expect("clear cache");
    assert!(!cache_path.exists());

    engine.rebuild_index().expect("rebuild index");
    assert!(cache_path.exists());
    let cache = fs::read_to_string(cache_path).expect("read cache");
    assert!(cache.contains("UNISEQ_CACHE_V1"));
}
