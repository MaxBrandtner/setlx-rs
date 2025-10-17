use std::fs::{File, OpenOptions, read};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::exit;

pub fn debug_file_create(name: String) -> File {
    match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&name)
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create {name}");
            match e.kind() {
                ErrorKind::PermissionDenied => eprintln!("permission denied"),
                _ => eprintln!("unknown error occured"),
            }

            exit(1);
        }
    }
}

pub fn file_read(path: &PathBuf) -> String {
    let bytes = match read(path) {
        Ok(c) => c,
        Err(e) => {
            match e.kind() {
                ErrorKind::NotFound => {
                    eprintln!("File not found: {}", path.to_str().unwrap())
                }
                ErrorKind::PermissionDenied => {
                    eprintln!("Permission denied: {}", path.to_str().unwrap())
                }
                ErrorKind::InvalidData => eprintln!(
                    "File contains invalid UTF-8: {}",
                    path.to_str().unwrap()
                ),
                _ => eprintln!(
                    "Unexpected error ({:?}): {}",
                    e,
                    path.to_str().unwrap()
                ),
            }
            exit(1);
        }
    };

    String::from_utf8_lossy(&bytes).into_owned()
}
