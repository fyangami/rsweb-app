use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default, Hash)]
pub struct TokenUser {
    #[serde(rename = "user_id")]
    pub user_id: i64,
}
