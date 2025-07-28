use clap::Parser;
use config::ty::App;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, help = "Program to command")]
    program: Program,
    #[arg(short, help = "Output in JSON format", default_value = "false")]
    json: bool,
    #[arg(name = "DATA", help = "Data to parse")]
    data: Option<String>,
}

#[allow(non_camel_case_types)]
#[derive(strum::Display, strum::EnumString, Clone, Debug)]
enum Program {
    scan,
    index,
}

fn main() {
    let args = Args::parse();
    match args.program {
        Program::scan => {
            match ipsea::send_command(App::Scan, &(), Some(|_: ()| {})) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Failed to connect to service. ({e:?})");
                }
            };
        }
        Program::index => {
            let q = args.data.unwrap_or_else(|| {
                eprintln!("No data provided");
                std::process::exit(1);
            });

            println!("Searching for: {}", q);
            ipsea::send_command(
                App::IndexService,
                &index::ty::Request { query: q },
                Some(move |value: index::ty::SearchResult| match args.json {
                    true => println!("{}", serde_json::to_string(&value).unwrap()),
                    false => {
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
                    }
                }),
            )
            .expect("Failed to connect to index");

            println!("Done")
        }
    }
}
