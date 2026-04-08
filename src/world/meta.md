# meta

## 역할

- world가 하나의 저장 세계로서 가지는 식별 정보와 버전 메타데이터를 정의한다.
- generation / storage / core가 공통으로 참조하는 월드 identity를 제공한다.

## 책임

- `WorldMeta` 정의
- world seed 정의
- world version / generator version / save format version 정의
- generation / storage 호환성 판단의 기준 제공

## 비책임

- loaded chunk map 소유
- 블록 수정
- 실제 파일 입출력
- 절차 생성 알고리즘 구현

## 소유 데이터

### WorldMeta

- `seed`
- `world_version`
- `generator_version`
- `save_format_version`

## 입력

- 새 world 생성 시점의 초기 설정값
- 저장된 world를 열 때 읽어온 메타 정보
- generation / storage 호환성 질의

## 출력

- 런타임 동안 참조되는 immutable world identity
- generation / storage가 참고하는 버전 기준

## 상태 전이 규칙

- `WorldMeta`는 world open 이후 일반적으로 immutable로 취급한다.
- 버전 필드 변경은 명시적인 migration 또는 tooling 경로를 통해서만 일어난다.
- `seed`는 동일 world 인스턴스 동안 바뀌지 않는다.

## 불변식

- 하나의 loaded world는 하나의 authoritative `WorldMeta`를 가진다.
- `generator_version`과 `save_format_version`은 호환성 판단에 쓰이는 명시적 버전이다.
- world identity는 chunk 개별 데이터보다 상위 레벨에서 유지된다.

## 관련 모듈

- `core.md`
- `generation.md`
- `storage.md`

## 메모

- block registry ownership은 여기서 정의하지 않는다.
- `WorldMeta`는 world가 소유하지만, generation / storage는 이를 읽기 전용으로 참조한다.
