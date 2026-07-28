#[derive(Debug, Clone)]
pub struct Server {
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct Deployment {
    pub name: String,
    pub environment: String,
    pub time_zone: String,
}

impl Deployment {
    pub fn is_production(&self) -> bool {
        self.environment.eq_ignore_ascii_case("production")
    }
}
