use ratatui::{layout::Rect, style::Color, text::Line};
use unicode_width::UnicodeWidthStr;

use super::{bubble::push_bubble, conversation_scroll, input::input_viewport};
use crate::provider::Role;

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
fn history_scroll_is_measured_from_the_bottom() {
    let area = Rect::new(0, 0, 12, 6);
    let lines = (0..10)
        .map(|index| Line::from(format!("line {index}")))
        .collect::<Vec<_>>();
    assert_eq!(conversation_scroll(&lines, area, 0), 6);
    assert_eq!(conversation_scroll(&lines, area, 2), 4);
    assert_eq!(conversation_scroll(&lines, area, u16::MAX), 0);
}

#[test]
fn rounded_bubbles_have_complete_borders_without_background_bands() {
    let mut system = Vec::new();
    push_bubble(&mut system, &Role::System, "已切换。", false, 20);
    assert!(system[0].to_string().starts_with("╭─ > 系统 "));
    assert!(system[0].to_string().ends_with('╮'));
    assert_eq!(UnicodeWidthStr::width(system[0].to_string().as_str()), 20);
    assert_eq!(system[0].spans[0].style.fg, Some(Color::Yellow));
    assert_eq!(system[1].spans[0].style.bg, None);
    assert_eq!(system[1].spans[1].style.bg, None);
    assert_eq!(system[2].to_string(), "╰──────────────────╯");

    let mut assistant = Vec::new();
    push_bubble(&mut assistant, &Role::Assistant, "晚上好。", false, 20);
    assert!(assistant[0].to_string().starts_with("╭─ > 小鞠 "));
    assert_eq!(assistant[0].spans[0].style.fg, Some(Color::Magenta));

    let mut user = Vec::new();
    push_bubble(&mut user, &Role::User, "你好。", false, 20);
    assert!(user[0].to_string().starts_with("╭─ > 你 "));
}

#[test]
fn bubble_content_wraps_to_rounded_box_width() {
    let mut lines = Vec::new();
    push_bubble(&mut lines, &Role::Assistant, "一二三四五六", false, 12);
    assert_eq!(lines[1].to_string(), "│ 一二三四 │");
    assert_eq!(lines[2].to_string(), "│ 五六     │");
    assert!(
        lines[..4]
            .iter()
            .all(|line| UnicodeWidthStr::width(line.to_string().as_str()) == 12)
    );
}
