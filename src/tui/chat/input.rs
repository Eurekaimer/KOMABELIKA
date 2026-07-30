use ratatui::layout::Rect;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InputViewport {
    pub(super) vertical_scroll: u16,
    pub(super) horizontal_scroll: u16,
    pub(super) cursor_x: u16,
    pub(super) cursor_y: u16,
}

pub(super) fn input_viewport(input: &str, area: Rect) -> InputViewport {
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
