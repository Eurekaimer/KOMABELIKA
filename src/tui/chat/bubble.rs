use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::BorderStyle;
use crate::provider::Role;

pub(super) fn push_bubble(
    lines: &mut Vec<Line<'static>>,
    role: &Role,
    content: &str,
    interrupted: bool,
    width: usize,
    border_style: BorderStyle,
    bold_text: bool,
) {
    let (name, color) = match role {
        Role::User => ("你", Color::Cyan),
        Role::Assistant => ("小鞠", Color::Magenta),
        Role::System => ("系统", Color::Yellow),
    };
    let interrupted = if interrupted { "（已中断）" } else { "" };
    let accent_style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    let glyphs = BorderGlyphs::for_style(border_style);
    let width = width.max(8);
    let header = format!(
        "{}{} > {name}{interrupted} ",
        glyphs.top_left, glyphs.horizontal
    );
    let header_fill = width.saturating_sub(UnicodeWidthStr::width(header.as_str()) + 1);
    lines.push(Line::from(Span::styled(
        format!(
            "{header}{}{}",
            glyphs.horizontal.to_string().repeat(header_fill),
            glyphs.top_right
        ),
        accent_style,
    )));
    let content_width = width.saturating_sub(4).max(1);
    for content_line in wrap_content(content, content_width) {
        let padding = content_width.saturating_sub(UnicodeWidthStr::width(content_line.as_str()));
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", glyphs.vertical), accent_style),
            Span::styled(content_line, text_style(bold_text)),
            Span::raw(" ".repeat(padding)),
            Span::styled(format!(" {}", glyphs.vertical), accent_style),
        ]));
    }
    lines.push(Line::from(Span::styled(
        format!(
            "{}{}{}",
            glyphs.bottom_left,
            glyphs
                .horizontal
                .to_string()
                .repeat(width.saturating_sub(2)),
            glyphs.bottom_right
        ),
        accent_style,
    )));
    lines.push(Line::default());
}

fn text_style(bold: bool) -> Style {
    let style = Style::default().fg(Color::Green);
    if bold {
        style.add_modifier(Modifier::BOLD)
    } else {
        style
    }
}

struct BorderGlyphs {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    vertical: char,
}

impl BorderGlyphs {
    fn for_style(style: BorderStyle) -> Self {
        let glyphs = match style {
            BorderStyle::Plain => ('┌', '┐', '└', '┘', '─', '│'),
            BorderStyle::Rounded => ('╭', '╮', '╰', '╯', '─', '│'),
            BorderStyle::Double => ('╔', '╗', '╚', '╝', '═', '║'),
            BorderStyle::Thick => ('┏', '┓', '┗', '┛', '━', '┃'),
        };
        Self {
            top_left: glyphs.0,
            top_right: glyphs.1,
            bottom_left: glyphs.2,
            bottom_right: glyphs.3,
            horizontal: glyphs.4,
            vertical: glyphs.5,
        }
    }
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
