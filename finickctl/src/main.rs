use clap::Parser;
use config::ty::App;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {}

fn main() {
    let _args = Args::parse();
    match ipc::send_command(App::Scan, &(), Some(|_: ()| {})) {
        Ok(_) => {},
        Err(e) => {
            eprintln!("Failed to connect to service. ({e:?})");
        }
    };
}
