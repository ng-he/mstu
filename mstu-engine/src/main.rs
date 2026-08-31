use std::env;

use mstu_engine::app::App;
use mstu_engine::rpc;
use mstu_engine::{log_error, log_info, log_warn, logging};

/// Loads every plugin library in `dir`, keyed by file stem without `lib`.
fn load_plugins(app: &mut App, dir: &str) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };

    let mut loaded = 0;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.extension().and_then(|ext| ext.to_str()) != Some("so") {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        let key = stem.strip_prefix("lib").unwrap_or(stem).replace('_', "-");

        match app.load(&key, &path.to_string_lossy()) {
            Ok(()) => loaded += 1,
            // Not every .so in the folder is a plugin.
            Err(error) => log_warn!("skipped {}: {}", path.display(), error),
        }
    }

    loaded
}

#[tokio::main]
async fn main() -> Result<(), String> {
    logging::init();

    log_info!("MSTU started");

    if env::args().any(|arg| arg == "--serve") {
        let mut app = App::new();
        let loaded = load_plugins(&mut app, "plugins");

        log_info!("serving {loaded} plugin librarie(s)");

        let served = rpc::serve(app).await.map_err(|error| error.to_string());

        if let Err(error) = &served {
            log_error!("{error}");
        }

        return served;
    }

    log_info!("MSTU stopped");

    Ok(())
}
