use super::{Block, BlockKind, CoreError, PlaintextKind, SourceSpan};

const TAB_WIDTH: usize = 4;
const FENCE_MARKER: &str = "```";
const PLAINTEXT_MARKER: char = '\u{25E6}';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkerKind {
    Outliner,
    ExplicitPlaintext,
}

#[derive(Debug, Clone, Copy)]
struct LineInfo<'a> {
    _raw: &'a str,
    start: usize,
    end: usize,
    indent_width: usize,
    marker: Option<MarkerKind>,
    content_start: usize,
    is_fence_line: bool,
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
    let mut stack = Vec::new();
    let mut in_fence = false;

    for line in iter_lines(text) {
        let line = analyze_line(text, line.start(), line.end(), in_fence)?;

        let marker = normalize_marker_for_context(&stack, line.marker, line.indent_width);

        if let Some(marker) = marker {
            close_for_explicit_marker(&mut stack, &mut roots, marker, line.indent_width)?;
            stack.push(start_explicit_block(marker, line));
        } else if !continue_existing_block(&mut stack, &mut roots, line.indent_width, line.end)? {
            stack.push(start_implicit_plaintext_block(line));
        }

        if line.is_fence_line {
            in_fence = !in_fence;
        }
    }

    while !stack.is_empty() {
        close_top_block(&mut stack, &mut roots)?;
    }

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

fn analyze_line<'a>(
    text: &'a str,
    start: usize,
    end: usize,
    in_fence: bool,
) -> Result<LineInfo<'a>, CoreError> {
    let raw = &text[start..end];
    let line_without_newline = raw.strip_suffix('\n').unwrap_or(raw);

    let mut indent_bytes = 0;
    let mut indent_width = 0;
    for ch in line_without_newline.chars() {
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

    let trimmed = &line_without_newline[indent_bytes..];
    let is_fence_line = trimmed.starts_with(FENCE_MARKER);

    let mut marker = None;
    let mut content_start = start;

    if !in_fence {
        if let Some((kind, marker_len)) = explicit_marker(trimmed) {
            marker = Some(kind);
            let after_marker = start + indent_bytes + marker_len;
            let after_space = line_without_newline[after_marker - start..]
                .chars()
                .next()
                .filter(|ch| matches!(ch, ' ' | '\t'))
                .map(|ch| after_marker + ch.len_utf8())
                .unwrap_or(after_marker);
            content_start = after_space;
        }
    }

    Ok(LineInfo {
        _raw: raw,
        start,
        end,
        indent_width,
        marker,
        content_start,
        is_fence_line,
    })
}

fn explicit_marker(trimmed: &str) -> Option<(MarkerKind, usize)> {
    match trimmed.chars().next()? {
        '-' if marker_is_terminated(trimmed, '-'.len_utf8()) => Some((MarkerKind::Outliner, 1)),
        PLAINTEXT_MARKER if marker_is_terminated(trimmed, PLAINTEXT_MARKER.len_utf8()) => {
            Some((MarkerKind::ExplicitPlaintext, PLAINTEXT_MARKER.len_utf8()))
        }
        _ => None,
    }
}

fn normalize_marker_for_context(
    stack: &[OpenBlock],
    marker: Option<MarkerKind>,
    line_indent: usize,
) -> Option<MarkerKind> {
    match marker {
        Some(MarkerKind::ExplicitPlaintext) if !stack.is_empty() && line_indent > 0 => None,
        _ => marker,
    }
}

fn marker_is_terminated(text: &str, marker_len: usize) -> bool {
    text[marker_len..]
        .chars()
        .next()
        .is_none_or(|ch| matches!(ch, ' ' | '\t'))
}

fn close_for_explicit_marker(
    stack: &mut Vec<OpenBlock>,
    roots: &mut Vec<Block>,
    marker: MarkerKind,
    line_indent: usize,
) -> Result<(), CoreError> {
    while stack.last().is_some_and(|block| match marker {
        MarkerKind::Outliner => line_indent <= block.indent_width,
        MarkerKind::ExplicitPlaintext => true,
    }) {
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

fn start_explicit_block(marker: MarkerKind, line: LineInfo<'_>) -> OpenBlock {
    let kind = match marker {
        MarkerKind::Outliner => BlockKind::Outliner,
        MarkerKind::ExplicitPlaintext => BlockKind::Plaintext(PlaintextKind::Explicit),
    };

    OpenBlock {
        kind,
        indent_width: line.indent_width,
        content_column: line.indent_width + 2,
        block_start: line.start,
        content_start: line.content_start,
        content_end: line.end,
        children: Vec::new(),
    }
}

fn start_implicit_plaintext_block(line: LineInfo<'_>) -> OpenBlock {
    OpenBlock {
        kind: BlockKind::Plaintext(PlaintextKind::Implicit),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(blocks: &[Block]) -> Vec<(BlockKind, SourceSpan, SourceSpan)> {
        blocks
            .iter()
            .map(|block| (block.kind, block.block_span, block.content_span))
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
    fn parses_single_explicit_plaintext_block() {
        let text = "\u{25E6} hello\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(
            spans(&blocks),
            vec![(
                BlockKind::Plaintext(PlaintextKind::Explicit),
                SourceSpan::unchecked(0, text.len()),
                SourceSpan::unchecked("\u{25E6} ".len(), text.len()),
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
                BlockKind::Plaintext(PlaintextKind::Implicit),
                SourceSpan::unchecked(0, text.len()),
                SourceSpan::unchecked(0, text.len()),
            )]
        );
    }

    #[test]
    fn parses_nested_mixed_blocks() {
        let text = "- parent\n\t\u{25E6} child text\n\t- child bullet\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(blocks.len(), 1);
        let parent = &blocks[0];
        assert_eq!(parent.kind, BlockKind::Outliner);
        assert_eq!(
            parent.content_span.slice(text).unwrap(),
            "parent\n\t\u{25E6} child text\n"
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
                BlockKind::Plaintext(PlaintextKind::Implicit),
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
    fn explicit_plaintext_block_can_continue_onto_an_indented_next_line() {
        let text = "\u{25E6} first line\n  second line\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].kind,
            BlockKind::Plaintext(PlaintextKind::Explicit)
        );
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
            vec![
                BlockKind::Outliner,
                BlockKind::Plaintext(PlaintextKind::Implicit),
            ]
        );
    }

    #[test]
    fn fenced_code_is_opaque_for_block_start_detection() {
        let text = "\u{25E6} code\n  ```rust\n  - not a block\n  ```\n- next\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(blocks.len(), 2);
        assert_eq!(
            blocks[0].kind,
            BlockKind::Plaintext(PlaintextKind::Explicit)
        );
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
        assert_eq!(
            blocks[1].kind,
            BlockKind::Plaintext(PlaintextKind::Implicit)
        );
        assert_eq!(
            blocks[1].content_span.slice(text).unwrap(),
            "  later text\n"
        );
    }

    #[test]
    fn spans_remain_valid_for_utf8_content() {
        let text = "\u{25E6} 챕터\n";
        let blocks = parse_blocks(text).unwrap();

        let content = blocks[0].content_span.slice(text).unwrap();
        assert_eq!(content, "챕터\n");
    }

    #[test]
    fn explicit_plaintext_blocks_are_root_only() {
        let text = "- parent\n\t\u{25E6} child text\n";
        let blocks = parse_blocks(text).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, BlockKind::Outliner);
        assert!(blocks[0].children.is_empty());
        assert_eq!(
            blocks[0].content_span.slice(text).unwrap(),
            "parent\n\t\u{25E6} child text\n"
        );
    }
}
