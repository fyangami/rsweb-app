use serde::{Deserialize, Serialize};
use validator::Validate;

pub const DEFAULT_PAGE_SIZE: u64 = 10;

pub const MAXIMUM_PAGE_SIZE: u64 = 50;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct Pager {
    #[serde(rename = "pageNum")]
    #[validate(range(min = 1, max = 65535))]
    pub page_num: u64,
    #[validate(range(min = 1, max = MAXIMUM_PAGE_SIZE))]
    #[serde(rename = "pageSize")]
    pub page_size: u64,
}

impl Default for Pager {
    fn default() -> Self {
        Self {
            page_num: 1,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

impl Pager {
    pub fn offset(&self) -> u64 {
        return (self.page_num - 1) * self.page_size;
    }
}
