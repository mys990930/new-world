#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldMeta {
    pub seed: u64,
    pub world_version: u32,
    pub generator_version: u32,
    pub save_format_version: u32,
}

impl WorldMeta {
    pub const CURRENT_WORLD_VERSION: u32 = 1;
    pub const CURRENT_GENERATOR_VERSION: u32 = 10;
    pub const CURRENT_SAVE_FORMAT_VERSION: u32 = 1;

    pub const fn new(seed: u64) -> Self {
        Self {
            seed,
            world_version: Self::CURRENT_WORLD_VERSION,
            generator_version: Self::CURRENT_GENERATOR_VERSION,
            save_format_version: Self::CURRENT_SAVE_FORMAT_VERSION,
        }
    }
}

impl Default for WorldMeta {
    fn default() -> Self {
        Self::new(0)
    }
}
