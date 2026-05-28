use crossterm::event::{KeyCode, KeyEvent};

/// An app that exposes a text filter and a way to refresh derived state when the filter changes.
pub trait Filterable {
    fn filter_mut(&mut self) -> &mut String;
    fn set_filtering(&mut self, on: bool);
    /// Rebuild any derived state (e.g. displayed rows) from the current filter.
    fn rebuild(&mut self);
}

/// Generic handler for the filter-input view. Returns once filtering ends (Enter/Esc)
/// or after applying a character/backspace edit.
pub fn handle_filter<A: Filterable>(app: &mut A, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.set_filtering(false);
            app.filter_mut().clear();
            app.rebuild();
        }
        KeyCode::Enter => {
            app.set_filtering(false);
        }
        KeyCode::Backspace => {
            app.filter_mut().pop();
            app.rebuild();
        }
        KeyCode::Char(c) => {
            app.filter_mut().push(c);
            app.rebuild();
        }
        _ => {}
    }
}
