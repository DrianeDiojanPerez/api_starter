#[derive(Debug, Clone)]
pub struct Logger {
    /// Accepts either a tracing level name (`debug`) or the numeric slog style
    /// level the Go service used (`-4`, `0`, `4`, `8`).
    pub level: String,
    pub directory: String,
}

impl Logger {
    pub fn directive(&self) -> &str {
        match self.level.trim() {
            "-8" => "trace",
            "-4" => "debug",
            "0" => "info",
            "4" => "warn",
            "8" => "error",
            named => named,
        }
    }
}
