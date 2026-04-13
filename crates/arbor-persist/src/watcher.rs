use anyhow::Result;
use ignore::WalkBuilder;
use notify::RecursiveMode;
use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Events emitted by the file watcher
#[derive(Debug)]
pub enum WatchEvent {
    /// Files that were modified or created
    Changed(Vec<PathBuf>),
    /// Files that were removed
    Removed(Vec<PathBuf>),
}

/// Start watching a directory for file changes.
/// Returns a receiver that emits `WatchEvents`.
/// The watcher respects .gitignore rules.
///
/// # Errors
/// Returns an error if the file watcher cannot be created or the directory cannot be watched.
pub fn watch(root: &Path) -> Result<(mpsc::Receiver<WatchEvent>, impl Drop)> {
    let (tx, rx) = mpsc::channel();
    let root_owned = root.to_path_buf();

    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        move |events: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
            let Ok(events) = events else {
                return;
            };

            let mut changed = Vec::new();
            let mut removed = Vec::new();

            for event in events {
                if should_ignore(&root_owned, &event.path) {
                    continue;
                }

                match event.kind {
                    DebouncedEventKind::Any => {
                        if event.path.exists() {
                            changed.push(event.path);
                        } else {
                            removed.push(event.path);
                        }
                    }
                    DebouncedEventKind::AnyContinuous => {
                        if event.path.exists() {
                            changed.push(event.path);
                        }
                    }
                    _ => {}
                }
            }

            if !changed.is_empty() {
                let _ = tx.send(WatchEvent::Changed(changed));
            }
            if !removed.is_empty() {
                let _ = tx.send(WatchEvent::Removed(removed));
            }
        },
    )?;

    debouncer.watcher().watch(root, RecursiveMode::Recursive)?;

    Ok((rx, debouncer))
}

const IGNORED_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "__pycache__",
    ".arbor",
    "vendor",
    "third_party",
    "testdata",
];

/// Check if a path should be ignored based on directory components
fn should_ignore(_root: &Path, path: &Path) -> bool {
    if path.is_dir() {
        return true;
    }
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|s| IGNORED_DIRS.contains(&s))
    })
}

/// Walk the project directory respecting .gitignore
pub fn walk_files(root: &Path) -> Vec<PathBuf> {
    WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
        .map(ignore::DirEntry::into_path)
        .collect()
}
