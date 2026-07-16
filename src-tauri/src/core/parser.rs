use comrak::{
    nodes::{AstNode, NodeValue},
    parse_document, Arena, Options,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCourse {
    pub title: String,
    pub sections: Vec<ParsedSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSection {
    pub title: String,
    pub level: u8,
    pub body: String,
    pub children: Vec<ParsedSection>,
}

#[derive(Debug, Clone)]
struct FlatHeading {
    level: u8,
    title: String,
    start_line: usize,
    parent: Option<usize>,
}

pub fn parse_markdown_course(markdown: &str, title_hint: Option<&str>) -> ParsedCourse {
    let options = markdown_options();
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &options);

    let mut headings = Vec::new();
    collect_headings(root, &mut headings);
    headings.sort_by_key(|heading| heading.start_line);

    if headings.is_empty() {
        let title = clean_title(title_hint.unwrap_or("Overview"));
        return ParsedCourse {
            title: clean_title(title_hint.unwrap_or("Untitled Course")),
            sections: vec![ParsedSection {
                title,
                level: 1,
                body: markdown.to_string(),
                children: Vec::new(),
            }],
        };
    }

    assign_parents(&mut headings);

    let course_title = headings
        .iter()
        .find(|heading| heading.level == 1)
        .or_else(|| headings.first())
        .map(|heading| heading.title.clone())
        .or_else(|| title_hint.map(clean_title))
        .unwrap_or_else(|| "Untitled Course".to_string());

    let line_offsets = line_start_offsets(markdown);
    let total_lines = line_offsets.len();
    let mut children_by_parent = vec![Vec::new(); headings.len()];
    let mut roots = Vec::new();

    for (index, heading) in headings.iter().enumerate() {
        if let Some(parent) = heading.parent {
            children_by_parent[parent].push(index);
        } else {
            roots.push(index);
        }
    }

    let sections = roots
        .into_iter()
        .map(|index| {
            build_section(
                index,
                &headings,
                &children_by_parent,
                &line_offsets,
                markdown,
                total_lines,
            )
        })
        .collect();

    ParsedCourse {
        title: course_title,
        sections,
    }
}

fn markdown_options<'a>() -> Options<'a> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options
}

fn collect_headings<'a>(node: &'a AstNode<'a>, headings: &mut Vec<FlatHeading>) {
    let ast = node.data.borrow();
    if let NodeValue::Heading(heading) = &ast.value {
        if ast.sourcepos.start.line > 0 {
            let title = clean_title(&collect_plain_text(node));
            headings.push(FlatHeading {
                level: heading.level,
                title: if title.is_empty() {
                    "Untitled Section".to_string()
                } else {
                    title
                },
                start_line: ast.sourcepos.start.line,
                parent: None,
            });
        }
    }
    drop(ast);

    for child in node.children() {
        collect_headings(child, headings);
    }
}

fn collect_plain_text<'a>(node: &'a AstNode<'a>) -> String {
    let ast = node.data.borrow();
    let text = match &ast.value {
        NodeValue::Text(text) => text.clone(),
        NodeValue::Code(code) => code.literal.clone(),
        NodeValue::HtmlInline(html) => html.clone(),
        NodeValue::SoftBreak | NodeValue::LineBreak => " ".to_string(),
        _ => String::new(),
    };
    drop(ast);

    let mut output = text;
    for child in node.children() {
        output.push_str(&collect_plain_text(child));
    }
    output
}

fn assign_parents(headings: &mut [FlatHeading]) {
    let mut stack: Vec<usize> = Vec::new();

    for index in 0..headings.len() {
        while let Some(previous) = stack.last().copied() {
            if headings[previous].level < headings[index].level {
                break;
            }
            stack.pop();
        }

        headings[index].parent = stack.last().copied();
        stack.push(index);
    }
}

fn build_section(
    index: usize,
    headings: &[FlatHeading],
    children_by_parent: &[Vec<usize>],
    line_offsets: &[usize],
    markdown: &str,
    total_lines: usize,
) -> ParsedSection {
    let heading = &headings[index];
    let end_line = headings
        .get(index + 1)
        .map(|next| next.start_line.saturating_sub(1))
        .unwrap_or(total_lines);

    ParsedSection {
        title: heading.title.clone(),
        level: heading.level,
        body: slice_lines(markdown, line_offsets, heading.start_line, end_line),
        children: children_by_parent[index]
            .iter()
            .copied()
            .map(|child| {
                build_section(
                    child,
                    headings,
                    children_by_parent,
                    line_offsets,
                    markdown,
                    total_lines,
                )
            })
            .collect(),
    }
}

fn line_start_offsets(markdown: &str) -> Vec<usize> {
    if markdown.is_empty() {
        return Vec::new();
    }

    let mut offsets = vec![0];
    for (index, byte) in markdown.bytes().enumerate() {
        if byte == b'\n' && index + 1 < markdown.len() {
            offsets.push(index + 1);
        }
    }
    offsets
}

fn slice_lines(
    markdown: &str,
    line_offsets: &[usize],
    start_line: usize,
    end_line: usize,
) -> String {
    if markdown.is_empty() || start_line == 0 || end_line < start_line {
        return String::new();
    }

    let start = line_offsets
        .get(start_line.saturating_sub(1))
        .copied()
        .unwrap_or(markdown.len());
    let end = line_offsets
        .get(end_line)
        .copied()
        .unwrap_or(markdown.len());

    markdown[start..end].to_string()
}

fn clean_title(title: &str) -> String {
    let cleaned = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        "Untitled".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_heading_tree() {
        let markdown = "# Rust\nIntro\n\n## Ownership\nOwn body\n\n### Borrowing\nBorrow body\n\n## Traits\nTrait body\n";

        let course = parse_markdown_course(markdown, None);

        assert_eq!(course.title, "Rust");
        assert_eq!(course.sections.len(), 1);
        let root = &course.sections[0];
        assert_eq!(root.title, "Rust");
        assert_eq!(root.level, 1);
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].title, "Ownership");
        assert_eq!(root.children[0].children[0].title, "Borrowing");
        assert_eq!(root.children[1].title, "Traits");
        assert!(root.body.contains("Intro"));
        assert!(!root.body.contains("Ownership"));
    }

    #[test]
    fn skipped_levels_attach_to_nearest_lower_heading() {
        let markdown = "# Top\n\n### Deep\nBody\n";

        let course = parse_markdown_course(markdown, None);

        assert_eq!(course.sections[0].children.len(), 1);
        assert_eq!(course.sections[0].children[0].title, "Deep");
        assert_eq!(course.sections[0].children[0].level, 3);
    }

    #[test]
    fn no_headings_doc_becomes_overview_section() {
        let markdown = "Just notes\n\n- one\n- two\n";

        let course = parse_markdown_course(markdown, Some("Loose Notes"));

        assert_eq!(course.title, "Loose Notes");
        assert_eq!(course.sections.len(), 1);
        assert_eq!(course.sections[0].title, "Loose Notes");
        assert_eq!(course.sections[0].body, markdown);
    }

    #[test]
    fn gfm_tables_and_task_lists_survive_section_split() {
        let markdown = "# Plan\n\n| A | B |\n| - | - |\n| 1 | 2 |\n\n- [x] Done\n- [ ] Next\n";

        let course = parse_markdown_course(markdown, None);
        let body = &course.sections[0].body;

        assert!(body.contains("| A | B |"));
        assert!(body.contains("- [x] Done"));
        assert!(body.contains("- [ ] Next"));
    }
}
