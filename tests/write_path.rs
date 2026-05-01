mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use uniseq::Engine;
use uniseq::model::{FrontMatterPatch, FrontMatterValue, JournalDate};

#[test]
fn append_edit_toggle_rename_and_move_use_rust_write_path() {
    let root = common::copy_fixture("mixed").expect("copy fixture");
    let mut engine = Engine::open_workspace(&root).expect("open workspace");

    let date = JournalDate::parse("2026-05-01").expect("date");
    engine
        .append_journal_entry(&date, "- [ ] New task for #project-x")
        .expect("append journal");
    let journal = fs::read_to_string(root.join("journals/2026-05-01.md")).expect("read journal");
    assert!(journal.contains("New task"));

    let search_hit = engine.search("Budget review").into_iter().next().expect("search hit");
    engine
        .edit_markdown_span(&search_hit.anchor, "Budget review for [[areas/work/client-a]]", "Budget review for [[areas/work/client-a]] and [[Project X]]")
        .expect("edit span");
    let journal = fs::read_to_string(root.join("journals/2026-04-30.md")).expect("read journal");
    assert!(journal.contains("[[Project X]]"));

    let task = engine
        .tasks(Some("areas/work/client-a"))
        .into_iter()
        .next()
        .expect("task");
    engine.toggle_task(&task, true).expect("toggle task");
    let journal = fs::read_to_string(root.join("journals/2026-04-29.md")).expect("read journal");
    assert!(journal.contains("- [x] Follow up"));

    engine
        .rename_page("project-x", "projects/client-a")
        .expect("rename page");
    let renamed_page = root.join("pages/projects___client-a.md");
    assert!(renamed_page.exists());
    let renamed_journal = fs::read_to_string(root.join("journals/2026-04-29.md")).expect("read renamed journal");
    assert!(renamed_journal.contains("#projects/client-a"));
    assert!(renamed_journal.contains("[[projects/client-a]]"));

    let mut patch = FrontMatterPatch {
        values: BTreeMap::new(),
    };
    patch.values.insert(
        "aliases".to_string(),
        FrontMatterValue::List(vec!["client-work".to_string(), "engagement-a".to_string()]),
    );
    engine
        .update_page_front_matter("areas/work/client-a", &patch)
        .expect("update front matter");
    let page_content = fs::read_to_string(root.join("pages/areas___work___client-a.md")).expect("read page");
    assert!(page_content.contains("client-work"));

    engine
        .move_asset(Path::new("2026/sketch.png"), Path::new("2026/sketch-renamed.png"))
        .expect("move asset");
    assert!(root.join("assets/2026/sketch-renamed.png").exists());
    let journal = fs::read_to_string(root.join("journals/2026-04-29.md")).expect("read journal");
    assert!(journal.contains("assets/2026/sketch-renamed.png"));
}

#[test]
fn stale_anchor_fails_safely() {
    let root = common::copy_fixture("mixed").expect("copy fixture");
    let mut engine = Engine::open_workspace(&root).expect("open workspace");
    let search_hit = engine.search("Budget review").into_iter().next().expect("search hit");

    fs::write(
        root.join("journals/2026-04-30.md"),
        "Completely changed line without previous anchor\n",
    )
    .expect("mutate file");

    let error = engine
        .edit_markdown_span(
            &search_hit.anchor,
            "Budget review for [[areas/work/client-a]]",
            "Replacement text",
        )
        .expect_err("stale anchor should fail");
    assert!(error.to_string().contains("stale anchor"));
}
