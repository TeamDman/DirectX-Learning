#![deny(clippy::disallowed_methods)]
#![deny(clippy::disallowed_macros)]

pub mod cli;
pub mod graphics;
pub mod logging_init;

use crate::cli::Cli;

const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (rev ",
    env!("GIT_REVISION"),
    ")"
);

pub fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let cli: Cli = figue::Driver::new(
        figue::builder::<Cli>()
            .expect("schema should be valid")
            .cli(|cli| cli.args_os(std::env::args_os().skip(1)).strict())
            .help(|help| {
                help.version(VERSION)
                    .include_implementation_source_file(true)
                    .include_implementation_git_url(
                        "TeamDman/DirectX-Learning",
                        env!("GIT_REVISION"),
                    )
            })
            .build(),
    )
    .run()
    .unwrap();

    logging_init::init_logging(&cli.global_args)?;

    #[cfg(windows)]
    {
        let _ = teamy_windows::console::enable_ansi_support();
        teamy_windows::string::warn_if_utf8_not_enabled();
    }

    cli.invoke()?;
    Ok(())
}
