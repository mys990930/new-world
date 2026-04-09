# registry

## 역할

- 데이터 파일 기반 block definition과 texture tile 카탈로그를 world 쪽에서 해석한다.
- `BlockId`가 가진 gameplay/render 의미를 `BlockRegistry`를 통해 읽을 수 있게 한다.

## 책임

- `BlockRegistry`, `BlockDef`, `TextureTileDef` 정의
- block key/id lookup 규칙 정의
- texture key/layer lookup 규칙 정의
- TOML manifest 파싱과 상대 경로 해석
- built-in white tile 예약
- missing block fallback 정의

## 비책임

- PNG 디코드
- GPU texture 생성/업로드
- loaded chunk map 소유
- generation/meshing 실행 정책 결정

## 소유 데이터

### TextureTileDef

- `id`
- `key`
- `source`

### BlockDef

- `id`
- `key`
- `solid`
- `opaque`
- `render_kind`
- `face_textures`
- `tint`

### BlockRegistry

- `tile_size`
- ordered texture tile list
- block table indexed by `BlockId`
- block/texture key lookup map
- fallback `__missing` block 정의

## 입력

- manifest path
- TOML text
- manifest 기준 상대 texture path

## 출력

- loaded `BlockRegistry`
- `BlockDef` / `TextureTileDef` 조회 결과
- `BlockRegistryError`

## 공개 인터페이스

```rust
BlockRegistry::load_default() -> Result<BlockRegistry, BlockRegistryError>
BlockRegistry::load_from_path(path: impl AsRef<Path>) -> Result<BlockRegistry, BlockRegistryError>

BlockRegistry::tile_size(&self) -> u32
BlockRegistry::texture_tiles(&self) -> &[TextureTileDef]
BlockRegistry::block_id(&self, key: &str) -> Option<BlockId>
BlockRegistry::texture_id(&self, key: &str) -> Option<TextureTileId>
BlockRegistry::block(&self, id: BlockId) -> Option<&BlockDef>
BlockRegistry::block_or_missing(&self, id: BlockId) -> &BlockDef
BlockRegistry::is_solid(&self, id: BlockId) -> bool
BlockRegistry::is_opaque(&self, id: BlockId) -> bool

default_manifest_path() -> PathBuf
```

## 불변식

- manifest의 `tile_size`는 0이면 안 된다.
- texture layer `0`은 항상 built-in white tile로 예약된다.
- manifest에 선언한 texture key와 block key는 각각 중복되면 안 된다.
- `air` block은 반드시 `id = 0`으로 존재해야 한다.
- registry는 block meaning을 읽기 전용으로 제공하고, loaded world state를 직접 mutate하지 않는다.

## 관련 모듈

- `world.md`
- `chunk.md`
- `generation.md`
- `meshing.md`
- `../../assets/blocks/blocks.toml`

## 메모

- 기본 manifest 경로는 `assets/blocks/blocks.toml`이다.
- texture 파일 경로는 manifest 파일 기준 상대 경로로 해석된다.
- 현재 manifest는 `tile_size`, `[[textures]]`, `[[blocks]]` 섹션을 사용한다.
- cube block은 `top`, `bottom`, `side` texture key를 사용하고, `tint`는 선택 사항이다.
