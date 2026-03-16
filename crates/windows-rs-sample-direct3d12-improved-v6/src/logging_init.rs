use crate::cli::global_args::GlobalArgs;
use chrono::Local;
use eyre::bail;
use std::fs::File;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::debug;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Registry;
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::util::SubscriberInitExt;

pub fn init_logging(global_args: &GlobalArgs) -> eyre::Result<()> {
    let subscriber = Registry::default();

    let env_filter_layer = EnvFilter::builder()
        .with_default_directive(match (global_args.debug, global_args.log_filter.as_ref()) {
            (true, None) => LevelFilter::DEBUG.into(),
            (false, None) => LevelFilter::INFO.into(),
            (true, Some(_)) => bail!("cannot specify log filter with --debug"),
            (false, Some(filter)) => LevelFilter::from_str(filter)?.into(),
        })
        .from_env()?;
    let subscriber = subscriber.with(env_filter_layer);

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_file(cfg!(debug_assertions))
        .with_line_number(cfg!(debug_assertions))
        .with_target(true)
        .with_writer(std::io::stderr)
        .pretty()
        .without_time();
    let subscriber = subscriber.with(stderr_layer);

    let json_log_path = match global_args.log_file.as_deref() {
        None => None,
        Some(path) if std::path::Path::new(path).is_dir() => {
            let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
            Some(std::path::PathBuf::from(path).join(format!("log_{timestamp}.ndjson")))
        }
        Some(path) => Some(std::path::PathBuf::from(path)),
    };

    let json_layer = if let Some(ref json_log_path) = json_log_path {
        if let Some(parent) = json_log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = File::create(json_log_path)?;
        let file = Arc::new(Mutex::new(file));
        let json_writer = BoxMakeWriter::new(move || {
            file.lock()
                .expect("failed to lock json log file")
                .try_clone()
                .expect("failed to clone json log file handle")
        });

        Some(
            tracing_subscriber::fmt::layer()
                .event_format(tracing_subscriber::fmt::format().json())
                .with_file(true)
                .with_target(false)
                .with_line_number(true)
                .with_writer(json_writer),
        )
    } else {
        None
    };
    let subscriber = subscriber.with(json_layer);

    if let Err(error) = subscriber.try_init() {
        eprintln!("Failed to initialize tracing subscriber: {error}");
        return Ok(());
    }

    debug!(?json_log_path, debug = global_args.debug, "Tracing initialized");
    Ok(())
}
