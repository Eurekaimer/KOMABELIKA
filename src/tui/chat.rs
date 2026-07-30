use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

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

    let input = Paragraph::new(view.input)
        .block(Block::default().title(" 输入 ").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(input, chunks[2]);

    let footer_text = view.error.map_or_else(
        || {
            format!(
                "Enter 发送  Shift+Enter 换行  Esc 取消  Ctrl+N 新会话  Ctrl+L 切换  Ctrl+C 退出  tokens {}→{}",
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
        let last_line = view.input.rsplit('\n').next().unwrap_or_default();
        let input_line = view.input.matches('\n').count() as u16;
        let max_x = chunks[2].right().saturating_sub(2);
        let max_y = chunks[2].bottom().saturating_sub(2);
        frame.set_cursor_position((
            (chunks[2].x + 1 + last_line.chars().count() as u16).min(max_x),
            (chunks[2].y + 1 + input_line).min(max_y),
        ));
    }
}
