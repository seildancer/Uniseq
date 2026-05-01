use chrono::NaiveDate;
use std::fs;
use tempfile::TempDir;
use uniseq_engine::*;

fn fixture() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    create_workspace(dir.path()).unwrap();
    fs::write(
        dir.path().join("journals/2026-04-29.md"),
        "# Daily\n- [ ] ship [[Project X]] #Project-X\n- note about #people/alice\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("pages/project-x.md"),
        "---\naliases:\n  - px\n---\n# Project X\nCurated page note.\n",
    )
    .unwrap();
    fs::write(dir.path().join("pages/people___alice.md"), "# Alice\n").unwrap();
    dir
}

#[test]
fn opens_workspace_without_rewriting_markdown() {
    let dir = fixture();
    let before = fs::read_to_string(dir.path().join("journals/2026-04-29.md")).unwrap();
    let summary = open_workspace(dir.path()).unwrap();
    let after = fs::read_to_string(dir.path().join("journals/2026-04-29.md")).unwrap();
    assert_eq!(before, after);
    assert_eq!(summary.journals.len(), 1);
    assert_eq!(summary.pages.len(), 2);
}

#[test]
fn parses_tags_links_aliases_tasks_and_source_anchors() {
    let dir = fixture();
    let index = build_index(dir.path()).unwrap();
    let journal = query_journal(&index, NaiveDate::from_ymd_opt(2026, 4, 29).unwrap());
    let task = journal
        .iter()
        .find(|e| e.task == Some(TaskState::Todo))
        .unwrap();
    assert!(task.runtime_id.contains("2026-04-29.md"));
    assert_eq!(task.links[0].page_path, "project-x");
    assert_eq!(task.tags[0].page_path, "project-x");
    assert!(!task.anchor.snippet.is_empty());
    let page = query_page(&index, "Project X");
    assert_eq!(page.page_path, "project-x");
    assert_eq!(page.aliases, vec!["px"]);
    assert!(page
        .incoming_entries
        .iter()
        .any(|e| e.text.contains("ship")));
}

#[test]
fn referenced_pages_exist_without_page_files() {
    let dir = tempfile::tempdir().unwrap();
    create_workspace(dir.path()).unwrap();
    fs::write(
        dir.path().join("journals/2026-04-29.md"),
        "- mention #ghost-page\n",
    )
    .unwrap();
    let index = build_index(dir.path()).unwrap();
    assert!(index.pages.contains_key("ghost-page"));
    assert!(!index.pages["ghost-page"].has_file);
}

#[test]
fn cache_is_disposable_and_index_rebuilds() {
    let dir = fixture();
    fs::remove_dir_all(dir.path().join(".cache")).unwrap();
    let index = build_index(dir.path()).unwrap();
    assert!(!search(&index, "curated").is_empty());
}

#[test]
fn writes_are_rust_owned_and_validate_anchors() {
    let dir = fixture();
    let index = build_index(dir.path()).unwrap();
    let task = task_rollup(&index).into_iter().next().unwrap();
    let result = toggle_task(&task.anchor, None).unwrap();
    assert!(result.anchor.is_some());
    let updated = fs::read_to_string(dir.path().join("journals/2026-04-29.md")).unwrap();
    assert!(updated.contains("- [x] ship"));
    assert!(
        toggle_task(&task.anchor, None).is_err(),
        "old anchor should be stale after marker changed"
    );

    let append_result = append_journal_entry(
        dir.path(),
        NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
        "- new #Inbox",
    )
    .unwrap();
    assert!(append_result.anchor.is_some());
    assert!(
        fs::read_to_string(dir.path().join("journals/2026-04-30.md"))
            .unwrap()
            .contains("#Inbox")
    );
}

#[test]
fn page_rename_updates_flat_file_and_references() {
    let dir = fixture();
    // Use the raw form "Project X" so the literal-regex matches the file's [[Project X]]
    rename_page(dir.path(), "Project X", "Project Y").unwrap();
    assert!(dir.path().join("pages").join("project-y.md").exists());
    let journal = fs::read_to_string(dir.path().join("journals/2026-04-29.md")).unwrap();
    assert!(
        journal.contains("[[Project Y]]"),
        "expected [[Project Y]] in journal: {journal}"
    );
    assert!(journal.contains("#project-y"));
}
