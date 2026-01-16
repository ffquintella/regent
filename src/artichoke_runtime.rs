// Artichoke Runtime Configuration
// Handles Ruby/Rust interoperability through Artichoke
// Note: This is a Phase 4 stub, will be fully implemented later

#[allow(dead_code)]
pub struct ArtichokeConfig {
    pub enable_stdlib: bool,
    pub enable_gems: bool,
}

impl Default for ArtichokeConfig {
    fn default() -> Self {
        Self {
            enable_stdlib: true,
            enable_gems: true,
        }
    }
}

#[allow(dead_code)]
impl ArtichokeConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_stdlib(mut self, enable: bool) -> Self {
        self.enable_stdlib = enable;
        self
    }

    pub fn with_gems(mut self, enable: bool) -> Self {
        self.enable_gems = enable;
        self
    }
}
