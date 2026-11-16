use expression::angles::AngleMode;
pub struct Settings {
    pub tokens: bool,
    pub angle: AngleMode,
}

impl Settings {
    pub fn new() -> Self {
        Settings {
            tokens: false,               // off by default
            angle: AngleMode::default(), // Degrees by default
        }
    }
}
