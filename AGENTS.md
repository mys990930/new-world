## Core Rules

이 프로젝트는 **Specification-Driven Development (SDD)** 와 **Test-Driven Development (TDD)** 를 함께 따른다. 문서, 계약, 모듈 경계가 의도된 시스템 동작을 정의하고, 구현과 테스트는 그 문서를 기준으로 개발·검증·정렬한다.

0. **최상위 개요 문서를 먼저 읽는다.**
   - 문서를 읽을땐 반드시 utf-8 인코딩을 사용한다.
   - 현재 저장소의 실제 상위 인덱스는 `context.md`다.
   - 이후 `PROJECT.md`가 추가되면 두 문서를 동기화하고, 존재하는 상위 개요 문서를 먼저 읽는다.
   - 상위 개요 문서가 비어 있거나 없으면, 구현 전에 그 공백부터 보고한다.

1. **구현 전에 문서를 먼저 읽는다.**
   - 먼저 `context.md`로 전체 계층, 의존 방향, 런타임 흐름을 파악한다.
   - 그다음 작업 대상 모듈의 루트 문서 `src/<module>/<module>.md`를 읽는다.
   - 해당 모듈에 세부 문서가 있으면 관련 `src/<module>/*.md`를 추가로 읽는다.
   - 문서와 구현이 충돌하면, 사용자가 명시적으로 다르게 결정하지 않는 한 문서를 source of truth로 취급한다.
   - 구현이 의도된 동작을 바꾸면 같은 작업에서 문서를 즉시 함께 갱신한다.

2. **이 프로젝트의 실제 모듈 문서 구조를 따른다.**
   - 이 저장소는 전역 `docs/` 트리보다, **코드 옆의 모듈별 문서**를 기준으로 설계한다.
   - 현재 표준 구조는 아래와 같다.
     - 모듈 루트 문서: `src/<module>/<module>.md`
     - 세부 설계 문서: `src/<module>/<leaf>.md`
     - 구현 파일: `src/<module>/<leaf>.rs`, `src/<module>/mod.rs`
     - 엔트리 포인트: `src/main.rs`
   - 현재 세부 문서까지 분해된 모듈:
     - `src/app/`: `app.md`, `bootstrap.md`, `config.md`, `state.md`, `runner.md`, `frame.md`, `fixed.md`, `bridge.md`, `shutdown.md`
     - `src/platform/`: `platform.md`, `event.md`, `window.md`, `input.md`, `lifecycle.md`, `runtime.md`
   - 현재 루트 문서 중심인 모듈:
     - `src/world/world.md`
     - `src/simulation/simulation.md`
     - `src/ecs/ecs.md`
     - `src/jobs/jobs.md`
     - `src/renderer/renderer.md`
     - `src/network/network.md`
   - 작업 전 기본 읽기 순서는 아래와 같다.
     1. `context.md`
     2. 대상 모듈의 `src/<module>/<module>.md`
     3. 관련 세부 문서 `src/<module>/*.md`
     4. `src/<module>/mod.rs` 및 관련 `*.rs`
   - 큰 작업은 구현 전에 어떤 문서를 읽었고 어떤 순서로 수정할지 먼저 요약한다.

3. **모듈 경계를 실제 소유 구조에 맞춰 엄격히 지킨다.**
   - 먼저 어떤 모듈이 그 책임을 소유하는지 결정한다.
   - 모듈 루트 문서가 경계 정의의 기준이고, 세부 문서는 파일 단위 책임을 보강한다.
   - 현재 저장소에는 `docs/contracts/*` 같은 공용 계약 디렉터리가 없다.
   - 따라서 cross-module communication은 `context.md`와 각 모듈 문서의 인터페이스/불변식 섹션을 기준으로만 노출한다.
   - 새로운 공유 DTO나 계약 위치가 필요하면, 먼저 문서에 반영하고 필요 시 사용자와 범위를 맞춘다.
   - 구현이 연결되어 있는 실제 활성 파일은 `src/main.rs`, `mod.rs`, `pub mod` 선언을 기준으로 판단한다.
   - 비어 있거나 아직 연결되지 않은 placeholder 파일이 있더라도, 문서 없는 편의 구현으로 경계를 넓히지 않는다.

4. **정책이나 소유권이 애매하면 사용자가 결정한다.**
   - 모듈 소유권이 불명확하면 임의로 정하지 않는다.
   - 상충하는 계약이나 API 해석이 있으면 조용히 하나를 고르지 않는다.
   - 그 경우 충돌 지점을 요약하고 사용자에게 결정받는다.

5. **문서와 구현을 항상 같이 맞춘다.**
   - 코드를 끝냈다고 작업이 끝난 것이 아니다.
   - 수정한 `*.rs`에 대응하는 `*.md`가 있으면 함께 갱신한다.
   - 새 leaf 구현 파일을 만들면 같은 폴더에 대응하는 설계 문서도 같이 만든다.
   - 모듈 책임이나 의존 방향이 바뀌면 `context.md`와 `src/<module>/<module>.md`를 함께 갱신한다.
   - 현재 프로젝트는 `module.md / contracts.md / test.md`가 기본 템플릿이 아니다.
   - 별도 `contracts.md` 또는 `test.md`가 정말 필요해지면, 해당 모듈 폴더 안에 추가하고 기존 루트 문서와 관계를 문서에 명시한다.
   - 아직 구현되지 않은 문서 전용 모듈도 설계상 authoritative 하므로, 구현보다 문서를 먼저 확장한다.

6. **현재 구현 성숙도에 맞춰 범위를 넓힌다.**
   - 현재 실행 가능한 최소 vertical slice는 대체로 `src/main.rs -> app -> platform`이다.
   - `world`, `simulation`, `ecs`, `jobs`, `renderer`, `network`는 주로 스펙 문서가 앞서 있는 상태다.
   - 따라서 큰 기능을 넣을 때는 문서화된 의존 순서를 따라 선행 레이어부터 확장한다.
   - 예를 들어 입력 해석은 `platform`이 아니라 `ecs`, 고정 시간 규칙은 `simulation`, 원본 블록 데이터 수정은 `world`가 소유한다.
   - 편의상 상위 모듈에 로직을 임시로 밀어 넣지 않는다.

7. **작업을 마친 뒤에는 아래를 반드시 수행한다.**
   - 어떤 파일을 왜 수정했는지 보고한다.
   - 문서와 구현이 어긋났다면 관련 문서를 함께 수정한다.
   - 가능한 범위에서 관련 검증을 실행하고, 못 했다면 이유를 적는다.
   - 큰 변화가 끝나면 commit한다. **push는 사용자 허가 없이 하지 않는다.**
