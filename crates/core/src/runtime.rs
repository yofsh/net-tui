use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEvent, MouseButton, MouseEventKind,
};
use ratatui::{DefaultTerminal, Frame};

/// Tick rate used by the shared event loop. Matches the original 250ms cadence.
pub const TICK_RATE: Duration = Duration::from_millis(250);

/// Contract a TUI app must satisfy to be driven by `run`.
pub trait TuiApp {
    fn draw(&mut self, frame: &mut Frame);
    fn tick(&mut self);
    fn handle_key(&mut self, key: KeyEvent);
    fn handle_scroll_up(&mut self);
    fn handle_scroll_down(&mut self);
    /// Left mouse click. `row`/`col` are the cell coordinates of the click.
    /// `term_width`/`term_height` are the terminal dimensions so the impl can
    /// locate the (possibly multi-row) hotbar at the bottom of the screen.
    fn handle_left_click(&mut self, row: u16, col: u16, term_width: u16, term_height: u16);
    fn should_quit(&self) -> bool;
    /// Called once on a clean exit (after the loop quits).
    fn cleanup(&mut self) {}
}

/// Install the standard panic hook, enable mouse capture, then drive the event loop
/// until the app reports `should_quit`. Restores the terminal on the way out.
pub fn run<A: TuiApp>(app: &mut A) -> io::Result<()> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        ratatui::restore();
        original_hook(panic);
    }));

    crossterm::execute!(std::io::stdout(), EnableMouseCapture).ok();
    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, app);
    ratatui::restore();
    crossterm::execute!(std::io::stdout(), DisableMouseCapture).ok();
    result
}

fn run_loop<A: TuiApp>(terminal: &mut DefaultTerminal, app: &mut A) -> io::Result<()> {
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| app.draw(f))?;

        let timeout = TICK_RATE.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            loop {
                match event::read()? {
                    Event::Key(key) => app.handle_key(key),
                    Event::Mouse(mouse) => match mouse.kind {
                        MouseEventKind::ScrollUp => app.handle_scroll_up(),
                        MouseEventKind::ScrollDown => app.handle_scroll_down(),
                        MouseEventKind::Down(MouseButton::Left) => {
                            let size = terminal.size()?;
                            app.handle_left_click(mouse.row, mouse.column, size.width, size.height);
                        }
                        _ => {}
                    },
                    _ => {}
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }

        if last_tick.elapsed() >= TICK_RATE {
            app.tick();
            last_tick = Instant::now();
        }

        if app.should_quit() {
            app.cleanup();
            return Ok(());
        }
    }
}
