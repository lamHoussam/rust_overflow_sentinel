

// TODO: Replace var_type with sizeof_type
#[derive(Debug)]
pub struct Variable {
    pub name: String,
    pub size: usize,
    pub var_type: String,
    pub max_bounds_checked: usize,
}