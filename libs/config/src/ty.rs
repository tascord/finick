#[derive(strum::Display)]
pub enum App {
    Scan,
    IndexService,
    Other(String)
}