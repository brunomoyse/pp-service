#[derive(Debug, Clone, Copy)]
pub struct LimitOffset {
    pub limit: i64,
    pub offset: i64,
}

impl Default for LimitOffset {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
        }
    }
}
