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
/// If a file was indexed less than this many seconds ago, skip re-indexing.
const REINDEX_THRESHOLD: i64 = 60;

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
        Regex::new(r"/\..+").unwrap(),
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
    // Use a sync_channel with a fixed capacity to prevent unbounded memory usage.
    let (tx, rx) = mpsc::sync_channel::<(String, String, i64, usize, bool)>(1000);

    // Spawn a thread to process file entries.
    std::thread::spawn(move || {
        let pool = pool.clone();
        while let Ok((name, path, timestamp, depth, executable)) = rx.recv() {
            let conn = pool.get().unwrap();
            // Check if the file already exists and if it was indexed recently.
            let should_index = match conn.query_row(
                "SELECT last_accessed FROM files WHERE path = ?1",
                params![&path],
                |row| row.get::<_, i64>(0),
            ) {
                Ok(last_accessed) => {
                    // Only re-index if the stored timestamp is older than our threshold.
                    timestamp >= last_accessed + REINDEX_THRESHOLD
                }
                Err(_) => true, // File not indexed yet.
            };

            if should_index {
                // Using REPLACE here so that we update if it already exists.
                conn.execute(
                    "REPLACE INTO files (name, path, depth, last_accessed, executable) values (?1, ?2, ?3, ?4, ?5)",
                    params![name, path, depth, timestamp, executable],
                )
                .unwrap();
            }
        }
    });

    folders.into_iter().for_each(|f| task(f, ignore.clone(), tx.clone(), 0));
}

fn task(
    dir: PathBuf,
    ignore: Arc<Vec<Regex>>,
    tx: mpsc::SyncSender<(String, String, i64, usize, bool)>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    if let Ok(entries) = dir.read_dir() {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();
            let name = entry.file_name().to_string_lossy().to_string();
            let ft = entry.file_type().unwrap();
            if ft.is_dir() && !ignore.iter().any(|pat| pat.is_match(&path_str)) {
                task(path, ignore.clone(), tx.clone(), depth + 1);
            } else if ft.is_file() && !is_binary_file(&path) {
                let timestamp = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                let executable = is_executable(&path);
                // Send the file information. If the channel is full, this will block.
                let _ = tx.send((name, path_str, timestamp, depth, executable));
            } else if ft.is_symlink() {
                if let Ok(target) = fs::read_link(&path) {
                    if is_executable(&target) {
                        let timestamp = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64;
                        let executable = true;
                        // Send the symlink information. If the channel is full, this will block.
                        let _ = tx.send((name, path_str, timestamp, depth, executable));
                    }
                }
            }
        }
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
    let conn = pool.get().unwrap();
    let mut res = conn
        .prepare(
            "SELECT name, path
             FROM files
             WHERE name LIKE ?1 OR path LIKE ?1
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
    let (tx, rx) = mpsc::channel(); // std::sync::mpsc::channel

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
                    event.paths.iter().for_each(|p| {
                        let param = format!("{}%", p.display());
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
