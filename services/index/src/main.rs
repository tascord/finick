use std::{sync::mpsc::Sender, thread};

use config::ty::App;
use index::ty::{Request, SearchResult};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let manager = SqliteConnectionManager::file(config::finick_root().join("index.db"));
    let pool = Pool::new(manager).unwrap();
    pool.get().unwrap().execute_batch("PRAGMA journal_mode = WAL;").unwrap();
 
    pool.get().unwrap().execute("CREATE TABLE IF NOT EXISTS files (
        name TEXT NOT NULL,
        path TEXT PRIMARY KEY,
        depth INTEGER NOT NULL,
        executable BOOL NOT NULL,
        desktop BOOL NOT NULL,
        icon TEXT,
        last_accessed INTEGER NOT NULL
    )", params![]).unwrap();

    thread::spawn({ let pool = pool.clone(); move || index::watch(pool.clone())});
    thread::spawn({ let pool = pool.clone(); move || index::index(None, pool.clone())});
    ipsea::start_server(App::IndexService, {
        let pool = pool.clone();
        move |t: Request, sender: Sender<SearchResult>| {
            println!("Searching for {}", &t.query);
            if t.query.len() > 2 { index::search(&t.query, pool.clone(), |v| { let _ = sender.send(v); }) };
        }
    }).expect("Failed to start index service");
}