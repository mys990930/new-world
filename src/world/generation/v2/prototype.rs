use crate::world::coord::ChunkCoord;

use super::{ChunkCorridorWindow, ChunkGenerationV2Inputs};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrototypeColumn {
    pub base_height: f32,
    pub relief_budget: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BaseHeightfieldPrototype {
    pub chunk: ChunkCoord,
    pub columns: Vec<PrototypeColumn>,
}

pub fn empty_base_heightfield_prototype(chunk: ChunkCoord) -> BaseHeightfieldPrototype {
    BaseHeightfieldPrototype {
        chunk,
        columns: Vec::new(),
    }
}

pub fn build_chunk_base_heightfield_prototype(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    corridor_window: &ChunkCorridorWindow,
) -> BaseHeightfieldPrototype {
    let _ = inputs.chunk;
    let _ = corridor_window.chunk;
    debug_assert_eq!(inputs.chunk, chunk);
    debug_assert_eq!(corridor_window.chunk, chunk);
    let _ = corridor_window.corridors.len();

    empty_base_heightfield_prototype(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::v2::{
        build_chunk_corridor_window, build_chunk_v2_scaffold, prepare_chunk_v2_inputs,
    };
    use crate::world::meta::WorldMeta;

    #[test]
    fn base_heightfield_stub_accepts_the_corridor_window() {
        let meta = WorldMeta::new(42);
        let chunk = ChunkCoord(4, 0, -3);
        let inputs = prepare_chunk_v2_inputs(chunk, &meta);
        let corridor_window = build_chunk_corridor_window(chunk, &inputs);

        let prototype = build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window);

        assert_eq!(prototype, empty_base_heightfield_prototype(chunk));
    }

    #[test]
    fn scaffold_carries_the_corridor_window_into_the_boundary() {
        let meta = WorldMeta::new(42);
        let scaffold = build_chunk_v2_scaffold(ChunkCoord(4, 0, -3), &meta);

        assert_eq!(scaffold.stage, crate::world::generation::v2::V2ScaffoldStage::CorridorWindowReady);
        assert_eq!(scaffold.corridor_window.chunk, scaffold.chunk);
        let prototype = build_chunk_base_heightfield_prototype(
            scaffold.chunk,
            &scaffold.inputs,
            &scaffold.corridor_window,
        );
        assert_eq!(prototype, empty_base_heightfield_prototype(scaffold.chunk));
    }
}
