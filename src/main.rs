use chrono;
use colored::*;
use serde_derive::Deserialize;
use shellexpand;
use sqlite::State;
use std::convert::TryFrom;
use std::env;
use std::fmt;
use std::fs;
use std::fs::File;
use std::path::Display;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time;
use structopt::StructOpt;
use tempfile::tempdir;
use toml::de;

const CONFIG_FILENAME: &'static str = ".recall.toml";
const DEFAULT_EDITOR: &'static str = "vi";

#[derive(Deserialize)]
struct Config {
    db_path: String,
    editor_command: Option<Vec<String>>,
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

    #[structopt(short = "e", long = "edit")]
    edit: bool,

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
            NoteContent::Path(path) => (Some(path), None, None),
            NoteContent::Link(link) => (None, Some(link), None),
            NoteContent::Text(text) => (None, None, Some(text)),
        },
    };

    let mut statement = connnection.prepare(
        "
    INSERT INTO notes (datetime, title, path, link, text)
    VALUES (?, ?, ?, ?, ?)
    ",
    )?;

    statement.bind(1, note.datetime_millis)?;
    statement.bind(2, note.title.as_str())?;
    statement.bind(3, path.as_ref().map(|a| a.as_str()))?;
    statement.bind(4, link.as_ref().map(|a| a.as_str()))?;
    statement.bind(5, text.as_ref().map(|a| a.as_str()))?;
    statement.next()?;
    Ok(())
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
        Some(NoteContent::Path(path)) => Some("path".italic()),
        Some(NoteContent::Link(link)) => Some("link".italic()),
        Some(NoteContent::Text(text)) => Some("text".italic()),
        None => None,
    };
    let title_display = note.title.yellow();
    let time_display = chrono::DateTime::<chrono::Local>::from(
        std::time::UNIX_EPOCH
            + std::time::Duration::from_millis(u64::try_from(note.datetime_millis).unwrap()),
    )
    .format("%F %H:%M:%S");
    match content_display {
        None => format!("{}\t\t{}", time_display, title_display),
        Some(content_display) => {
            format!("{}\t{}\t{}", time_display, content_display, title_display)
        }
    }
}

fn note_content_display_string(note: &Note) -> String {
    match &note.content {
        Some(content) => match content {
            NoteContent::Link(link) => link.clone(),
            NoteContent::Path(path) => path.clone(),
            _ => String::from(""),
        },
        None => String::from(""),
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

#[derive(Debug)]
struct EditorError {
    message: String,
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error using editor: {}", self.message)
    }
}

#[derive(Debug)]
struct RecallError {
    message: String,
}

impl fmt::Display for RecallError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error using editor: {}", self.message)
    }
}

impl From<sqlite::Error> for RecallError {
    fn from(e: sqlite::Error) -> Self {
        RecallError {
            message: format!(
                "sqlite error code {}: {}",
                e.code.unwrap_or(-1),
                e.message.unwrap_or("unknown".to_owned())
            ),
        }
    }
}

impl From<EditorError> for RecallError {
    fn from(e: EditorError) -> Self {
        RecallError {
            message: format!("Error using editor: {}", e.message),
        }
    }
}

fn edit_text_in_editor(config: &Config, text: String) -> Result<String, EditorError> {
    let (editor_program, editor_args) = if let Some(config_editor_command) = &config.editor_command
    {
        if config_editor_command.len() == 0 {
            return Err(EditorError {
                message: String::from(
                    "The first entry in editor_command must be the path to a text editor program",
                ),
            });
        }

        (
            config_editor_command[0].clone(),
            config_editor_command[1..].to_vec(),
        )
    } else {
        (String::from(DEFAULT_EDITOR), vec![])
    };

    let maybe_dir = tempdir();
    if maybe_dir.is_err() {
        return Err(EditorError {
            message: format!("Error creating temp dir: {}", maybe_dir.unwrap_err()),
        });
    }

    let dir = maybe_dir.unwrap();
    let file_path = dir.path().join("recall-temp.txt");
    let write_result = fs::write(&file_path, text.as_str());
    if write_result.is_err() {
        return Err(EditorError {
            message: format!("Error writing to temp file: {}", write_result.unwrap_err()),
        });
    }

    let mut args = Vec::<String>::new();
    args.extend(editor_args);
    args.push(String::from(file_path.to_str().unwrap()));

    let spawn_result = Command::new(editor_program).args(args.as_slice()).spawn();

    match spawn_result {
        Ok(mut child) => {
            let exit_status = child.wait();
            match exit_status {
                Ok(status) => {
                    let code = status.code().unwrap_or(-1);
                    if (code != 0) {
                        Err(EditorError {
                            message: format!("Editor exited with code {}", code),
                        })
                    } else {
                        let result = fs::read_to_string(&file_path);
                        if result.is_err() {
                            Err(EditorError {
                                message: format!(
                                    "Error read from temp file: {}",
                                    result.unwrap_err()
                                ),
                            })
                        } else {
                            Ok(result.unwrap())
                        }
                    }
                }
                Err(err) => Err(EditorError {
                    message: format!("Editor exited with error: {}", err),
                }),
            }
        }
        Err(err) => Err(EditorError {
            message: format!("Error executing editor: {}", err),
        }),
    }
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

fn run(config: Config, options: Options) -> Result<(), RecallError> {
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
                    Ok(note_index) => Ok(archive_note(connection, note_index)?),
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
                    if options.edit {
                        match note.content {
                            Some(note_content) => match note_content {
                                NoteContent::Text(text) => {
                                    match edit_text_in_editor(&config, text) {
                                        Ok(new_text) => {
                                            // TODO: save new text
                                            Ok(())
                                        }
                                        Err(err) => Err(RecallError::from(err)),
                                    }
                                }
                                _ => Err(RecallError {
                                    message: format!("Unsupported"),
                                }),
                            },
                            None => Err(RecallError {
                                message: format!("Unsupported"),
                            }),
                        }
                    } else {
                        println!("{}", note_display_string(&note));
                        println!("{}", note_content_display_string(&note));
                        open_note(&note);
                        Ok(())
                    }
                }
                Err(err) => {
                    let note_title = note_title_or_index;

                    if options.edit {
                        if options.text.is_some()
                            || options.link.is_some()
                            || options.path.is_some()
                        {
                            println!("--edit with note title is for making a new text note with an editors.");
                            Ok(())
                        } else {
                            // TODO: error handling
                            let text = edit_text_in_editor(&config, String::from("")).unwrap();
                            Ok(insert_note(
                                connection,
                                Note::new_with_text(note_title, text),
                            )?)
                        }
                    } else if let Some(path) = options.path {
                        Ok(insert_note(
                            connection,
                            Note::new_with_path(note_title, path),
                        )?)
                    } else if let Some(link) = options.link {
                        Ok(insert_note(
                            connection,
                            Note::new_with_link(note_title, link),
                        )?)
                    } else if let Some(text) = options.text {
                        Ok(insert_note(
                            connection,
                            Note::new_with_text(note_title, text),
                        )?)
                    } else {
                        Ok(insert_note(connection, Note::new(note_title))?)
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
                            Err(err) => println!("{}", err)
                        },
                        Err(err) => println!("Could not parse config located at {}: {}", config_file.display(), err)
                    }
                }
                Err(err) => println!("Could not read config file: {}", err)
            }
        }
    }
}
