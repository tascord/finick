use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use regex::Regex;
use rusqlite::params;
use std::env::{self, var};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{mpsc, Arc};
use std::{path::PathBuf, time::SystemTime};

pub mod ty;
const MAX_DEPTH: usize = 5;

fn folders() -> Vec<PathBuf> {
    let mut folders = vec![Path::new(&var("HOME").unwrap()).to_path_buf()];
    if let Ok(path) = var("PATH") {
        for folder in env::split_paths(&path) {
            folders.push(folder);
        }
    }
    folders
}

fn ignore() -> Vec<Regex> {
    vec![
        Regex::new("node_modules/.+").unwrap(),
        Regex::new(r"/\..+").unwrap()
    ]
}

fn is_binary_file(path: &PathBuf) -> bool {
    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        return matches!(
            ext.as_str(),
            "exe" | "bin" | "o" | "dll" | "so" | "dat" | "class"
        );
    }
    false
}
pub fn index(dirs: Option<Vec<PathBuf>>, pool: Pool<SqliteConnectionManager>) {
    let folders = dirs.unwrap_or(folders());
    let ignore = Arc::new(ignore());
    let (tx, rx) = mpsc::channel::<(String, String, i64, usize, bool)>();

    std::thread::spawn(move || {
        let pool = pool.clone();
        loop {
            while let Ok((name, path, timestamp, depth, executable)) = rx.recv() {
                pool.get().unwrap().execute("INSERT INTO files (name, path, depth, last_accessed, executable) values (?1, ?2, ?3, ?4, ?5)", params![
                    name, path, depth, timestamp, executable
                ]).unwrap();
            }
        }
    });

    folders
        .into_iter()
        .for_each(|f| task(f.clone(), ignore.clone(), tx.clone(), 0));
}

fn task(
    dir: PathBuf,
    ignore: Arc<Vec<Regex>>,
    tx: mpsc::Sender<(String, String, i64, usize, bool)>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }

    if let Ok(dir) = dir.read_dir() {
        dir.filter_map(Result::ok).for_each(|file| {
            let path = file.path();
            let path_str = path.to_string_lossy().to_string();
            let name = file.file_name().to_string_lossy().to_string();

            let ft = file.file_type().unwrap();
            if ft.is_dir() && !ignore.iter().any(|pat| pat.is_match(&path_str)) {
                task(file.path(), ignore.clone(), tx.clone(), depth + 1);
            } else if ft.is_file() && !is_binary_file(&path) {
                let timestamp = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                let executable = is_executable(&path);
                let _ = tx.send((name, path_str, timestamp, depth, executable));
            }
        });
    }
}

fn is_executable(path: &PathBuf) -> bool {
    if let Ok(metadata) = fs::metadata(path) {
        let permissions = metadata.permissions();
        permissions.mode() & 0o111 != 0
    } else {
        false
    }
}

pub fn search(query: &str, pool: Pool<SqliteConnectionManager>, cb: impl Fn(String, String)) {
    let query = format!("%{query}%");
    let binding = pool.get().unwrap();
    let mut res = binding
        .prepare(
            "SELECT name, path
            FROM files
            WHERE name LIKE ?1 or
            path LIKE ?1
            ORDER BY executable DESC, last_accessed DESC, depth ASC, LENGTH(replace(name, ?1, '')) ASC
            LIMIT 100",
        )
        .unwrap();

    let mut rows = res.query(params![query]).unwrap();
    while let Some(row) = rows.next().unwrap() {
        let name: String = row.get(0).unwrap();
        let path: String = row.get(1).unwrap();

        cb(name, path);
    }
}

pub fn watch(pool: Pool<SqliteConnectionManager>) {
    let (tx, rx) = mpsc::channel(); // Use std::sync::mpsc::channel

    let mut watcher: RecommendedWatcher = Watcher::new(tx, Config::default()).unwrap();

    for folder in folders() {
        let _ = watcher.watch(&folder, RecursiveMode::Recursive);
    }

    std::thread::spawn(move || loop {
        match rx.recv() {
            Ok(Ok(event)) => match event.kind {
                EventKind::Create(_) => {
                    index(Some(event.paths), pool.clone());
                }
                EventKind::Remove(_) => {
                    event
                        .paths
                        .iter()
                        .map(|p| format!("{}%", p.display()))
                        .for_each(|param| {
                            pool.get()
                                .unwrap()
                                .execute("DELETE FROM files WHERE path LIKE ?1", params![param])
                                .unwrap();
                        });
                }
                _ => {}
            },
            _ => {}
        }
    });
}
