use chrono;
use colored::*;
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
use std::process::Command;
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
    note_title_or_index: Option<String>,

    #[structopt(short = "a", long = "archive")]
    archive: bool,

    #[structopt(short = "p", long = "path")]
    path: Option<String>,

    #[structopt(short = "l", long = "link")]
    link: Option<String>,

    #[structopt(short = "t", long = "text")]
    text: Option<String>,
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

enum NoteContent {
    Path(String),
    Link(String),
    Text(String),
}

struct Note {
    datetime_millis: i64,
    title: String,
    content: Option<NoteContent>,
}

impl Note {
    fn new(title: String) -> Note {
        Note {
            datetime_millis: i64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            )
            .unwrap(),
            title: title,
            content: None,
        }
    }

    fn new_with_path(title: String, path: String) -> Note {
        Note {
            datetime_millis: i64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            )
            .unwrap(),
            title: title,
            content: Some(NoteContent::Path(path)),
        }
    }

    fn new_with_link(title: String, link: String) -> Note {
        Note {
            datetime_millis: i64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            )
            .unwrap(),
            title: title,
            content: Some(NoteContent::Link(link)),
        }
    }

    fn new_with_text(title: String, text: String) -> Note {
        Note {
            datetime_millis: i64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            )
            .unwrap(),
            title: title,
            content: Some(NoteContent::Text(text)),
        }
    }
}

fn insert_note(connnection: sqlite::Connection, note: Note) -> sqlite::Result<()> {
    let (path, link, text) = match note.content {
        None => (None, None, None),
        Some(content) => match content {
            NoteContent::Path(path) => (Some(format!("'{}'", path)), None, None),
            NoteContent::Link(link) => (None, Some(format!("'{}'", link)), None),
            NoteContent::Text(text) => (None, None, Some(format!("'{}'", text))),
        },
    };

    connnection.execute(format!(
        "
    INSERT INTO notes (datetime, title, path, link, text)
    VALUES ({}, '{}', {}, {}, {})
    ",
        note.datetime_millis,
        note.title,
        path.unwrap_or(String::from("NULL")),
        link.unwrap_or(String::from("NULL")),
        text.unwrap_or(String::from("NULL"))
    ))
}

fn read_note(statement: &mut sqlite::Statement) -> sqlite::Result<Note> {
    let datetime = statement.read::<i64>(0)?;
    let title = statement.read::<String>(1)?;
    let path = statement.read::<Option<String>>(2)?;
    let link = statement.read::<Option<String>>(3)?;
    let text = statement.read::<Option<String>>(4)?;

    let note = if let Some(path) = path {
        Note {
            datetime_millis: datetime,
            title: title,
            content: Some(NoteContent::Path(path)),
        }
    } else if let Some(link) = link {
        Note {
            datetime_millis: datetime,
            title: title,
            content: Some(NoteContent::Link(link)),
        }
    } else if let Some(text) = text {
        Note {
            datetime_millis: datetime,
            title: title,
            content: Some(NoteContent::Text(text)),
        }
    } else {
        Note {
            datetime_millis: datetime,
            title: title,
            content: None,
        }
    };
    Ok(note)
}

fn list_notes(connection: sqlite::Connection) -> sqlite::Result<Vec<Note>> {
    let mut statement = connection.prepare(
        "SELECT datetime, title, path, link, text FROM notes WHERE archived = FALSE ORDER BY datetime",
    )?;

    let mut result = Vec::<Note>::new();
    while let State::Row = statement.next()? {
        result.push(read_note(&mut statement)?);
    }

    Ok(result)
}

fn note_display_string(note: &Note) -> String {
    let content_display = match &note.content {
        Some(NoteContent::Path(path)) => Some(path.bold()),
        Some(NoteContent::Link(link)) => Some(link.cyan()),
        Some(NoteContent::Text(text)) => Some(text.italic()),
        None => None,
    };
    let title_display = note.title.yellow();
    let time_display = chrono::DateTime::<chrono::Local>::from(
        std::time::UNIX_EPOCH
            + std::time::Duration::from_millis(u64::try_from(note.datetime_millis).unwrap()),
    )
    .format("%F %H:%M:%S");
    match content_display {
        None => format!("{}\t{}", time_display, title_display),
        Some(content_display) => {
            format!("{}\t{}\t{}", time_display, title_display, content_display)
        }
    }
}

fn read_nth_note(connection: sqlite::Connection, note_index: i64) -> sqlite::Result<Note> {
    let mut statement = connection.prepare(
        "SELECT id, title, path, link, text FROM notes WHERE archived = FALSE ORDER BY datetime",
    )?;

    let mut current_index = 0;
    while let State::Row = statement.next()? {
        if current_index != note_index {
            current_index += 1;
            continue;
        }

        return read_note(&mut statement);
    }

    // TODO: should be error
    panic!("SHould be error");
}

fn open_note(note: &Note) {
    match &note.content {
        Some(note_content) => match note_content {
            NoteContent::Link(link) => {
                Command::new("open").arg(link).spawn().unwrap();
            }
            NoteContent::Path(path) => {
                Command::new("open").arg(path).spawn().unwrap();
            }
            NoteContent::Text(text) => {
                Command::new("echo").arg(text).spawn().unwrap();
            }
        },
        None => (),
    };
}

fn archive_note(connection: sqlite::Connection, note_index: i64) -> sqlite::Result<()> {
    let mut statement = connection
        .prepare("SELECT id, title FROM notes WHERE archived = FALSE ORDER BY datetime")?;

    let mut current_index = 0;
    while let State::Row = statement.next()? {
        if current_index != note_index {
            current_index += 1;
            continue;
        }

        let id = statement.read::<i64>(0)?;
        let title = statement.read::<String>(1)?;
        let mut statement2 = connection.prepare(
            " UPDATE notes
            SET
            archived = TRUE
            WHERE id = ?
            ",
        )?;
        statement2.bind(1, id)?;
        statement2.next()?;
        println!("Note titled '{}' archived", title);
        return Ok(());
    }

    println!("Note not found. Nothing archived");
    return Ok(());
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
            path TEXT,
            link TEXT,
            text TEXT
        );
        ",
    )?;

    if options.archive {
        return match options.note_title_or_index {
            None => {
                println!("Must supply note index with --archive");
                Ok(())
            }
            Some(note_title_or_index) => {
                let note_index = note_title_or_index.parse::<i64>();
                match note_index {
                    Ok(note_index) => archive_note(connection, note_index),
                    Err(err) => {
                        println!("Error parsing note index: {}", err);
                        Ok(())
                    }
                }
            }
        };
    }

    match options.note_title_or_index {
        Some(note_title_or_index) => {
            let note_index = note_title_or_index.parse::<i64>();
            match note_index {
                Ok(note_index) => {
                    let note = read_nth_note(connection, note_index)?;
                    println!("{}", note_display_string(&note));
                    open_note(&note);
                    Ok(())
                }
                Err(err) => {
                    let note_title = note_title_or_index;
                    // TODO: validate mutually exclusive
                    if let Some(path) = options.path {
                        insert_note(connection, Note::new_with_path(note_title, path))
                    } else if let Some(link) = options.link {
                        insert_note(connection, Note::new_with_link(note_title, link))
                    } else if let Some(text) = options.text {
                        insert_note(connection, Note::new_with_text(note_title, text))
                    } else {
                        insert_note(connection, Note::new(note_title))
                    }
                }
            }
        }
        None => {
            for (i, note) in list_notes(connection)?.iter().enumerate() {
                println!("{} {}", format!("{}", i).bold(), note_display_string(note));
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
