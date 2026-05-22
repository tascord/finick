use serde::{Deserialize, Serialize};

#[derive(strum::Display, strum::EnumString, Serialize, Deserialize, Debug, Clone)]
pub enum App {
    Scan,
    Files
    IndexService,
    Other(String),
}