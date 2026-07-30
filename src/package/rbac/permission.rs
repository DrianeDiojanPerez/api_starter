#[derive(Debug, Clone)]
pub struct Permission {
    pub id: i32,
    pub name: String,
    pub resource: String,
    pub module_id: i32,
}
