# world_dump_common

## Role

- Share created-world persistence helpers across `world_create`, `world_coords`, and `chunk_preview`.

## Responsibilities

- define the created-world manifest schema
- save/load generated chunk payloads through `world::storage`
- summarize chunk-stack relief for preview scoring

## Notes

- The current stack summary is stone-phase oriented and scores raw solid relief only.
- The manifest stores a default preview center so later preview runs can skip expensive coordinate search.
