use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::SelectionView;
use crate::tui::slash::SlashCommand;

pub(super) fn render_slash_suggestions(
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
                .title(" 命令候选 · Enter 执行 · Tab 补全 · ↑↓选择 ")
                .title_style(Style::default().fg(Color::Yellow))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        area,
    );
}

pub(super) fn render_selection(frame: &mut Frame<'_>, outer: Rect, picker: &SelectionView<'_>) {
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
