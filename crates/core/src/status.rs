/// Transient status-line message. Set with `set`, decay it once per tick.
/// Default display duration is 20 ticks (~5 s at the 250ms tick rate used by both apps).
#[derive(Default)]
pub struct Status {
    msg: Option<String>,
    ticks: u32,
}

impl Status {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, msg: impl Into<String>) {
        self.msg = Some(msg.into());
        self.ticks = 20;
    }

    pub fn tick(&mut self) {
        if self.ticks > 0 {
            self.ticks -= 1;
            if self.ticks == 0 {
                self.msg = None;
            }
        }
    }

    pub fn current(&self) -> Option<&str> {
        self.msg.as_deref()
    }
}
