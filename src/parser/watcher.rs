use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;

pub fn watch(data_dir: &Path) -> anyhow::Result<mpsc::Receiver<Vec<String>>> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
        if let Ok(event) = res {
            if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                let paths: Vec<String> = event
                    .paths
                    .iter()
                    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"))
                    .filter_map(|p| p.to_str().map(String::from))
                    .collect();
                if !paths.is_empty() {
                    let _ = tx.send(paths);
                }
            }
        }
    })?;
    watcher.watch(data_dir, RecursiveMode::Recursive)?;
    std::mem::forget(watcher);
    Ok(rx)
}
