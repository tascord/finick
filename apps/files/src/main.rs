#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::{env, path::PathBuf};

use config::ty::App;
use freya::prelude::*;
use freya::elements::rect::rect;
use index::ty::SearchResult;

enum ItemType {
    File,
    Folder
}

struct Item {
    ty: ItemType,
    name: String,
    path: String
}

impl From<SearchResult> for Item {
    fn from(value: SearchResult) -> Self {
        Item {
            ty: ItemType::File,
            name: value.name,
            path: value.path,
        }
    }
}

fn fetch(p: PathBuf, s: &mut State<Vec<Item>>) {
    let mut w = s.write();
    w.clear();

    let _ = ipsea::send_command(
        App::IndexService,
        &index::ty::Request { query: p.to_str().unwrap_or_default().to_owned() },
        Some(move |value: index::ty::SearchResult| {
            w.push(value.into());
    }));
}

fn app() -> impl IntoElement {
    let mut fr = use_state(|| 0.3);
    let mut items = use_state(Vec::<Item>::new);
    let mut p = use_state(env::home_dir().map(|v| v.to_str().unwrap().to_string()).unwrap_or("/".to_string()));

    let top_bar = rect()
    .height(Size::px(90.0))
    .width(Size::fill())
    .child(rect().child("Left").height(Size::fill()).width(Size::percent(*fr.read())))
    .child(rect().child("Right").height(Size::fill()).width(Size::percent(1.0 - *fr.read())));

    let content = rect()
    .width(Size::fill())
    .height(Size::fill())
    .child(rect().background(Color::BLACK).child("Left").height(Size::fill()).width(Size::percent(*fr.read())))
    .child(rect().background(Color::BLACK).child("Right").height(Size::fill()).width(Size::percent(1.0 - *fr.read())));

    rect()
        // .width(Size::fill())
        // .height(Size::fill())
        .vertical()
        .child(top_bar)
        .child(content)
}

fn main() {
    launch(LaunchConfig::new().with_window(WindowConfig::new(app).with_title("Files")))
}