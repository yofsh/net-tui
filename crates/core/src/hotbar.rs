use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use crate::theme::{CYAN, GRAY};

pub fn hotkey_spans<'a>(key: &str, desc: &str) -> Vec<Span<'a>> {
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
                Style::default().fg(Color::Black).bg(CYAN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{tail} "), Style::default().fg(GRAY)),
        ]
    } else {
        vec![
            Span::styled(
                format!(" {key} "),
                Style::default().fg(Color::Black).bg(CYAN),
            ),
            Span::styled(format!(" {desc} "), Style::default().fg(GRAY)),
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

/// Given a row of hotbar items `(key, desc)` and a click x-coordinate,
/// return the index of the item that contains that column, or `None` if past the end.
pub fn find_clicked_item(items: &[(&str, &str)], x: u16) -> Option<usize> {
    let mut pos: u16 = 0;
    for (i, (key, desc)) in items.iter().enumerate() {
        let w = hotkey_rendered_width(key, desc);
        if x >= pos && x < pos + w {
            return Some(i);
        }
        pos += w;
    }
    None
}
