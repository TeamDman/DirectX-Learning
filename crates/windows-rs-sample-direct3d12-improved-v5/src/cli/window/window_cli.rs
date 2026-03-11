use crate::cli::window::show::WindowShowArgs;
use eyre::Result;
use facet::Facet;
use figue::{self as args};

#[derive(Facet, Debug)]
pub struct WindowArgs {
    #[facet(args::subcommand)]
    pub command: WindowCommand,
}

#[derive(Facet, Debug)]
#[repr(u8)]
pub enum WindowCommand {
    Show(WindowShowArgs),
}

impl WindowArgs {
    pub async fn invoke(self) -> Result<()> {
        match self.command {
            WindowCommand::Show(args) => args.invoke().await,
        }
    }
}
