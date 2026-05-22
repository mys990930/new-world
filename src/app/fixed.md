# fixed

## 역할

- fixed timestep accumulator를 관리하고 fixed tick 실행을 오케스트레이션한다

## 소유 데이터

- accumulator
- fixed_dt
- max_fixed_steps_per_frame
- app-owned `SimulationCore`

## 입력

- frame dt
- TimingConfig
- ecs가 계산한 active simulation targets
- simulation 결과
- world edit 반영 결과
- world calendar/climate/weather 반영 결과
- player-focus chunk weather scalar state
- cached world region classification state

## 출력

- fixed tick 실행 횟수
- world edit 적용
- world-owned calendar/climate/weather advancement
- renderer environment 갱신
- dirty chunk / remesh / save 후속 요청
- structured fixed-tick event collection that textmode or diagnostics can aggregate into one-second summaries

## 처리 흐름

1. frame dt를 accumulator에 더한다
2. accumulator >= fixed_dt 인 동안 반복
3. ecs가 이번 tick의 simulation 대상/범위와 player-centered active chunk observer scope를 계산한다
4. cached region classification이 있는 경우에만 active-region time/weather cell input을 구성한다
5. ECS가 선택한 chunk scope 중 cached region classification이 있는 chunk만 ecology/weather chunk input으로 구성한다
6. simulation이 결과를 계산한다
7. world에 structured result를 반영한다
8. structured events를 textmode/diagnostic adapter가 소비할 수 있도록 보존하거나 전달한다
9. renderer environment를 world calendar time-of-day와 player-focus chunk weather scalar state로 갱신한다
10. 후속 jobs / renderer 연계용 dirty 신호를 만든다
11. accumulator에서 fixed_dt를 차감한다

## 상태 전이 규칙

- catch-up은 max_fixed_steps_per_frame 범위 내에서만 수행한다
- 초과 누적분 처리 방식은 정책적으로 고정한다
- simulation 실행 타이밍은 fixed 단계에서만 관리한다

## 불변식

- simulation 자체가 fixed tick을 소유하지 않는다
- fixed 실행 여부와 빈도는 app이 관리한다
- world 수정은 world API를 통해 반영한다
- frame update와 fixed update는 섞이지 않는다
- 현재 fixed vertical slice는 `time` subsystem과 ecology observer subsystem을 연결한다
- fixed tick은 uncached atlas region classification을 직접 생성하지 않고, 캐시가 없는 cell은 해당 tick의 time/weather cell update를 건너뛴다
- console text is formatted by app/binary-level observers such as `new-world-textmode`, not by simulation rules

## 비책임

- simulation 세부 규칙 구현
- gameplay command 해석
- simulation event meaning ownership
- core text formatting rules outside diagnostics/binary adapters
- renderer draw 실행
- OS 이벤트 수집

## 관련 모듈

- runner.rs
- state.rs
- ecs
- simulation
- world
- jobs

## 메모

- spiral of death 방지 정책을 문서로 고정해두는 게 좋다
- later에는 subsystem별 fixed rate 분리도 가능하다
- 현재 구현은 fixed tick마다 ECS `SimClock`/`ActiveSimRegion`/`ActiveChunkObserverScope`를 갱신하고, `SimulationCore::step_all(...)`로 time simulation과 ecology observer simulation을 실행한다
- time/weather cell input은 game-minute 경계에서만 만들어지며, region classification cache가 준비되지 않은 경우 calendar advance만 진행하고 local climate/weather update는 다음 cached tick으로 미룬다
- ecology/weather chunk input은 ECS가 선택한 `3x3` chunk scope를 사용하되, runtime frame thread에서는 uncached atlas region classification을 동기 생성하지 않는다. cache miss chunk는 해당 fixed tick에서 건너뛰고, app-owned region resolve job이 cache를 채운 뒤 다음 tick부터 input에 포함한다.
- `new-world-textmode`는 diagnostic binary라서 app runtime mapping 대신 graph-first `GraphMacroMap.biomes`를 직접 샘플한다.
- renderer environment sync는 legacy atlas `LocalWeatherState`가 아니라 world-owned `ChunkWeatherState`를 우선 사용한다. player-focus chunk의 `cloud`, `rain`, `temperature`, `moisture`, `kind`를 calendar time-of-day와 합쳐 `RenderEnvironment`로 변환한다.
- chunk weather presentation은 scene 전체가 뿌옇게 씻기지 않도록 낮은 기본 fog density와 약한 height falloff에서 출발하되, cloud/moisture/rain/storm scalar가 fog, tint, wetness, saturation, direct light를 점진적으로 조정한다.
- `new-world-textmode` should reuse this fixed orchestration shape, aggregate fixed-tick structured events once per real second, and print each `3x3` chunk line with cell biome, weather, surface condition, ecology events, and world update records.
