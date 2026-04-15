# context.md

## ASCII Addendum

- Atlas remains intentionally macro. One atlas cell is `16 x 16` chunks, which is `512 x 512` blocks or `256m x 256m` at the current block scale.
- We do not currently plan to shrink atlas just to make local terrain feel more casual.
- A future deterministic meso terrain layer is planned between atlas-scale macro guidance and chunk-local micro detail.
- That meso layer should span multiple chunks, be generated on demand, and be reproducible from seed-derived region coordinates plus nearby atlas context rather than from whole-world precomputation.
- Current generation-side profiles such as `Coast`, `Plain`, `Upland`, and `Ridge` should be treated as shape families and realization categories, not as the final meso feature system.

## 1. 프로젝트 한줄 개요

이 프로젝트는 **쿼터뷰 복셀 샌드박스 게임**이다.  
핵심 목표는 지금 당장 돌아가는 게임이 아니라, **오래 확장할 수 있는 복셀 런타임**을 만드는 것이다.

---

## 2. 전체 방향

이 프로젝트는 다음 성격을 가진다.

- 블록/청크 기반의 절차적 지형 생성을 사용한 복셀 월드
- 건축, 생존, 생활, 생태계가 있는 샌드박스 플레이
- WASD 이동, 좌클릭 상호작용, 우클릭 블럭 설치 중심의 조작
- Rust 기반, bevy ecs, winit, wgpu 사용
- 무거운 범용 엔진에 덜 의존하고 필요한 계층을 직접 설계
- ECS, 월드 데이터, 렌더링, 플랫폼 경계를 분리
- 나중에 모드와 멀티플레이 확장을 고려

---

## 3. 핵심 구조

프로젝트는 대략 아래 계층으로 나뉜다.

- **platform**: OS / window / raw input 경계
- **app**: 전체 조립과 프레임 오케스트레이션
- **ecs**: 게임 상태 전이와 command 해석
- **world**: 월드 원본 데이터와 정합성 있는 연산
- **simulation**: fixed tick 기반 시간 규칙 실행 코어
- **jobs**: 무거운 비동기 작업 처리
- **renderer**: GPU 리소스와 draw/present
- **network**: 향후 멀티플레이용 송수신 계층

이 문서는 각 모듈의 세부 규격이 아니라,  
**프로젝트 전체를 한눈에 보는 상위 인덱스** 역할을 한다.
아래 문서들이 각 모듈의 세부 설계를 담는다.

- `src/platform/platform.md`
- `src/world/world.md`
- `src/simulation/simulation.md`
- `src/ecs/ecs.md`
- `src/jobs/jobs.md`
- `src/renderer/renderer.md`
- `src/app/app.md`
- `src/network/network.md`

### 각 모듈 문서에 들어갈 것
- 책임
- 비책임
- 소유 데이터
- 주요 유스케이스
- 외부 인터페이스
- 의존성
- 불변식

---

## 4. 전체 설계 원칙

### 책임 분리
- 원본 월드 데이터와 렌더 데이터는 분리
- 입력 수집과 게임 의미 해석은 분리
- 상태 전이와 무거운 작업은 분리
- 시간 기반 규칙 실행과 원본 데이터 저장은 분리
- 플랫폼 의존 코드와 게임 코어 로직은 분리

### world 중심 구조
- 블록 원본 데이터의 source of truth는 world다
- 외부는 내부 배열을 직접 건드리지 않고, 명시적 API/command/결과를 통해 수정한다
- 렌더러는 world의 원본 데이터 소유자가 아니다
- chunk generation이 참조하는 거시 지형 구조도 world 하위 atlas가 소유한다
- 큰 산맥의 spine, 배수 방향, 강 경로 같은 macro guide는 chunk가 즉흥적으로 만들지 않고 atlas 구조를 기반으로 현실화한다

### ECS 중심의 상태 전이
- ECS는 게임 의미를 다루는 계층이다
- 플레이어/엔티티 상태, 입력 해석, 청크 메타 상태, jobs 결과 반영을 담당한다
- 블록 원본 데이터 전체를 ECS에 넣지 않는다
- fixed tick에서 어떤 시뮬레이션을 어떤 범위에 적용할지 결정하는 것도 ECS/App 조합이 담당한다

### simulation 분리
- 생태계, 전기, 유체, 화재, 작물 성장 같은 시간 기반 규칙은 simulation에 둔다
- simulation은 월드를 읽고 WorldEdit / SimulationResult를 계산한다
- simulation 자체가 fixed tick을 소유하지는 않고, 실행 타이밍은 app과 ecs가 관리한다

### jobs 분리
- 청크 로드/저장, 절차 생성, 메싱처럼 무거운 일은 jobs로 분리한다
- main loop는 요청과 결과 수거만 담당한다

### 멀티플레이 확장 고려
- 처음부터 server authoritative 구조를 염두에 둔다
- raw input 대신 gameplay command 단위로 해석할 수 있어야 한다
- 필요하면 network 계층을 붙일 수 있는 구조를 유지한다

---

## 5. 의존성 감각

대략적인 의존 방향은 아래처럼 생각한다.

- `app`이 전체를 조립한다
- `ecs`는 `world`, `jobs`와 연결된다
- `simulation`은 `world`의 공용 API와 데이터 타입을 사용한다
- `jobs`는 `world`의 공용 API와 데이터 타입을 사용한다
- `renderer`는 GPU 친화적 데이터와 window/surface 경계만 본다
- `network`는 공유 command/state DTO를 사용하되 내부 구현은 모르도록 유지한다
- `world`와 `platform`은 가능한 한 상위 모듈을 모른다

즉, 상위가 하위를 조립하고, 하위는 자기 책임만 가진다.

---

## 6. 런타임 흐름 개요

런타임은 크게 frame update와 fixed update 두 축으로 돈다.

### frame update
1. platform이 OS/window 이벤트를 수집한다
2. app이 그 상태를 게임 쪽에 연결한다
3. ecs가 입력을 해석하고 상태를 갱신한다
4. 필요한 world 수정이나 jobs 요청을 만든다
5. jobs 결과를 수거해 반영한다
6. renderer가 업로드와 draw/present를 수행한다

### fixed update
1. app이 fixed timestep accumulator를 관리한다
2. fixed dt를 만족하면 fixed tick을 실행한다
3. ecs가 active simulation region / subsystem 실행 대상을 계산한다
4. simulation이 해당 tick의 결과를 계산한다
5. 결과를 world.apply_edit(...) 또는 동등한 world API로 반영한다
6. dirty chunk / remesh / save 같은 후속 요청을 만든다

즉, 전체 흐름은:

`platform → app → ecs → (simulation) → world/jobs → renderer`

