use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Request {
    pub query: String
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Response {
    pub name: String,
    pub path: String
}
