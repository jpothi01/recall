use chrono;
use serde_derive::Deserialize;
use shellexpand;
use sqlite::State;
use std::convert::TryFrom;
use std::env;
use std::fs;
use std::fs::File;
use std::path::Display;
use std::path::Path;
use std::path::PathBuf;
use std::time;
use structopt::StructOpt;
use toml::de;

const CONFIG_FILENAME: &'static str = ".recall.toml";

#[derive(Deserialize)]
struct Config {
    db_path: String,
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "recall",
    about = "An integrated task tracking CLI application."
)]
struct Options {
    /// Input file
    #[structopt(parse(from_str))]
    note_title: Option<String>,
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

struct Note {
    datetime_millis: i64,
    title: String,
    path: Option<String>,
    link: Option<String>,
}

impl Note {
    fn new(title: String, path: Option<String>, link: Option<String>) -> Note {
        Note {
            datetime_millis: i64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            )
            .unwrap(),
            title: title,
            path: path,
            link: link,
        }
    }
}

fn insert_note(connnection: sqlite::Connection, note: Note) -> sqlite::Result<()> {
    connnection.execute(format!(
        "
    INSERT INTO notes (datetime, title, link, path)
    VALUES ({}, '{}', '{}', '{}')
    ",
        note.datetime_millis,
        note.title,
        note.path.unwrap_or(String::from("NULL")),
        note.link.unwrap_or(String::from("NULL"))
    ))
}

fn list_notes(connection: sqlite::Connection) -> sqlite::Result<Vec<Note>> {
    let mut statement = connection
        .prepare("SELECT (datetime, title, link, path) FROM notes WHERE archived = FALSE")?;

    let mut result = Vec::<Note>::new();
    while let State::Row = statement.next()? {
        let datetime = statement.read::<i64>(0)?;
        let title = statement.read::<String>(1)?;
        let link = statement.read::<Option<String>>(2)?;
        let path = statement.read::<Option<String>>(3)?;
        result.push(Note {
            datetime_millis: datetime,
            title: title,
            link: link,
            path: path,
        });
    }

    Ok(result)
}

fn run(config: Config, options: Options) -> sqlite::Result<()> {
    let connection = sqlite::open(Path::new(&*shellexpand::tilde(&config.db_path)))?;

    connection.execute(
        "
        CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            datetime INTEGER NOT NULL,
            archived BOOLEAN NOT NULL DEFAULT FALSE,
            title TEXT NOT NULL,
            link TEXT,
            path TEXT
        );
        ",
    )?;

    match options.note_title {
        Some(note_title) => insert_note(connection, Note::new(note_title, None, None)),
        None => {
            for note in list_notes(connection)? {
                println!(
                    "{}\t|{}",
                    chrono::NaiveDateTime::from_timestamp(note.datetime_millis, 0),
                    note.title
                );
            }
            Ok(())
        }
    }
}

fn main() {
    let options = Options::from_args();
    let maybe_config_file = find_config_file();
    match maybe_config_file {
        None => println!("Could not find config file. Create a file named '{}' in an ancestor to the current directory", CONFIG_FILENAME),
        Some(config_file) => {
            match fs::read_to_string(*config_file.clone()) {
                Ok(config_string) => {
                    let maybe_config = toml::from_str::<Config>(&config_string);
                    match maybe_config {
                        Ok(config) => match run(config, options) {
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
}
