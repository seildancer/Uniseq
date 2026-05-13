use std::str::FromStr;

use super::{Block, BlockKind, CoreError, PageId, PageRefOccurrence, SourceSpan};

const TAB_WIDTH: usize = 4;
// Reference suppression intentionally only treats fenced sections as code
// blocks. Indentation-only Markdown code blocks remain part of normal block
// content and still participate in page-ref extraction.
const FENCE_MARKER: &str = "```";

#[derive(Debug, Clone, Copy)]
struct LineInfo {
    start: usize,
    end: usize,
    indent_width: usize,
    is_outliner: bool,
    content_start: usize,
    is_fence_line: bool,
    is_blank: bool,
}

#[derive(Debug, Clone)]
struct OpenBlock {
    kind: BlockKind,
    indent_width: usize,
    content_column: usize,
    block_start: usize,
    content_start: usize,
    content_end: usize,
    children: Vec<Block>,
}

pub fn parse_blocks(text: &str) -> Result<Vec<Block>, CoreError> {
    let mut roots = Vec::new();
    let mut stack: Vec<OpenBlock> = Vec::new();
    let mut in_fence = false;

    for line in iter_lines(text) {
        let line = analyze_line(text, line.start(), line.end(), in_fence)?;

        if line.is_fence_line {
            in_fence = !in_fence;
        }

        if in_fence {
            continue;
        }

        if line.is_blank {
            if let Some(top) = stack.last_mut() {
                top.content_end = line.end;
            }
            continue;
        }

        if line.is_outliner {
            close_for_outliner_marker(&mut stack, &mut roots, line.indent_width)?;
            stack.push(start_outliner_block(line));
        } else if !continue_existing_block(&mut stack, &mut roots, line.indent_width, line.end)? {
            stack.push(start_plaintext_block(line));
        }
    }

    while !stack.is_empty() {
        close_top_block(&mut stack, &mut roots)?;
    }

    populate_outgoing_refs(&mut roots, text)?;

    Ok(roots)
}

fn iter_lines(text: &str) -> impl Iterator<Item = SourceSpan> + '_ {
    let mut start = 0;
    std::iter::from_fn(move || {
        if start >= text.len() {
            return None;
        }

        let tail = &text[start..];
        let end = tail
            .find('\n')
            .map(|offset| start + offset + 1)
            .unwrap_or(text.len());
        let line = SourceSpan::unchecked(start, end);
        start = end;
        Some(line)
    })
}

fn analyze_line(
    text: &str,
    start: usize,
    end: usize,
    in_fence: bool,
) -> Result<LineInfo, CoreError> {
    let raw = &text[start..end];

    let mut indent_bytes = 0;
    let mut indent_width = 0;
    for ch in raw.chars() {
        match ch {
            ' ' => {
                indent_bytes += 1;
                indent_width += 1;
            }
            '\t' => {
                indent_bytes += 1;
                indent_width += TAB_WIDTH;
            }
            _ => break,
        }
    }

    let trimmed = &raw[indent_bytes..];
    let is_blank = trimmed.is_empty();
    let is_fence_line = trimmed.starts_with(FENCE_MARKER);

    let mut is_outliner = false;
    let mut content_start = start;

    if !in_fence && outliner_marker(trimmed) {
        is_outliner = true;
        let after_marker = start + indent_bytes + 1;
        let after_space = raw[after_marker - start..]
            .chars()
            .next()
            .filter(|ch| matches!(ch, ' ' | '\t'))
            .map(|ch| after_marker + ch.len_utf8())
            .unwrap_or(after_marker);
        content_start = after_space;
    }

    Ok(LineInfo {
        start,
        end,
        indent_width,
        is_outliner,
        content_start,
        is_fence_line,
        is_blank,
    })
}

fn outliner_marker(trimmed: &str) -> bool {
    trimmed.starts_with('-') && marker_is_terminated(trimmed, 1)
}

fn marker_is_terminated(text: &str, marker_len: usize) -> bool {
    text[marker_len..]
        .chars()
        .next()
        .is_none_or(|ch| matches!(ch, ' ' | '\t'))
}

fn close_for_outliner_marker(
    stack: &mut Vec<OpenBlock>,
    roots: &mut Vec<Block>,
    line_indent: usize,
) -> Result<(), CoreError> {
    while stack
        .last()
        .is_some_and(|block| line_indent <= block.indent_width)
    {
        close_top_block(stack, roots)?;
    }

    Ok(())
}

fn continue_existing_block(
    stack: &mut Vec<OpenBlock>,
    roots: &mut Vec<Block>,
    line_indent: usize,
    line_end: usize,
) -> Result<bool, CoreError> {
    while let Some(top) = stack.last() {
        if top.children.is_empty() && line_indent >= top.content_column {
            if let Some(top) = stack.last_mut() {
                top.content_end = line_end;
            }
            return Ok(true);
        }

        close_top_block(stack, roots)?;
    }

    Ok(false)
}

fn start_outliner_block(line: LineInfo) -> OpenBlock {
    OpenBlock {
        kind: BlockKind::Outliner,
        indent_width: line.indent_width,
        content_column: line.indent_width + 2,
        block_start: line.start,
        content_start: line.content_start,
        content_end: line.end,
        children: Vec::new(),
    }
}

fn start_plaintext_block(line: LineInfo) -> OpenBlock {
    OpenBlock {
        kind: BlockKind::Plaintext,
        indent_width: line.indent_width,
        content_column: line.indent_width,
        block_start: line.start,
        content_start: line.start,
        content_end: line.end,
        children: Vec::new(),
    }
}

fn close_top_block(stack: &mut Vec<OpenBlock>, roots: &mut Vec<Block>) -> Result<(), CoreError> {
    let open = stack.pop().expect("close_top_block requires a block");
    let block_end = open
        .children
        .last()
        .map(|child| child.block_span.end())
        .unwrap_or(open.content_end)
        .max(open.content_end);
    let block = Block::new(
        open.kind,
        SourceSpan::new(open.block_start, block_end)?,
        SourceSpan::new(open.content_start, open.content_end)?,
        open.children,
        Vec::new(),
    );

    if let Some(parent) = stack.last_mut() {
        parent.children.push(block);
    } else {
        roots.push(block);
    }

    Ok(())
}

fn populate_outgoing_refs(blocks: &mut [Block], text: &str) -> Result<(), CoreError> {
    for block in blocks {
        block.outgoing_refs = extract_page_refs(text, block.content_span)?;
        populate_outgoing_refs(&mut block.children, text)?;
    }

    Ok(())
}

fn extract_page_refs(
    text: &str,
    content_span: SourceSpan,
) -> Result<Vec<PageRefOccurrence>, CoreError> {
    let content = content_span.slice(text)?;
    let mut refs = Vec::new();
    let mut in_fence = false;

    for line_span in iter_lines(content) {
        let line = line_span.slice(content)?;
        let line_without_newline = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = line_without_newline.trim_start_matches([' ', '\t']);
        let is_fence_line = trimmed.starts_with(FENCE_MARKER);

        if !in_fence && !is_fence_line {
            extract_inline_page_refs(
                line_without_newline,
                content_span.start() + line_span.start(),
                &mut refs,
            )?;
        }

        if is_fence_line {
            in_fence = !in_fence;
        }
    }

    Ok(refs)
}

fn extract_inline_page_refs(
    line: &str,
    line_start: usize,
    refs: &mut Vec<PageRefOccurrence>,
) -> Result<(), CoreError> {
    let mut offset = 0;

    while offset < line.len() {
        let tail = &line[offset..];

        if let Some((consumed, page_ref)) = parse_bracket_ref(tail, line_start + offset)? {
            refs.push(page_ref);
            offset += consumed;
            continue;
        }

        if let Some((consumed, page_ref)) = parse_hashtag_ref(line, offset, line_start)? {
            refs.push(page_ref);
            offset += consumed;
            continue;
        }

        let ch = tail
            .chars()
            .next()
            .expect("offset is always positioned on a char boundary");
        offset += ch.len_utf8();
    }

    Ok(())
}

fn parse_bracket_ref(
    tail: &str,
    absolute_start: usize,
) -> Result<Option<(usize, PageRefOccurrence)>, CoreError> {
    if !tail.starts_with("[[") {
        return Ok(None);
    }

    let Some(closing_offset) = tail[2..].find("]]") else {
        return Ok(None);
    };

    let closing_offset = closing_offset + 2;
    let consumed = closing_offset + 2;
    let candidate = &tail[2..closing_offset];

    let Some(target_page_id) = parse_page_id_candidate(candidate) else {
        return Ok(None);
    };

    let ref_span = SourceSpan::new(absolute_start, absolute_start + consumed)?;
    Ok(Some((
        consumed,
        PageRefOccurrence::new(target_page_id, ref_span),
    )))
}

fn parse_hashtag_ref(
    line: &str,
    offset: usize,
    line_start: usize,
) -> Result<Option<(usize, PageRefOccurrence)>, CoreError> {
    let tail = &line[offset..];
    if !tail.starts_with('#') {
        return Ok(None);
    }

    let previous = line[..offset].chars().next_back();
    if previous.is_some_and(is_hashtag_ref_char) {
        return Ok(None);
    }

    let mut end = offset + '#'.len_utf8();
    let mut consumed_all = true;
    for (relative, ch) in line[end..].char_indices() {
        if !is_hashtag_ref_char(ch) {
            end += relative;
            consumed_all = false;
            break;
        }
    }

    if consumed_all {
        end = line.len();
    }

    if end == offset + '#'.len_utf8() {
        return Ok(None);
    }

    let candidate_end = trim_trailing_hashtag_punctuation(line, offset + '#'.len_utf8(), end);
    if candidate_end == offset + '#'.len_utf8() {
        return Ok(None);
    }

    let candidate = &line[offset + '#'.len_utf8()..candidate_end];
    let Some(target_page_id) = parse_page_id_candidate(candidate) else {
        return Ok(None);
    };

    let ref_span = SourceSpan::new(line_start + offset, line_start + candidate_end)?;
    Ok(Some((
        candidate_end - offset,
        PageRefOccurrence::new(target_page_id, ref_span),
    )))
}

fn parse_page_id_candidate(candidate: &str) -> Option<PageId> {
    let trimmed = candidate.trim();
    (!trimmed.is_empty())
        .then(|| PageId::from_str(trimmed).ok())
        .flatten()
}

fn trim_trailing_hashtag_punctuation(line: &str, start: usize, end: usize) -> usize {
    let mut trimmed_end = end;

    while trimmed_end > start {
        let ch = line[..trimmed_end]
            .chars()
            .next_back()
            .expect("trimmed_end stays on a char boundary");

        if !matches!(ch, '.' | ',' | '!' | '?' | ';' | ':' | ')' | ']' | '}') {
            break;
        }

        trimmed_end -= ch.len_utf8();
    }

    trimmed_end
}

fn is_hashtag_ref_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | '/' | '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(blocks: &[Block]) -> Vec<(BlockKind, SourceSpan, SourceSpan)> {
        blocks
            .iter()
            .map(|block| (block.kind, block.block_span, block.content_span))
            .collect()
    }

    fn ref_targets(block: &Block) -> Vec<String> {
        block
            .outgoing_refs
            .iter()
            .map(|page_ref| page_ref.target_page_id.hierarchy_display())
            .collect()
    }

    #[test]
    fn empty_file_has_no_blocks() {
        assert!(parse_blocks("").unwrap().is_empty());
    }

    #[test]
    fn parses_single_outliner_block() {
        let text = "- hello\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(
            spans(&blocks),
            vec![(
                BlockKind::Outliner,
                SourceSpan::unchecked(0, text.len()),
                SourceSpan::unchecked(2, text.len()),
            )]
        );
    }

    #[test]
    fn parses_non_uniseq_markdown_as_implicit_plaintext() {
        let text = "hello\nworld\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(
            spans(&blocks),
            vec![(
                BlockKind::Plaintext,
                SourceSpan::unchecked(0, text.len()),
                SourceSpan::unchecked(0, text.len()),
            )]
        );
    }

    #[test]
    fn parses_nested_mixed_blocks() {
        let text = "- parent\n  child text\n\t- child bullet\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(blocks.len(), 1);
        let parent = &blocks[0];
        assert_eq!(parent.kind, BlockKind::Outliner);
        assert_eq!(
            parent.content_span.slice(text).unwrap(),
            "parent\n  child text\n"
        );
        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.children[0].kind, BlockKind::Outliner);
    }

    #[test]
    fn separates_unattached_text_into_plaintext_blocks() {
        let text = "- one\nloose text\n- two\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(
            blocks.iter().map(|block| block.kind).collect::<Vec<_>>(),
            vec![
                BlockKind::Outliner,
                BlockKind::Plaintext,
                BlockKind::Outliner,
            ]
        );
    }

    #[test]
    fn continuation_lines_attach_when_they_reach_the_content_column() {
        let text = "- one\n  still one\n- two\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, BlockKind::Outliner);
        assert_eq!(blocks[0].content_span, SourceSpan::unchecked(2, 18));
    }

    #[test]
    fn outliner_block_can_continue_onto_an_indented_next_line() {
        let text = "- first line\n  second line\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, BlockKind::Outliner);
        assert_eq!(
            blocks[0].content_span.slice(text).unwrap(),
            "first line\n  second line\n"
        );
    }

    #[test]
    fn less_indented_text_becomes_a_separate_plaintext_block() {
        let text = "\t- one\n text\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(
            blocks.iter().map(|block| block.kind).collect::<Vec<_>>(),
            vec![BlockKind::Outliner, BlockKind::Plaintext,]
        );
    }

    #[test]
    fn fenced_code_is_opaque_for_block_start_detection() {
        let text = "- code\n  ```rust\n  - not a block\n  ```\n- next\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, BlockKind::Outliner);
        assert!(blocks[0].children.is_empty());
        assert_eq!(blocks[1].kind, BlockKind::Outliner);
    }

    #[test]
    fn parent_content_span_stops_before_children() {
        let text = "- parent\n\t- child\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].content_span, SourceSpan::unchecked(2, 9));
        assert_eq!(
            blocks[0].children[0].block_span,
            SourceSpan::unchecked(9, text.len())
        );
    }

    #[test]
    fn text_after_a_child_does_not_reattach_to_the_parent_content_span() {
        let text = "- parent\n\t- child\n  later text\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, BlockKind::Outliner);
        assert_eq!(blocks[0].content_span.slice(text).unwrap(), "parent\n");
        assert_eq!(blocks[0].children.len(), 1);
        assert_eq!(blocks[0].children[0].kind, BlockKind::Outliner);
        assert_eq!(
            blocks[0].children[0].content_span.slice(text).unwrap(),
            "child\n"
        );
        assert_eq!(blocks[1].kind, BlockKind::Plaintext);
        assert_eq!(
            blocks[1].content_span.slice(text).unwrap(),
            "  later text\n"
        );
    }

    #[test]
    fn spans_remain_valid_for_utf8_content() {
        let text = "챕터\n";
        let blocks = parse_blocks(text).unwrap();

        let content = blocks[0].content_span.slice(text).unwrap();
        assert_eq!(content, "챕터\n");
    }

    #[test]
    fn extracts_bracket_and_hashtag_page_refs() {
        let text = "- visit [[A/B]] and #C\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(ref_targets(&blocks[0]), vec!["A/B", "C"]);
        assert_eq!(
            blocks[0]
                .outgoing_refs
                .iter()
                .map(|page_ref| page_ref.ref_span.slice(text).unwrap())
                .collect::<Vec<_>>(),
            vec!["[[A/B]]", "#C"]
        );
    }

    #[test]
    fn ignores_invalid_or_mid_word_reference_candidates() {
        let text = "- bad [[A___B]] and prefix#Page and #\n";
        let blocks = parse_blocks(text).unwrap();

        assert!(blocks[0].outgoing_refs.is_empty());
    }

    #[test]
    fn ignores_references_inside_fenced_code_content() {
        let text = "- before #A\n  ```rust\n  [[B]]\n  #C\n  ```\n  after [[D]]\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(ref_targets(&blocks[0]), vec!["A", "D"]);
    }

    #[test]
    fn extracts_refs_from_parent_and_child_blocks_independently() {
        let text = "- parent [[A]]\n\t- child #B\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(ref_targets(&blocks[0]), vec!["A"]);
        assert_eq!(ref_targets(&blocks[0].children[0]), vec!["B"]);
    }

    #[test]
    fn ignores_markdown_headings_when_scanning_hashtags() {
        let text = "# Heading\n- body #Page\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].outgoing_refs.is_empty());
        assert_eq!(ref_targets(&blocks[1]), vec!["Page"]);
    }

    #[test]
    fn blank_lines_are_absorbed_into_plaintext() {
        let text = "hello\n\nworld\n";
        let blocks = parse_blocks(text).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, BlockKind::Plaintext);
        assert_eq!(
            blocks[0].content_span.slice(text).unwrap(),
            "hello\n\nworld\n"
        );
    }

    #[test]
    fn multiple_blank_lines_stay_single_plaintext_block() {
        let text = "hello\n\n\n\nworld\n";
        let blocks = parse_blocks(text).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, BlockKind::Plaintext);
        assert_eq!(
            blocks[0].content_span.slice(text).unwrap(),
            "hello\n\n\n\nworld\n"
        );
    }

    #[test]
    fn blank_line_between_outliners_preserves_separator_plaintext() {
        let text = "- one\n\n- two\n";
        let blocks = parse_blocks(text).unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].kind, BlockKind::Outliner);
        assert_eq!(blocks[1].kind, BlockKind::Plaintext);
        assert_eq!(blocks[1].content_span.slice(text).unwrap(), "\n");
        assert_eq!(blocks[2].kind, BlockKind::Outliner);
    }

    #[test]
    fn blank_lines_within_plaintext_are_preserved() {
        let text = "line one\n\n\nline two\n\nline three\n";
        let blocks = parse_blocks(text).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, BlockKind::Plaintext);
        assert_eq!(
            blocks[0].content_span.slice(text).unwrap(),
            "line one\n\n\nline two\n\nline three\n"
        );
    }
}
