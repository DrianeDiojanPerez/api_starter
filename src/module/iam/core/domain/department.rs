use super::Company;

#[derive(Debug, Clone, Default)]
pub struct Department {
    pub id: i32,
    pub name: String,
    pub company: Company,
}
