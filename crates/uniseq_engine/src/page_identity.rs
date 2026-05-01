pub fn normalize_page_path(input: &str) -> String {
    let mut out = input
        .trim()
        .trim_matches('[')
        .trim_matches(']')
        .replace('\\', "/")
        .split('/')
        .map(|part| {
            part.trim()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join("-")
                .to_lowercase()
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");

    while out.contains("--") {
        out = out.replace("--", "-");
    }
    out.trim_matches('-').to_string()
}

pub fn page_path_to_filename(page_path: &str) -> String {
    format!("{}.md", normalize_page_path(page_path).replace('/', "___"))
}

pub fn filename_to_page_path(file_name: &str) -> Option<String> {
    file_name
        .strip_suffix(".md")
        .map(|stem| stem.replace("___", "/"))
        .map(|path| normalize_page_path(&path))
        .filter(|path| !path.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_and_links_share_identity() {
        assert_eq!(
            normalize_page_path("Project X"),
            normalize_page_path("project-x")
        );
        assert_eq!(
            normalize_page_path(" Areas / Work / Client A "),
            "areas/work/client-a"
        );
    }

    #[test]
    fn storage_mapping_is_flat() {
        assert_eq!(page_path_to_filename("people/alice"), "people___alice.md");
        assert_eq!(
            filename_to_page_path("areas___work___client-a.md"),
            Some("areas/work/client-a".into())
        );
    }
}
