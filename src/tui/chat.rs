use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

mod bubble;
mod input;
mod overlay;
#[cfg(test)]
mod tests;

use bubble::push_bubble;
use input::input_viewport;
use overlay::{render_selection, render_slash_suggestions};

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
    pub history_focused: bool,
    pub history_scroll: u16,
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
    let bubble_width = chunks[1].width.saturating_sub(2).max(8) as usize;
    for message in view.messages {
        push_bubble(
            &mut lines,
            message.role,
            message.content,
            message.interrupted,
            bubble_width,
        );
    }
    if !view.reasoning_text.is_empty() {
        push_bubble(
            &mut lines,
            &Role::System,
            view.reasoning_text,
            false,
            bubble_width,
        );
    }
    if !view.streaming_text.is_empty() {
        push_bubble(
            &mut lines,
            &Role::Assistant,
            view.streaming_text,
            false,
            bubble_width,
        );
    }
    if lines.is_empty() {
        push_bubble(
            &mut lines,
            &Role::Assistant,
            "……晚上好。想说什么都可以，不必组织得很完整。",
            false,
            bubble_width,
        );
    }
    let scroll = conversation_scroll(&lines, chunks[1], view.history_scroll);
    let conversation_title = if view.history_focused {
        " 对话 · 浏览模式 "
    } else {
        " 对话 "
    };
    let conversation_border = if view.history_focused {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let conversation = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .title(conversation_title)
                .borders(Borders::ALL)
                .border_style(conversation_border),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(conversation, chunks[1]);

    let input_viewport = input_viewport(view.input, chunks[2]);
    let input_border = if view.history_focused {
        Style::default()
    } else {
        Style::default().fg(Color::Cyan)
    };
    let input = Paragraph::new(view.input)
        .block(
            Block::default()
                .title(" 输入 ")
                .borders(Borders::ALL)
                .border_style(input_border),
        )
        .scroll((
            input_viewport.vertical_scroll,
            input_viewport.horizontal_scroll,
        ));
    frame.render_widget(input, chunks[2]);

    let footer_text = view.error.map_or_else(

        || {
            format!(
                "t 切换输入/历史  历史区 j/k 滚动  Enter 发送  Esc 取消  Ctrl+P 模型  Ctrl+C 退出  tokens {}→{}",
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

    if !view.generating && view.model_picker.is_none() && !view.history_focused {
        frame.set_cursor_position((input_viewport.cursor_x, input_viewport.cursor_y));
    }
}

fn conversation_scroll(lines: &[Line<'_>], area: Rect, rows_above_bottom: u16) -> u16 {
    let inner_width = area.width.saturating_sub(2).max(1) as usize;
    let visible_height = area.height.saturating_sub(2) as usize;
    let wrapped_rows = lines
        .iter()
        .map(|line| line.width().max(1).div_ceil(inner_width))
        .sum::<usize>();
    let max_scroll = wrapped_rows
        .saturating_sub(visible_height)
        .min(u16::MAX as usize) as u16;
    max_scroll.saturating_sub(rows_above_bottom.min(max_scroll))
}
