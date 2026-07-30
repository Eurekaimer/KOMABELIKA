use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use super::slash::SlashCommand;
use crate::provider::Role;

pub struct VisibleMessage<'a> {
    pub role: &'a Role,
    pub content: &'a str,
    pub interrupted: bool,
}

pub struct SelectionView<'a> {
    pub title: &'a str,
    pub options: &'a [String],
    pub selected: usize,
}

pub struct ChatView<'a> {
    pub session_name: &'a str,
    pub provider: &'a str,
    pub model: &'a str,
    pub generating: bool,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub error: Option<&'a str>,
    pub messages: &'a [VisibleMessage<'a>],
    pub streaming_text: &'a str,
    pub reasoning_text: &'a str,
    pub input: &'a str,
    pub slash_suggestions: &'a [SlashCommand],
    pub selected_suggestion: usize,
    pub model_picker: Option<SelectionView<'a>>,
}

pub fn render(frame: &mut Frame<'_>, view: ChatView<'_>) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(5),
            Constraint::Length(2),
        ])
        .split(area);

    let generation = if view.generating {
        "生成中…"
    } else {
        "就绪"
    };
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " Komari Call ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "会话：{}  Provider：{}  模型：{}  {}",
            view.session_name, view.provider, view.model, generation
        )),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, chunks[0]);

    let mut lines = Vec::new();
    for message in view.messages {
        push_bubble(
            &mut lines,
            message.role,
            message.content,
            message.interrupted,
        );
    }
    if !view.reasoning_text.is_empty() {
        push_bubble(&mut lines, &Role::System, view.reasoning_text, false);
    }
    if !view.streaming_text.is_empty() {
        push_bubble(&mut lines, &Role::Assistant, view.streaming_text, false);
    }
    if lines.is_empty() {
        push_bubble(
            &mut lines,
            &Role::Assistant,
            "……晚上好。想说什么都可以，不必组织得很完整。",
            false,
        );
    }
    let visible_height = chunks[1].height.saturating_sub(2) as usize;
    let scroll = lines.len().saturating_sub(visible_height) as u16;
    let conversation = Paragraph::new(Text::from(lines))
        .block(Block::default().title(" 对话 ").borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(conversation, chunks[1]);

    let input_viewport = input_viewport(view.input, chunks[2]);
    let input = Paragraph::new(view.input)
        .block(Block::default().title(" 输入 ").borders(Borders::ALL))
        .scroll((
            input_viewport.vertical_scroll,
            input_viewport.horizontal_scroll,
        ));
    frame.render_widget(input, chunks[2]);

    let footer_text = view.error.map_or_else(

        || {
            format!(
                "Enter 发送  Esc 取消  Ctrl+N/clear 新会话  Ctrl+P 选择模型  /help 命令  Ctrl+C 退出  tokens {}→{}",
                view.input_tokens, view.output_tokens
            )
        },
        |error| format!("错误：{error}"),
    );
    frame.render_widget(
        Paragraph::new(footer_text).style(Style::default().fg(if view.error.is_some() {
            Color::Red
        } else {
            Color::DarkGray
        })),
        chunks[3],
    );

    render_slash_suggestions(
        frame,
        chunks[2],
        view.slash_suggestions,
        view.selected_suggestion,
    );
    if let Some(picker) = &view.model_picker {
        render_selection(frame, area, picker);
    }

    if !view.generating && view.model_picker.is_none() {
        frame.set_cursor_position((input_viewport.cursor_x, input_viewport.cursor_y));
    }
}

fn push_bubble(lines: &mut Vec<Line<'static>>, role: &Role, content: &str, interrupted: bool) {
    let (name, color, background) = match role {
        Role::User => ("你", Color::Cyan, Color::Rgb(8, 26, 34)),
        Role::Assistant => ("小鞠", Color::Magenta, Color::Rgb(30, 14, 38)),
        Role::System => ("系统", Color::Yellow, Color::Rgb(38, 32, 6)),
    };
    let interrupted = if interrupted { "（已中断）" } else { "" };
    let border_style = Style::default()
        .fg(color)
        .bg(background)
        .add_modifier(Modifier::BOLD);
    lines.push(Line::from(Span::styled(
        format!("╭─ > {name}{interrupted} "),
        border_style,
    )));
    for content_line in content.lines() {
        lines.push(Line::from(vec![
            Span::styled("│ ", border_style),
            Span::styled(
                format!("{content_line} "),
                Style::default().fg(Color::White).bg(background),
            ),
        ]));
    }
    lines.push(Line::from(Span::styled("╰─", border_style)));
    lines.push(Line::default());
}

fn render_slash_suggestions(
    frame: &mut Frame<'_>,
    input_area: Rect,
    suggestions: &[SlashCommand],
    selected: usize,
) {
    if suggestions.is_empty() || input_area.width < 12 {
        return;
    }
    const MAX_VISIBLE: usize = 6;
    let selected = selected.min(suggestions.len() - 1);
    let start = selected
        .saturating_add(1)
        .saturating_sub(MAX_VISIBLE)
        .min(suggestions.len().saturating_sub(MAX_VISIBLE));
    let visible = &suggestions[start..suggestions.len().min(start + MAX_VISIBLE)];
    let height = visible.len() as u16 + 2;
    let area = Rect::new(
        input_area.x,
        input_area.y.saturating_sub(height),
        input_area.width,
        height,
    );
    let lines = visible
        .iter()
        .enumerate()
        .map(|(offset, command)| {
            let is_selected = start + offset == selected;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };
            Line::from(vec![
                Span::styled(format!(" {:<25}", command.usage), style),
                Span::styled(command.description, style),
            ])
        })
        .collect::<Vec<_>>();
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" 命令补全 · Tab 确认 · ↑↓选择 ")
                .title_style(Style::default().fg(Color::Yellow))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        area,
    );
}

fn render_selection(frame: &mut Frame<'_>, outer: Rect, picker: &SelectionView<'_>) {
    if picker.options.is_empty() || outer.width < 16 || outer.height < 6 {
        return;
    }
    let width = outer.width.saturating_mul(3).saturating_div(4).max(16);
    let max_rows = outer.height.saturating_sub(4).min(12) as usize;
    let selected = picker.selected.min(picker.options.len() - 1);
    let start = selected
        .saturating_add(1)
        .saturating_sub(max_rows)
        .min(picker.options.len().saturating_sub(max_rows));
    let visible = &picker.options[start..picker.options.len().min(start + max_rows)];
    let height = visible.len() as u16 + 2;
    let area = Rect::new(
        outer.x + outer.width.saturating_sub(width) / 2,
        outer.y + outer.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let lines = visible
        .iter()
        .enumerate()
        .map(|(offset, model)| {
            let is_selected = start + offset == selected;
            let marker = if is_selected { "▶ " } else { "  " };
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Magenta)
            };
            Line::from(Span::styled(format!("{marker}{model}"), style))
        })
        .collect::<Vec<_>>();
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(format!(" {} ", picker.title))
                .title_style(
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        ),
        area,
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InputViewport {
    vertical_scroll: u16,
    horizontal_scroll: u16,
    cursor_x: u16,
    cursor_y: u16,
}

fn input_viewport(input: &str, area: Rect) -> InputViewport {
    let inner_width = area.width.saturating_sub(2).max(1) as usize;
    let inner_height = area.height.saturating_sub(2).max(1) as usize;
    let line_index = input.matches('\n').count();
    let display_column = UnicodeWidthStr::width(input.rsplit('\n').next().unwrap_or_default());
    let horizontal_scroll = display_column.saturating_sub(inner_width.saturating_sub(1));
    let vertical_scroll = line_index.saturating_sub(inner_height.saturating_sub(1));

    InputViewport {
        vertical_scroll: vertical_scroll.min(u16::MAX as usize) as u16,
        horizontal_scroll: horizontal_scroll.min(u16::MAX as usize) as u16,
        cursor_x: area.x
            + 1
            + display_column
                .saturating_sub(horizontal_scroll)
                .min(u16::MAX as usize) as u16,
        cursor_y: area.y
            + 1
            + line_index
                .saturating_sub(vertical_scroll)
                .min(u16::MAX as usize) as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(10, 5, 10, 5);

    #[test]
    fn cursor_uses_terminal_width_for_chinese_text() {
        let viewport = input_viewport("晚上好", AREA);
        assert_eq!(viewport.cursor_x, 17);
        assert_eq!(viewport.cursor_y, 6);
    }

    #[test]
    fn cursor_scrolls_with_long_and_multiline_input() {
        let horizontal = input_viewport("123456789", AREA);
        assert_eq!(horizontal.horizontal_scroll, 2);
        assert_eq!(horizontal.cursor_x, 18);

        let vertical = input_viewport("一\n二\n三\n四", AREA);
        assert_eq!(vertical.vertical_scroll, 1);
        assert_eq!(vertical.cursor_y, 8);
    }

    #[test]
    fn bubbles_use_prefixed_role_colors() {
        let mut system = Vec::new();
        push_bubble(&mut system, &Role::System, "已切换。", false);
        assert_eq!(system[0].to_string(), "╭─ > 系统 ");
        assert_eq!(system[0].spans[0].style.fg, Some(Color::Yellow));

        let mut assistant = Vec::new();
        push_bubble(&mut assistant, &Role::Assistant, "晚上好。", false);
        assert_eq!(assistant[0].to_string(), "╭─ > 小鞠 ");
        assert_eq!(assistant[0].spans[0].style.fg, Some(Color::Magenta));

        let mut user = Vec::new();
        push_bubble(&mut user, &Role::User, "你好。", false);
        assert_eq!(user[0].to_string(), "╭─ > 你 ");
    }
}
