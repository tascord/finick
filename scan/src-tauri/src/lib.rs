#![allow(dead_code, non_upper_case_globals)]

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::Command,
    sync::RwLock,
};

use config::ty::App;
use gdk::WMDecoration;
use gtk::prelude::*;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

const CURRENT_SEARCH_ID: RwLock<String> = RwLock::new(String::new());

#[tauri::command]
fn search(app: AppHandle, query: String) {
    *CURRENT_SEARCH_ID.write().unwrap() = query.clone();
    std::thread::spawn(|| {
        ipc::send_command(
            App::IndexService,
            &index::ty::Request {
                query: query.clone(),
            },
            Some(move |value: index::ty::Response| {
                if !CURRENT_SEARCH_ID.read().unwrap().eq(&query) {
                    let _ = app.emit("search-result", (value.name, value.path));
                }
            }),
        )
    });
}

#[tauri::command]
fn open(path: String) {
    let mut command = if Path::new(&path)
        .metadata()
        .map(|p| p.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
    {
        Command::new(path)
    } else {
        Command::new("xdg-open")
    };

    command.spawn().unwrap();
}

#[tauri::command]
fn close(app: AppHandle) {
    app.webview_windows()
        .values()
        .next()
        .unwrap()
        .clone()
        .hide()
        .unwrap();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            pretty_env_logger::init();
            let binding = app.webview_windows();
            let win = binding.values().next().unwrap();

            let gtk = win.gtk_window().unwrap();
            let window = gtk.window().unwrap();
            window.set_decorations(WMDecoration::empty());
            window.set_type_hint(gdk::WindowTypeHint::Dialog);
            win.center().unwrap();

            std::thread::spawn({
                let win = win.clone();
                let _ = win.clone().hide();
                move || {
                    ipc::start_server(App::Scan, {
                        let win = win.clone();
                        move |_: (), _: std::sync::mpsc::Sender<()>| {
                            let gtk = win.gtk_window().unwrap();
                            let window = gtk.window().unwrap();

                            win.center().unwrap();
                            win.clone().show().unwrap();
                            win.center().unwrap();

                            window.show();
                            window.set_decorations(WMDecoration::empty());
                            window.set_type_hint(gdk::WindowTypeHint::Dialog);
                            window.set_urgency_hint(true);
                            window.raise();
                            win.set_focus().unwrap();
                        }
                    })
                    .unwrap();
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                window.hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![search, open, close])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ScanConfig {
    port: u16,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self { port: 3051 }
    }
}

pub fn addr() -> SocketAddr {
    let port = config::get_config::<ScanConfig>(App::IndexService)
        .unwrap()
        .port;
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
}
