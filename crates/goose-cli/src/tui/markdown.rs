use pulldown_cmark::{
    Alignment as MarkdownAlignment, CodeBlockKind, Event as MarkdownEvent,
    HeadingLevel as MarkdownHeadingLevel, Options, Parser, Tag, TagEnd,
};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use super::{
    bold, display_width, fg, italic, truncate, truncate_flat, wrap_words, CRANBERRY, GOLD,
    RULE_COLOR, TEAL, TEXT_DIM, TEXT_PRIMARY, TEXT_SECONDARY,
};

pub(super) fn push_markdown(lines: &mut Vec<Line<'static>>, text: &str, width: usize) {
    let mut renderer = MarkdownRenderer::new(width);
    renderer.render(text);
    lines.extend(renderer.lines);
}

struct MarkdownRenderer {
    lines: Vec<Line<'static>>,
    width: usize,
    spans: Vec<Span<'static>>,
    styles: Vec<Style>,
    list_stack: Vec<ListState>,
    quote_depth: usize,
    block: MarkdownBlock,
    link_urls: Vec<String>,
    code_lang: Option<String>,
    code_text: String,
    table: Option<TableState>,
}

struct ListState {
    next: Option<u64>,
}

#[derive(Default)]
enum MarkdownBlock {
    #[default]
    None,
    Paragraph,
    Heading(MarkdownHeadingLevel),
    Item,
    TableCell,
}

#[derive(Default)]
struct TableState {
    alignments: Vec<MarkdownAlignment>,
    rows: Vec<Vec<Vec<Span<'static>>>>,
    current_row: Vec<Vec<Span<'static>>>,
    current_cell: Vec<Span<'static>>,
}

impl MarkdownRenderer {
    fn new(width: usize) -> Self {
        Self {
            lines: Vec::new(),
            width: width.max(10),
            spans: Vec::new(),
            styles: vec![fg(TEXT_PRIMARY)],
            list_stack: Vec::new(),
            quote_depth: 0,
            block: MarkdownBlock::None,
            link_urls: Vec::new(),
            code_lang: None,
            code_text: String::new(),
            table: None,
        }
    }

    fn render(&mut self, text: &str) {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_SMART_PUNCTUATION);
        options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
        options.insert(Options::ENABLE_DEFINITION_LIST);
        options.insert(Options::ENABLE_GFM);

        for event in Parser::new_ext(text, options) {
            self.handle_event(event);
        }
        self.finish_inline_block();
        self.finish_code_block();
        self.finish_table();
    }

    fn handle_event(&mut self, event: MarkdownEvent<'_>) {
        match event {
            MarkdownEvent::Start(tag) => self.start_tag(tag),
            MarkdownEvent::End(tag) => self.end_tag(tag),
            MarkdownEvent::Text(text) => self.push_text(text.as_ref()),
            MarkdownEvent::Code(code) => self.push_styled(code.as_ref(), bold(GOLD)),
            MarkdownEvent::InlineMath(math) => self.push_styled(&format!("${math}$"), italic(GOLD)),
            MarkdownEvent::DisplayMath(math) => {
                self.finish_inline_block();
                self.lines
                    .push(Line::from(Span::styled(format!("  {math}"), italic(GOLD))));
            }
            MarkdownEvent::Html(html) | MarkdownEvent::InlineHtml(html) => {
                self.push_styled(html.as_ref(), fg(TEXT_DIM));
            }
            MarkdownEvent::FootnoteReference(reference) => {
                self.push_styled(&format!("[{reference}]"), bold(TEAL));
            }
            MarkdownEvent::SoftBreak => self.push_text(" "),
            MarkdownEvent::HardBreak => self.push_text("\n"),
            MarkdownEvent::Rule => {
                self.finish_inline_block();
                self.lines.push(Line::from(Span::styled(
                    "─".repeat(self.width),
                    fg(RULE_COLOR),
                )));
            }
            MarkdownEvent::TaskListMarker(checked) => {
                self.push_styled(if checked { "☑ " } else { "☐ " }, fg(TEAL));
            }
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.block = MarkdownBlock::Paragraph,
            Tag::Heading { level, .. } => self.block = MarkdownBlock::Heading(level),
            Tag::BlockQuote(_) => self.quote_depth += 1,
            Tag::CodeBlock(kind) => {
                self.finish_inline_block();
                self.block = MarkdownBlock::None;
                self.code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let lang = lang.to_string();
                        Some(if lang.is_empty() { "code".into() } else { lang })
                    }
                    CodeBlockKind::Indented => Some("code".into()),
                };
                self.code_text.clear();
            }
            Tag::List(start) => self.list_stack.push(ListState { next: start }),
            Tag::Item => {
                self.block = MarkdownBlock::Item;
                let prefix = self.next_list_prefix();
                self.push_styled(&prefix, fg(TEAL));
            }
            Tag::Emphasis => self.push_style(
                Style::default()
                    .fg(TEXT_SECONDARY)
                    .add_modifier(Modifier::ITALIC),
            ),
            Tag::Strong => self.push_style(
                Style::default()
                    .fg(TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            ),
            Tag::Strikethrough => self.push_style(
                Style::default()
                    .fg(TEXT_DIM)
                    .add_modifier(Modifier::CROSSED_OUT),
            ),
            Tag::Superscript => self.push_style(fg(GOLD)),
            Tag::Subscript => self.push_style(fg(TEXT_DIM)),
            Tag::Link { dest_url, .. } => {
                self.link_urls.push(dest_url.to_string());
                self.push_style(fg(TEAL).add_modifier(Modifier::UNDERLINED));
            }
            Tag::Image { dest_url, .. } => {
                self.link_urls.push(dest_url.to_string());
                self.push_styled("[image: ", fg(TEXT_DIM));
                self.push_style(fg(TEAL).add_modifier(Modifier::UNDERLINED));
            }
            Tag::Table(alignments) => {
                self.finish_inline_block();
                self.table = Some(TableState {
                    alignments,
                    ..TableState::default()
                });
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(table) = &mut self.table {
                    table.current_row.clear();
                }
            }
            Tag::TableCell => {
                self.block = MarkdownBlock::TableCell;
                self.spans.clear();
            }
            Tag::HtmlBlock
            | Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::MetadataBlock(_) => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Item => self.finish_inline_block(),
            TagEnd::Heading(_) => self.finish_heading(),
            TagEnd::BlockQuote(_) => self.quote_depth = self.quote_depth.saturating_sub(1),
            TagEnd::CodeBlock => self.finish_code_block(),
            TagEnd::List(_) => {
                self.finish_inline_block();
                self.list_stack.pop();
            }
            TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Superscript
            | TagEnd::Subscript => {
                self.styles.pop();
            }
            TagEnd::Link => {
                self.styles.pop();
                if let Some(url) = self.link_urls.pop() {
                    self.push_styled(&format!(" ({url})"), fg(TEXT_DIM));
                }
            }
            TagEnd::Image => {
                self.styles.pop();
                if let Some(url) = self.link_urls.pop() {
                    self.push_styled(&format!("]({url})"), fg(TEXT_DIM));
                }
            }
            TagEnd::TableCell => {
                if let Some(table) = &mut self.table {
                    table.current_cell = std::mem::take(&mut self.spans);
                    table
                        .current_row
                        .push(std::mem::take(&mut table.current_cell));
                }
                self.block = MarkdownBlock::None;
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                if let Some(table) = &mut self.table {
                    table.rows.push(std::mem::take(&mut table.current_row));
                }
            }
            TagEnd::Table => self.finish_table(),
            TagEnd::HtmlBlock
            | TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::MetadataBlock(_) => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        if self.code_lang.is_some() {
            self.code_text.push_str(text);
        } else {
            self.push_styled(text, *self.styles.last().unwrap_or(&fg(TEXT_PRIMARY)));
        }
    }

    fn push_style(&mut self, style: Style) {
        self.styles.push(style);
    }

    fn push_styled(&mut self, text: &str, style: Style) {
        self.spans.push(Span::styled(text.to_string(), style));
    }

    fn next_list_prefix(&mut self) -> String {
        match self
            .list_stack
            .last_mut()
            .and_then(|list| list.next.as_mut())
        {
            Some(next) => {
                let prefix = format!("{next}. ");
                *next += 1;
                prefix
            }
            None => "• ".to_string(),
        }
    }

    fn finish_inline_block(&mut self) {
        if self.spans.is_empty() {
            self.block = MarkdownBlock::None;
            return;
        }
        let prefix = quote_prefix(self.quote_depth);
        let available = self.width.saturating_sub(display_width(&prefix)).max(1);
        let wrapped = wrap_spans(std::mem::take(&mut self.spans), available);
        for line in wrapped {
            let mut spans = Vec::new();
            if !prefix.is_empty() {
                spans.push(Span::styled(prefix.clone(), fg(RULE_COLOR)));
            }
            spans.extend(line);
            self.lines.push(Line::from(spans));
        }
        self.block = MarkdownBlock::None;
    }

    fn finish_heading(&mut self) {
        let level = match self.block {
            MarkdownBlock::Heading(level) => level,
            _ => MarkdownHeadingLevel::H3,
        };
        let text = spans_plain_text(&self.spans);
        self.spans.clear();
        if text.is_empty() {
            self.block = MarkdownBlock::None;
            return;
        }

        let (prefix, style, underline) = match level {
            MarkdownHeadingLevel::H1 => ("# ", bold(CRANBERRY), true),
            MarkdownHeadingLevel::H2 => ("## ", bold(GOLD), true),
            MarkdownHeadingLevel::H3 => (
                "### ",
                Style::default()
                    .fg(TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD),
                false,
            ),
            _ => (
                "",
                Style::default()
                    .fg(TEXT_SECONDARY)
                    .add_modifier(Modifier::BOLD),
                false,
            ),
        };
        for (index, line) in wrap_words(&text, self.width.saturating_sub(prefix.len()))
            .into_iter()
            .enumerate()
        {
            let marker = if index == 0 { prefix } else { "  " };
            self.lines.push(Line::from(vec![
                Span::styled(marker.to_string(), fg(RULE_COLOR)),
                Span::styled(line, style),
            ]));
        }
        if underline {
            self.lines.push(Line::from(Span::styled(
                "─".repeat(self.width.min(text.chars().count() + prefix.len())),
                fg(RULE_COLOR),
            )));
        }
        self.block = MarkdownBlock::None;
    }

    fn finish_code_block(&mut self) {
        if self.code_text.is_empty() && self.code_lang.is_none() {
            return;
        }
        let label = self.code_lang.take().unwrap_or_else(|| "code".into());
        self.lines.push(Line::from(vec![
            Span::styled("╭─ ", fg(RULE_COLOR)),
            Span::styled(label, italic(TEXT_DIM)),
        ]));
        for raw in self.code_text.trim_end_matches('\n').lines() {
            self.lines.push(Line::from(vec![
                Span::styled("│ ", fg(RULE_COLOR)),
                Span::styled(
                    truncate(raw, self.width.saturating_sub(2)),
                    fg(TEXT_SECONDARY),
                ),
            ]));
        }
        self.lines
            .push(Line::from(Span::styled("╰", fg(RULE_COLOR))));
        self.code_text.clear();
        self.block = MarkdownBlock::None;
    }

    fn finish_table(&mut self) {
        let Some(table) = self.table.take() else {
            return;
        };
        if table.rows.is_empty() {
            return;
        }
        self.lines.extend(render_table(table, self.width));
    }
}

fn wrap_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Vec<Span<'static>>> {
    let width = width.max(1);
    let mut lines = vec![Vec::new()];
    let mut current_width = 0;

    for span in spans {
        let style = span.style;
        for segment in span.content.split_inclusive('\n') {
            let hard_break = segment.ends_with('\n');
            let segment = segment.trim_end_matches('\n');
            for word in segment.split_whitespace() {
                let word_width = display_width(word);
                if current_width == 0 {
                    lines
                        .last_mut()
                        .expect("line exists")
                        .push(Span::styled(word.to_string(), style));
                    current_width = word_width;
                } else if current_width + 1 + word_width <= width {
                    lines
                        .last_mut()
                        .expect("line exists")
                        .push(Span::styled(" ", style));
                    lines
                        .last_mut()
                        .expect("line exists")
                        .push(Span::styled(word.to_string(), style));
                    current_width += 1 + word_width;
                } else {
                    lines.push(vec![Span::styled(word.to_string(), style)]);
                    current_width = word_width;
                }
            }
            if hard_break {
                lines.push(Vec::new());
                current_width = 0;
            }
        }
    }

    lines
}

fn spans_plain_text(spans: &[Span<'_>]) -> String {
    spans.iter().map(|span| span.content.as_ref()).collect()
}

fn quote_prefix(depth: usize) -> String {
    "│ ".repeat(depth)
}

fn render_table(table: TableState, width: usize) -> Vec<Line<'static>> {
    let columns = table.rows.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
        return Vec::new();
    }

    let mut widths = vec![3usize; columns];
    for row in &table.rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(spans_plain_text(cell).chars().count().min(32));
        }
    }

    let chrome = columns.saturating_add(1);
    let separators = columns.saturating_sub(1) * 3;
    let available = width.saturating_sub(chrome + separators).max(columns);
    let total: usize = widths.iter().sum();
    if total > available {
        for column_width in &mut widths {
            *column_width = ((*column_width * available) / total).max(3);
        }
    }

    let mut lines = Vec::new();
    lines.push(table_border('┌', '┬', '┐', &widths));
    for (row_index, row) in table.rows.iter().enumerate() {
        lines.push(table_row(row, &widths, &table.alignments));
        if row_index == 0 {
            lines.push(table_border('├', '┼', '┤', &widths));
        }
    }
    lines.push(table_border('└', '┴', '┘', &widths));
    lines
}

fn table_border(left: char, separator: char, right: char, widths: &[usize]) -> Line<'static> {
    let mut text = String::new();
    text.push(left);
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            text.push(separator);
        }
        text.push_str(&"─".repeat(*width + 2));
    }
    text.push(right);
    Line::from(Span::styled(text, fg(RULE_COLOR)))
}

fn table_row(
    row: &[Vec<Span<'static>>],
    widths: &[usize],
    alignments: &[MarkdownAlignment],
) -> Line<'static> {
    let mut spans = vec![Span::styled("│", fg(RULE_COLOR))];
    for (index, width) in widths.iter().enumerate() {
        let text = row
            .get(index)
            .map(|cell| spans_plain_text(cell))
            .unwrap_or_default();
        let text = truncate_flat(&text, *width);
        let text_width = display_width(&text);
        let remaining = width.saturating_sub(text_width);
        let (left_pad, right_pad) = match alignments
            .get(index)
            .copied()
            .unwrap_or(MarkdownAlignment::None)
        {
            MarkdownAlignment::Right => (remaining, 0),
            MarkdownAlignment::Center => (remaining / 2, remaining - remaining / 2),
            MarkdownAlignment::Left | MarkdownAlignment::None => (0, remaining),
        };
        spans.push(Span::raw(format!(
            " {}{}{} ",
            " ".repeat(left_pad),
            text,
            " ".repeat(right_pad)
        )));
        spans.push(Span::styled("│", fg(RULE_COLOR)));
    }
    Line::from(spans)
}
