use crate::graphics::TransparentTriangleOptions;
use facet::Facet;
use figue::{self as args};

#[derive(Facet, Debug)]
#[facet(rename_all = "kebab-case")]
pub struct WindowShowArgs {
    #[facet(args::named)]
    pub width: Option<u32>,

    #[facet(args::named)]
    pub height: Option<u32>,

    #[facet(args::named, default)]
    pub warp: bool,

    #[facet(args::named)]
    pub title: Option<String>,
}

impl WindowShowArgs {
    pub async fn invoke(self) -> eyre::Result<()> {
        crate::graphics::run(TransparentTriangleOptions {
            width: self.width.unwrap_or(1280),
            height: self.height.unwrap_or(720),
            use_warp_device: self.warp,
            title: self
                .title
                .unwrap_or_else(|| "D3D12 transparent triangle v6".to_string()),
        })
    }
}
