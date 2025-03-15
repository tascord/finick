use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Request {
    pub query: String,
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchResult {
    pub name: String,
    pub path: String,
    pub is_desktop: bool,
    pub is_executable: bool,
    pub icon: Option<String>,
}
