pub mod global_args;
pub mod window;

use crate::cli::global_args::GlobalArgs;
use crate::cli::window::WindowArgs;
use eyre::Context;
use facet::Facet;
use figue::FigueBuiltins;
use figue::{self as args};

#[derive(Facet, Debug)]
pub struct Cli {
    #[facet(flatten)]
    pub global_args: GlobalArgs,

    #[facet(flatten)]
    pub builtins: FigueBuiltins,

    #[facet(args::subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn invoke(self) -> eyre::Result<()> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .wrap_err("Failed to build tokio runtime")?;
        runtime.block_on(async move { self.command.invoke().await })?;
        Ok(())
    }
}

#[derive(Facet, Debug)]
#[repr(u8)]
pub enum Command {
    Window(WindowArgs),
}

impl Command {
    pub async fn invoke(self) -> eyre::Result<()> {
        match self {
            Self::Window(args) => args.invoke().await,
        }
    }
}
