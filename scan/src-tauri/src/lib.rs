#![allow(dead_code, non_upper_case_globals)]

use std::{cell::{LazyCell, OnceCell}, net::{IpAddr, Ipv4Addr, SocketAddr}};

use bidi::tarpc::{self, context};
use config::ty::App;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};

const IndexService: LazyCell<index::ty::IndexRPCClient> = LazyCell::new(|| index::client());
const Handle: OnceCell<AppHandle> = OnceCell::new();

#[derive(Clone)]
pub struct ResultListener(SocketAddr);

impl bidi::RecvRPC for ResultListener {
    async fn data(self, _: tarpc::context::Context, data: Value) -> () {
        if let Some(handle) = Handle.get() {
            let _ = handle.emit("search-result", data);
        }
    }
}

impl bidi::ConstructableServer for ResultListener {
    fn new(addr: SocketAddr) -> Self {
        Self(addr)
    }
}

#[tauri::command]
fn search(app: AppHandle, query: String) {
    let _ = Handle.set(app);
    let _ = IndexService.search(context::current(), query, addr());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![search])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ScanConfig {
    port: u16
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self { port: 3051 }
    }
}

pub fn addr() -> SocketAddr {
    let port =  config::get_config::<ScanConfig>(App::IndexService).unwrap().port;
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
 }