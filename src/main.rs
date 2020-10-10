use serde_derive::Deserialize;
use shellexpand;
use sqlite::State;
use std::env;
use std::fs;
use std::fs::File;
use std::path::Display;
use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;
use toml::de;

const CONFIG_FILENAME: &'static str = ".recall.toml";

#[derive(Deserialize)]
struct Config {
    db_path: String,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "recall", about = "An example of StructOpt usage.")]
struct Opt {
    /// Input file
    #[structopt(parse(from_str))]
    note_title: String,
}

fn find_config_file() -> Option<Box<PathBuf>> {
    let original_cwd = std::env::current_dir().unwrap();

    // TODO: recursve up directories
    let path = Path::new(CONFIG_FILENAME);
    if path.exists() {
        let result = Box::new(path.to_path_buf());
        std::env::set_current_dir(original_cwd).unwrap();
        return Some(result);
    }

    return None;
}

fn run(config: Config) -> sqlite::Result<()> {
    let connection = sqlite::open(Path::new(&*shellexpand::tilde(&config.db_path)))?;

    connection
        .execute(
            "
        CREATE TABLE users (name TEXT, age INTEGER);
        INSERT INTO users (name, age) VALUES ('Alice', 42);
        INSERT INTO users (name, age) VALUES ('Bob', 69);
        ",
        )
        .unwrap();

    Ok(())
}

fn main() {
    let maybe_config_file = find_config_file();
    match maybe_config_file {
        None => println!("Could not find config file. Create a file named '{}' in an ancestor to the current directory", CONFIG_FILENAME),
        Some(config_file) => {
            match fs::read_to_string(*config_file.clone()) {
                Ok(config_string) => {
                    let maybe_config = toml::from_str::<Config>(&config_string);
                    match maybe_config {
                        Ok(config) => match run(config) {
                            Ok(()) => (),
                            Err(err) => println!("Error: {}", err)
                        },
                        Err(err) => println!("Could not parse config located at {}: {}", config_file.display(), err)
                    }
                }
                Err(err) => println!("Could not read config file: {}", err)
            }
        }
    }

    // let mut statement = connection
    //     .prepare("SELECT * FROM users WHERE age > ?")
    //     .unwrap();

    // statement.bind(1, 50).unwrap();

    // while let State::Row = statement.next().unwrap() {
    //     println!("name = {}", statement.read::<String>(0).unwrap());
    //     println!("age = {}", statement.read::<i64>(1).unwrap());
    // }
}
