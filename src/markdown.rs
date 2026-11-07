use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Parsed on the commit worker; rendering never reparses the message.
#[derive(Clone, Debug, Default)]
pub struct Document(pub Vec<Block>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Paragraph,
    Heading(u8),
    Quote,
    List(Option<u64>),
    Item,
    Code,
    Rule,
    Table,
    TableHead,
    TableRow,
    TableCell,
}
#[derive(Clone, Debug)]
pub struct Block {
    pub kind: Kind,
    pub spans: Vec<Span>,
    pub children: Vec<Block>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Span {
    pub text: String,
    pub style: InlineStyle,
}
impl Block {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            spans: Vec::new(),
            children: Vec::new(),
        }
    }
}
impl Document {
    pub fn parse(source: &str) -> Self {
        let options =
            Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES;
        let mut stack = vec![Block::new(Kind::Paragraph)];
        let mut styles = vec![InlineStyle::default()];
        for event in Parser::new_ext(source, options) {
            match event {
                Event::Start(tag) => {
                    let kind = match tag {
                        Tag::Paragraph | Tag::HtmlBlock => Some(Kind::Paragraph),
                        Tag::Heading { level, .. } => Some(Kind::Heading(level as u8)),
                        Tag::BlockQuote(_) => Some(Kind::Quote),
                        Tag::List(start) => Some(Kind::List(start)),
                        Tag::Item => Some(Kind::Item),
                        Tag::CodeBlock(_) => Some(Kind::Code),
                        Tag::Table(_) => Some(Kind::Table),
                        Tag::TableHead => Some(Kind::TableHead),
                        Tag::TableRow => Some(Kind::TableRow),
                        Tag::TableCell => Some(Kind::TableCell),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        stack.push(Block::new(kind));
                    } else {
                        let mut style = styles.last().unwrap().clone();
                        match tag {
                            Tag::Strong => style.bold = true,
                            Tag::Emphasis => style.italic = true,
                            Tag::Strikethrough => style.strike = true,
                            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                                style.link = Some(dest_url.into_string())
                            }
                            _ => {}
                        }
                        styles.push(style);
                    }
                }
                Event::End(tag) => {
                    if matches!(
                        tag,
                        TagEnd::Paragraph
                            | TagEnd::HtmlBlock
                            | TagEnd::Heading(_)
                            | TagEnd::BlockQuote(_)
                            | TagEnd::List(_)
                            | TagEnd::Item
                            | TagEnd::CodeBlock
                            | TagEnd::Table
                            | TagEnd::TableHead
                            | TagEnd::TableRow
                            | TagEnd::TableCell
                    ) {
                        let block = stack.pop().unwrap();
                        stack.last_mut().unwrap().children.push(block);
                    } else {
                        styles.pop();
                    }
                }
                Event::Rule => stack
                    .last_mut()
                    .unwrap()
                    .children
                    .push(Block::new(Kind::Rule)),
                event => {
                    let mut style = styles.last().unwrap().clone();
                    let text = match event {
                        Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                            text.into_string()
                        }
                        Event::Code(text) => {
                            style.code = true;
                            text.into_string()
                        }
                        // Commit prose often contains meaningful hard-wrapped lines.
                        Event::SoftBreak | Event::HardBreak => "\n".into(),
                        Event::TaskListMarker(done) => if done { "☑ " } else { "☐ " }.into(),
                        _ => continue,
                    };
                    let spans = &mut stack.last_mut().unwrap().spans;
                    if let Some(last) = spans.last_mut().filter(|s| s.style == style) {
                        last.text.push_str(&text);
                    } else {
                        spans.push(Span { text, style });
                    }
                }
            }
        }
        Self(stack.pop().unwrap().children)
    }
}

/// Only explicit user clicks on web/mail links reach the external URL handler.
/// Image syntax renders its alt text; HTML is literal text, never executable.
pub fn external_link(url: &str) -> bool {
    !url.chars().any(char::is_control)
        && url.split_once(':').is_some_and(|(scheme, rest)| {
            !rest.is_empty()
                && matches!(
                    scheme.to_ascii_lowercase().as_str(),
                    "http" | "https" | "mailto"
                )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_blocks_and_nested_inline_styles_without_changing_source() {
        let doc = Document::parse(
            "## 标题\n\n**粗体 *斜体***、`code`、~~删除~~ [链接](https://example.invalid)\n\n> 引用\n\n1. 第一项\n2. 第二项\n   - 子项\n\n- [x] 完成\n\n```rust\nlet a = 1;\n```\n\n| A | B |\n|---|---|\n| 1 | 2 |\n",
        );
        assert_eq!(doc.0[0].kind, Kind::Heading(2));
        let spans = &doc.0[1].spans;
        assert!(spans.iter().any(|s| s.style.bold && s.style.italic));
        assert!(spans.iter().any(|s| s.style.code && s.text == "code"));
        assert!(spans.iter().any(|s| s.style.strike));
        assert!(
            spans
                .iter()
                .any(|s| s.style.link.as_deref() == Some("https://example.invalid"))
        );
        assert!(doc.0.iter().any(|b| b.kind == Kind::Quote));
        assert!(doc.0.iter().any(|b| b.kind == Kind::List(Some(1))));
        assert!(
            doc.0
                .iter()
                .any(|b| b.kind == Kind::Code && b.spans[0].text == "let a = 1;\n")
        );
        assert!(
            doc.0
                .iter()
                .any(|b| b.kind == Kind::Table && b.children.len() == 2)
        );
    }
    #[test]
    fn plain_lines_and_literal_html_are_retained_and_links_are_restricted() {
        let doc = Document::parse(
            "one\ntwo\n\n<script>alert(1)</script>\n\n![image](https://example.invalid/image.png)",
        );
        assert_eq!(doc.0[0].spans[0].text, "one\ntwo");
        assert!(doc.0[1].spans[0].text.contains("<script>"));
        assert_eq!(doc.0[2].spans[0].text, "image");
        for link in ["https://example.invalid", "mailto:test@example.invalid"] {
            assert!(external_link(link));
        }
        for link in [
            "javascript:alert(1)",
            "file:///tmp/test",
            "data:text/plain,x",
            "https:\ninvalid",
        ] {
            assert!(!external_link(link));
        }
    }
}
