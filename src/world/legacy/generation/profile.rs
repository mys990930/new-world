#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainProfile {
    DeepOcean,
    Shelf,
    Coast,
    Plain,
    Upland,
    Ridge,
}

impl TerrainProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeepOcean => "deep_ocean",
            Self::Shelf => "shelf",
            Self::Coast => "coast",
            Self::Plain => "plain",
            Self::Upland => "upland",
            Self::Ridge => "ridge",
        }
    }
}
