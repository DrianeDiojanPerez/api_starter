#[derive(Debug, Clone)]
pub struct Permission {
    pub id: i32,
    pub name: String,
    pub resource: String,
    pub module: String,
}

#[derive(Debug, Clone)]
pub struct AddPermission {
    pub name: String,
    pub resource: String,
    pub module: String,
}

#[derive(Debug, Clone)]
pub struct RemovePermission {
    pub name: String,
    pub resource: String,
    pub module: String,
}
