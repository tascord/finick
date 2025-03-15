use clap::Parser;
use config::ty::App;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, help = "Program to command")]
    program: Program,
}

#[allow(non_camel_case_types)]
#[derive(strum::Display, strum::EnumString, Clone, Debug)]
enum Program {
    scan,
    index(String),
}

fn main() {
    let args = Args::parse();
    match args.program {
        Program::scan => {
            match ipc::send_command(App::Scan, &(), Some(|_: ()| {})) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Failed to connect to service. ({e:?})");
                }
            };
        }
        Program::index(q) => {
            ipc::send_command(
                App::IndexService,
                &index::ty::Request { query: q },
                Some(move |value: index::ty::SearchResult| {
                    println!(
                        "{ic}{}\t{}",
                        value.name,
                        value.path,
                        ic = {
                            if value.is_desktop {
                                "@"
                            } else if value.is_executable {
                                "*"
                            } else {
                                ""
                            }
                        }
                    );
                }),
            )
            .expect("Failed to connect to index");

            println!("Done")
        }
    }
}
