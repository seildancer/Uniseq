mod common;

use std::fs;

use uniseq::Engine;
use uniseq::model::{PageKey, page_file_name_for_key};

#[test]
fn creates_workspace_with_canonical_layout() {
    let root = common::copy_fixture("empty").expect("copy fixture");
    let workspace_root = root.join("created");
    let engine = Engine::create_workspace(&workspace_root).expect("create workspace");
    let summary = engine.workspace_summary().expect("summary");

    assert!(engine.workspace.paths.journals.exists());
    assert!(engine.workspace.paths.pages.exists());
    assert!(engine.workspace.paths.assets.exists());
    assert!(engine.workspace.paths.whiteboards.exists());
    assert!(engine.workspace.paths.pdf.exists());
    assert!(engine.workspace.paths.app.exists());
    assert!(engine.workspace.paths.cache.join("index").exists());
    assert_eq!(summary.journal_files, 0);
    assert_eq!(summary.page_files, 0);
}

#[test]
fn opens_workspace_without_rewriting_files() {
    let root = common::copy_fixture("minimal").expect("copy fixture");
    let before = fs::read_to_string(root.join("journals/2026-04-29.md")).expect("read");
    let engine = Engine::open_workspace(&root).expect("open workspace");
    let after = fs::read_to_string(root.join("journals/2026-04-29.md")).expect("read");
    let summary = engine.workspace_summary().expect("summary");

    assert_eq!(before, after);
    assert_eq!(summary.journal_files, 1);
    assert_eq!(summary.page_files, 2);
    assert_eq!(summary.asset_files, 1);
}

#[test]
fn maps_namespaced_pages_to_flat_storage() {
    let key = PageKey::new("areas/work/client-a").expect("page key");
    assert_eq!(page_file_name_for_key(&key), "areas___work___client-a.md");
}

#[test]
fn reports_logseq_degradation_signals() {
    let root = common::copy_fixture("logseq-like").expect("copy fixture");
    let engine = Engine::open_workspace(&root).expect("open workspace");
    let summary = engine.workspace_summary().expect("summary");

    assert!(summary
        .issues
        .iter()
        .any(|issue| issue.construct == "manual block ref"));
    assert!(summary
        .issues
        .iter()
        .any(|issue| issue.construct == "manual block embed"));
}
