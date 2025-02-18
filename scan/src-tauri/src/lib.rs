#![allow(dead_code)]

use config::ty::App;
use rayon::iter::{ParallelBridge, ParallelIterator};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, LazyLock, Mutex},
    time::SystemTime,
};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

const SEARCH: LazyLock<Arc<SearchManager>> = LazyLock::new(|| SearchManager::new());

#[tauri::command]
fn search(app: AppHandle, query: String) {
    SEARCH.search(query, |file, path| {
        app.emit("search-result", (file, path)).unwrap()
    });
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ScanConfig {
    ignore: Vec<String>,
    last_index: SystemTime,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            ignore: Default::default(),
            last_index: SystemTime::now(),
        }
    }
}

struct SearchManager {
    config: ScanConfig,
    pool: Arc<Mutex<Connection>>,
}

impl SearchManager {
    pub fn new() -> Arc<Self> {
        let connection =
            Connection::open(config::finick_root().join("scan.db").display().to_string())
                .expect("Failed to open database");

        connection
        .execute(
            "CREATE TABLE IF NOT EXISTS file_index (id INTEGER PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL)",
            [],
        )
        .expect("Failed to create table");

        let pool = Arc::new(Mutex::new(connection));
        let instance = Arc::new(Self {
            config: config::get_config(App::Scan).unwrap(),
            pool: pool.clone(),
        });

        std::thread::spawn({
            let instance = instance.clone();
            move || {
                mountpoints::mountpaths()
                    .expect("Failed to find mount paths")
                    .iter()
                    .for_each(|path| {
                        instance.index(path.to_str().unwrap(), pool.clone());
                    });

                let mut config = config::get_config::<ScanConfig>(App::Scan).unwrap();
                config.last_index = SystemTime::now();
                config::write_config(App::Scan, config).unwrap();
            }
        });

        instance
    }

    fn index(&self, root: &str, pool: Arc<Mutex<Connection>>) {
        WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .par_bridge()
            .filter_map(Result::ok)
            .filter(|e| {
                e.metadata()
                    .map(|m| m.accessed().unwrap_or(SystemTime::now()) > self.config.last_index)
                    .unwrap_or(false)
            })
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                !self.config.ignore.iter().any(|i| name.contains(i))
            })
            .for_each(|entry| {
                let path = entry.path();
                if path.is_file() {
                    let name = path.file_name().unwrap().to_string_lossy().to_string();
                    let path = path.to_string_lossy().to_string();

                    let conn = pool.lock().unwrap();
                    conn.execute(
                        "INSERT INTO FILE (name, path) VALUES (?1, ?2)",
                        params![name, path],
                    )
                    .unwrap();
                }
            });
    }

    fn search(&self, query: String, cb: impl Fn(String, String)) {
        let pattern = format!("%{}%", query);
        let conn = self.pool.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT name, path FROM file_index WHERE name LIKE ?1 or path LIKE ?1")
            .unwrap();

        let file_iter = stmt
            .query_map([pattern], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap();

        for file in file_iter {
            if let Ok((file, path)) = file {
                cb(file, path)
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    .invoke_handler(tauri::generate_handler![search])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
