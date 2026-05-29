use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::theme::{CYAN, GRAY, GREEN};

/// Render one hotkey as `[ key ] desc`. When `active` is true (e.g. a toggle
/// that is currently on, like Power or Scan), the key chip and label turn green
/// instead of the usual cyan/gray.
pub fn hotkey_spans<'a>(key: &str, desc: &str, active: bool) -> Vec<Span<'a>> {
    let chip_bg = if active { GREEN } else { CYAN };
    let desc_fg = if active { GREEN } else { GRAY };

    let is_single = key.len() == 1;
    let key_char = key.chars().next();
    let desc_char = desc.chars().next();
    let first_char_matches = is_single
        && key_char
            .zip(desc_char)
            .map(|(k, d)| k.to_ascii_uppercase() == d.to_ascii_uppercase())
            .unwrap_or(false);

    if first_char_matches {
        let (_, tail) = desc.split_at(1);
        vec![
            Span::styled(
                format!(" {key} "),
                Style::default().fg(Color::Black).bg(chip_bg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{tail} "), Style::default().fg(desc_fg)),
        ]
    } else {
        vec![
            Span::styled(
                format!(" {key} "),
                Style::default().fg(Color::Black).bg(chip_bg),
            ),
            Span::styled(format!(" {desc} "), Style::default().fg(desc_fg)),
        ]
    }
}

pub fn hotkey_rendered_width(key: &str, desc: &str) -> u16 {
    let is_single = key.len() == 1;
    let first_match = is_single
        && key.chars().next()
            .zip(desc.chars().next())
            .map(|(k, d)| k.to_ascii_uppercase() == d.to_ascii_uppercase())
            .unwrap_or(false);

    if first_match {
        (3 + desc.chars().count()) as u16
    } else {
        (key.chars().count() + 2 + desc.chars().count() + 2) as u16
    }
}

/// A bottom-bar hotkey: shortcut, label, and whether it's in an "active" (on)
/// state that should render highlighted (see [`hotkey_spans`]).
pub type Hotkey<'a> = (&'a str, &'a str, bool);

/// Pack hotkey items into as many rows as needed so each row fits within `width`
/// columns. Returns one `Line` per row, ready to render as a multi-row hotbar.
pub fn layout_hotkeys<'a>(items: &[Hotkey<'a>], width: u16) -> Vec<Line<'a>> {
    let mut lines: Vec<Line> = Vec::new();
    let mut cur: Vec<Span> = Vec::new();
    let mut cur_w: u16 = 0;
    for (key, desc, active) in items {
        let w = hotkey_rendered_width(key, desc);
        if cur_w + w > width && !cur.is_empty() {
            lines.push(Line::from(std::mem::take(&mut cur)));
            cur_w = 0;
        }
        cur.extend(hotkey_spans(key, desc, *active));
        cur_w += w;
    }
    if !cur.is_empty() {
        lines.push(Line::from(cur));
    }
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines
}

/// Number of rows [`layout_hotkeys`] would produce for `width`.
pub fn rows_needed(items: &[Hotkey], width: u16) -> u16 {
    if width == 0 {
        return 1;
    }
    let mut rows: u16 = 1;
    let mut cur_w: u16 = 0;
    for (key, desc, _) in items {
        let w = hotkey_rendered_width(key, desc);
        if cur_w + w > width && cur_w > 0 {
            rows += 1;
            cur_w = 0;
        }
        cur_w += w;
    }
    rows
}

/// Map a click at (`line`, `x`) within a wrapped hotbar (see [`layout_hotkeys`])
/// to the item index, or `None` if the click misses every item.
pub fn find_clicked_item_wrapped(items: &[Hotkey], width: u16, line: u16, x: u16) -> Option<usize> {
    let mut row: u16 = 0;
    let mut pos: u16 = 0;
    for (i, (key, desc, _)) in items.iter().enumerate() {
        let w = hotkey_rendered_width(key, desc);
        if pos + w > width && pos > 0 {
            row += 1;
            pos = 0;
        }
        if row == line && x >= pos && x < pos + w {
            return Some(i);
        }
        pos += w;
    }
    None
}
