#[derive(strum::Display)]
pub enum App {
    Scan,
    Other(String)
}