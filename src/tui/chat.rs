use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::provider::Role;

pub struct VisibleMessage<'a> {
    pub role: &'a Role,
    pub content: &'a str,
    pub interrupted: bool,
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
        let (name, color) = match message.role {
            Role::User => ("你", Color::Cyan),
            Role::Assistant => ("小鞠", Color::Magenta),
            Role::System => ("系统", Color::DarkGray),
        };
        let interrupted = if message.interrupted {
            "（已中断）"
        } else {
            ""
        };
        lines.push(Line::from(Span::styled(
            format!("{name}{interrupted}"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )));
        lines.extend(message.content.lines().map(Line::from));
        lines.push(Line::default());
    }
    if !view.reasoning_text.is_empty() {
        lines.push(Line::from(Span::styled(
            "思考",
            Style::default().fg(Color::DarkGray),
        )));
        lines.extend(
            view.reasoning_text
                .lines()
                .map(|line| Line::styled(line, Style::default().fg(Color::DarkGray))),
        );
        lines.push(Line::default());
    }
    if !view.streaming_text.is_empty() {
        lines.push(Line::from(Span::styled(
            "小鞠",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )));
        lines.extend(view.streaming_text.lines().map(Line::from));
    }
    if lines.is_empty() {
        lines.push(Line::from("……晚上好。想说什么都可以，不必组织得很完整。"));
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
                "Enter 发送  Esc 取消  Ctrl+N 新会话  Ctrl+P 切换 Provider  /help 命令  Ctrl+C 退出  tokens {}→{}",
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

    if !view.generating {
        frame.set_cursor_position((input_viewport.cursor_x, input_viewport.cursor_y));
    }
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
}
