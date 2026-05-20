# 바이옴 빈도 샘플 요약

## 샘플링 기준

- `biome_cell_inspector`를 사용해 seed `1..5`를 샘플링했다.
- 중심 좌표는 world-block 기준 `(0, 0)`이고, span은 `32768` blocks다.
- HTML 안의 `CELL_DETAILS`에서 exact biome 이름을 파싱했다.
- 총 visible biome cell 수는 `145,581`개다.
- 엄밀한 전수조사는 아니며, 현재 generator의 대략적인 분포감을 보기 위한 샘플이다.
- 추가로 `biome_map_preview` grouped stats로 seed `6..15`도 확인했다.

## Exact Biome 빈도 순서

| 순위 | Biome | 한글 번역명 | 비율 | 보상(아이템/조합법 등) |
|---:|---|---|---|---:|
| 1 | TemperateBroadleafForest | 온대 활엽수림 | 20.04% | 목재, 씨앗
| 2 | TemperateGrassland | 온대 초원 | 19.64% | 동물, 꽃, 씨앗
| 3 | ShallowOcean | 얕은 바다 | 19.56% | 연안 통발 말뚝(연안 통발장), 조개, 게, 소형 어류
| 4 | TemperateMixedForest | 온대 혼합림 | 10.92% | 목재, 씨앗
| 5 | TropicalDryForest | 열대 건조림 | 7.62% | 단단한 건조림 목재, 건기 작물 씨앗, 숯
| 6 | SemiDesert | 반사막 | 4.65% | 붉은 모래, 알로에, 용설란류 섬유, 염소, 벽돌 제작대
| 7 | Steppe | 스텝 초원 | 4.44% | 야생 곡물, 풀씨, 건초, 사료
| 8 | Savanna | 사바나 | 3.64% | 지푸라기, 건초, 사바나 동물
| 9 | SandyCoast | 모래 해안 | 2.52% | 모래, 소금,
| 10 | BorealForest | 한대 침엽수림 | 2.50% |
| 11 | MonsoonForest | 몬순림 | 1.36% |
| 12 | TemperateRainforest | 온대 우림 | 0.93% |
| 13 | Desert | 사막 | 0.69% | 모래, 선인장, 고온저습 기반 건조창고 등
| 14 | Tundra | 툰드라 | 0.61% | 대형 초식동물, 짧은 여름 작물, 방한복, 저장고
| 15 | TropicalRainforest | 열대 우림 | 0.18% |
| 16 | MediterraneanShrubland | 지중해성 관목지 | 0.16% | 올리브, 포도, 허브, 벌꿀
| 17 | Lake | 호수 | 0.13% | 민물고기, 점토, 겨울 얼음 저장/얼음 채집
| 18 | DeepOcean | 깊은 바다 | 0.10% | 대형 닻돌(원양 정박), 대형 어류, 파묻힌 난파선 유적
| 19 | EstuarineCoast | 하구 해안 | 0.09% | 갈대, 진흙/점토(가마)
| 20 | Marsh | 소택지 | 0.07% | 갈대, 부들, 이탄
| 21 | RockyCoast | 암석 해안 | 0.05% | 따개비/해조류, 등대 기초석(등대)
| 22 | PolarBarrens | 극지 황무지 | 0.03% | 얼음, 털가죽, 영구 냉장고
| 23 | Mangrove | 맹그로브 | 0.03% | 맹그로브 목재
| 24 | Swamp | 늪지 | 0.02% | 방수 목재, 개구리
| 25 | AlpineMeadow | 고산 초지 | 0.01% |
| 26 | LagoonCoast | 석호 해안 | 0.01% | 조개, 진주, 패각(석회가마)
| 27 | DryShrubland | 건조 관목지 | 0.01% | 
| 28 | SubalpineWoodland | 아고산림 | 약 0.00% | 침엽수 목재

## 한글 분포 해석

- 온대 활엽수림, 온대 초원, 얕은 바다가 가장 많이 나온다.
- 상위 5개 바이옴만 합쳐도 전체의 약 `77.8%`를 차지한다.
- 숲 계열 전체는 대략 `44..46%` 정도로 가장 큰 비중을 차지한다.
- 바다 계열은 대략 `19..20%` 정도이며, 대부분 `ShallowOcean`이다.
- 초원, 사바나, 스텝 계열을 합치면 대략 `27..30%` 정도다.
- 호수, 습지, 고산, 극지 계열은 현재 샘플에서는 매우 드물다.
- `DeepOcean` 비율이 낮고 `ShallowOcean`이 높아서, 현재 샘플 window와 분류 기준에서는 바다가 얕은 해역 위주로 잡히는 편이다.

## Grouped Stats 보조 확인

`biome_map_preview`로 seed `6..15`를 추가 확인한 큰 묶음 비율은 아래와 비슷했다.

| 묶음 | 비율 |
|---|---:|
| Forest | 46.23% |
| Ocean | 19.53% |
| Grassland | 18.62% |
| Dry | 7.46% |
| Beach / Coast | 3.10% |
| Cold | 1.47% |
| Desert | 0.38% |
| Lake | 0.17% |
| Wetland | 0.12% |
| Mountain | 0.00% |
