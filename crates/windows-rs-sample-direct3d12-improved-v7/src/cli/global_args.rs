use facet::Facet;
use figue::{self as args};

#[derive(Facet, Debug, Default, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct GlobalArgs {
    #[facet(args::named, default)]
    pub debug: bool,

    #[facet(args::named)]
    pub log_filter: Option<String>,

    #[facet(args::named)]
    pub log_file: Option<String>,
}
