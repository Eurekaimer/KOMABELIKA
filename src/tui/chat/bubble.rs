use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::provider::Role;

pub(super) fn push_bubble(
    lines: &mut Vec<Line<'static>>,
    role: &Role,
    content: &str,
    interrupted: bool,
    width: usize,
) {
    let (name, color) = match role {
        Role::User => ("你", Color::Cyan),
        Role::Assistant => ("小鞠", Color::Magenta),
        Role::System => ("系统", Color::Yellow),
    };
    let interrupted = if interrupted { "（已中断）" } else { "" };
    let accent_style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    let width = width.max(8);
    let header = format!("╭─ > {name}{interrupted} ");
    let header_fill = width.saturating_sub(UnicodeWidthStr::width(header.as_str()) + 1);
    lines.push(Line::from(Span::styled(
        format!("{header}{}╮", "─".repeat(header_fill)),
        accent_style,
    )));
    let content_width = width.saturating_sub(4).max(1);
    for content_line in wrap_content(content, content_width) {
        let padding = content_width.saturating_sub(UnicodeWidthStr::width(content_line.as_str()));
        lines.push(Line::from(vec![
            Span::styled("│ ", accent_style),
            Span::styled(content_line, Style::default().fg(Color::White)),
            Span::raw(" ".repeat(padding)),
            Span::styled(" │", accent_style),
        ]));
    }
    lines.push(Line::from(Span::styled(
        format!("╰{}╯", "─".repeat(width.saturating_sub(2))),
        accent_style,
    )));
    lines.push(Line::default());
}

fn wrap_content(content: &str, width: usize) -> Vec<String> {
    let mut wrapped = Vec::new();
    for source_line in content.split('\n') {
        let mut line = String::new();
        let mut line_width = 0;
        for character in source_line.chars() {
            let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
            if line_width > 0 && line_width + character_width > width {
                wrapped.push(std::mem::take(&mut line));
                line_width = 0;
            }
            line.push(character);
            line_width += character_width;
        }
        wrapped.push(line);
    }
    wrapped
}
