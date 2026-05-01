mod common;

use uniseq::Engine;

#[test]
fn resolves_tags_and_links_to_one_page_identity() {
    let root = common::copy_fixture("mixed").expect("copy fixture");
    let engine = Engine::open_workspace(&root).expect("open workspace");
    let page = engine.get_page_view("project-x").expect("page view");

    assert_eq!(page.page.key.as_str(), "project-x");
    assert!(page.incoming.iter().any(|item| item.edge.source_text.contains("#project-x")));
    assert!(page
        .incoming
        .iter()
        .any(|item| item.edge.source_text.contains("[[Project X]]")));
}

#[test]
fn supports_synthetic_pages_without_page_files() {
    let root = common::copy_fixture("mixed").expect("copy fixture");
    let engine = Engine::open_workspace(&root).expect("open workspace");
    let page = engine.get_page_view("research-hub").expect("synthetic page");

    assert!(!page.page.has_page_file);
    assert_eq!(page.page.page_file, None);
    assert_eq!(page.incoming.len(), 1);
}

#[test]
fn parses_front_matter_aliases_and_namespaces() {
    let root = common::copy_fixture("mixed").expect("copy fixture");
    let engine = Engine::open_workspace(&root).expect("open workspace");
    let page = engine
        .get_page_view("areas/work/client-a")
        .expect("namespaced page");

    assert_eq!(page.page.display_title, "Client A");
    assert!(page.page.aliases.contains("client-a"));
    assert_eq!(page.page.namespaces, vec!["areas", "work", "client-a"]);
}
