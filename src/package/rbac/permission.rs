/// One action a user holds, checked as `resource.name`.
#[derive(Debug, Clone)]
pub struct Permission {
    pub resource: String,
    pub name: String,
}
