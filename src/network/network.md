## network

### 역할

- 멀티플레이 시 서버와 송수신하고 오차 수정에 필요한 네트워크 상태를 관리하는 모듈

### 책임

- connection management
- packet send/receive
- message serialization/deserialization
- server snapshot reception
- client command transmission
- ack/sequence/latency tracking
- session status management

### 비책임

- rendering
- game logic judgment

### 데이터

#### 연결/세션 데이터

- ConnectionState
- SessionId
- ClientId
- ServerTime
- RoundTripTime
- PacketLossEstimate

#### 송신 관련

- OutgoingCommandQueue
- OutgoingReliableQueue
- SequenceNumber
- LastSentInputSequence
- PendingAcks

#### 수신 관련

- IncomingPacketBuffer
- LastReceivedServerTick
- LastAppliedSnapshotTick
- ReceivedEventQueue

#### 클라 보정용

- PredictedInputHistory
    - seq별 로컬 입력 기록
- UnconfirmedCommandHistory
    - 서버 ack 안 온 command들
- SnapshotBuffer
    - remote entity interpolation용 최근 authoritative 상태들
- ReconiliationState
    - 마지막 confirmed input seq
    - 마지막 corrected player state

#### 서버 입장에서의 추가 필요 데이터:

- ClientConnectionMap
- PerClientInputBuffer
- ReplicationState
- InterestManagementState

#### Message Type Enum (실제 사용할만한 예시)

```rust
ClientToServer:
- InputCommand
- BlockEditCommand
- InteractCommand
- ChunkInterestUpdate
- Ping

ServerToClient:
- Ack
- AuthoritativePlayerState
- EntityStateDelta
- WorldEvent
- ChunkData
- ChunkUnload
- Pong
```

### 유스케이스

- 로컬 입력 명령 전송
    - 클라 ecs가 만든 command를 받아서 패킷으로 직렬화 후 서버로 보냄
- 서버 authoritative 상태 수신
    - 서버가 보내는 로컬 플레이어 authoritative state, 원격 플레이어/엔티티 delta, 월드 이벤트 결과를 받아 내부 버퍼에 저장
- ack 처리
    - 서버가 ‘seq 120’까지 처리했다 를 보내면
        - 그 이전 입력은 confirmed 처리/unconfirmed history 정리/reconciliation 기준점 갱신
- reconciliation 재료 제공
    - 클라 로컬 플레이어에 대해 authoritative state/마지막 confirmed seq 를 ecs/app에 제공해서 state 롤백 및 미확정 입력을 재적용할 수 있게 함
- interpolation 재료 제공
    - 원격 플레이어/엔티티에 대해 최근 authoritative snapshots 3개를 2~3개르 보관하고, 렌더 틱 기준 보간 가능한 상태를 제공
- 월드 이벤트 수신
    - 블럭 파괴/설치 승인, 청크 생성/변경 결과, 아이템 드랍 생성 등을 수신해서 ecs/world에 넘길 수 있게 함
- 청크 데이터 전송/수신
    - 클라가 필요한 청크 관심 범위를 서버에 알리면 서버는 해당 청크 데이터 또는 delta를 보냄
- 연결 상태 관리
    - connect/reconnect/timeout/pingpong/disconnect reason 등

### 인터페이스

#### 클라이언트 쪽 API 예시:

```rust
NetworkClient::connect(addr)
NetworkClient::disconnect()

NetworkClient::queue_command(cmd: ClientCommand)
NetworkClient::flush_outgoing()

NetworkClient::poll_incoming() -> Vec<ServerMessage>
NetworkClient::drain_events() -> Vec<NetworkEvent>

NetworkClient::latest_authoritative_player_state() -> Option<AuthoritativePlayerState>
NetworkClient::snapshot_buffer(entity_id) -> Option<&SnapshotBuffer>

NetworkClient::ack_state() -> AckState
NetworkClient::connection_state() -> ConnectionState
```

#### 서버 쪽 API 예시:

```rust
NetworkServer::start(bind_addr)
NetworkServer::poll_incoming() -> Vec<ClientEnvelope>
NetworkServer::broadcast(msg)
NetworkServer::send_to(client_id, msg)

NetworkServer::queue_replication(client_id, msg)
NetworkServer::flush_outgoing()

NetworkServer::connections() -> &ConnectionMap
```

- ECS가 `ClientCommand` 생성
- `app`이 `network.queue_command(...)`
- 프레임 중 `network.flush_outgoing()`
- `network.poll_incoming()`
- 받은 `ServerMessage`를 ECS/world에 반영

### 의존성

- 공용 메시지 스키마
- world의 일부 공용 탕비
- ecs와 공유하는 command/state DTO

NOT:

- renderer, app, platform
- world의 내부 구현
- ecs의 내부 구현체/리소스 구조

network 모듈은 전송용 타입(ecs::commands::BlockBreakCommand)은 알아도 되지만 구체 구현(ecs::resources::InputState, world::core::WorldCore 내부 자료구조 등)은 몰라야 한다.

### 불변식

1. network는 authoritative game rule을 결정하지 않는다. 그건 서버 ecs/world 책임이다.
2. 클라 로컬 예측 상태와 서버 authoritative 상태는 구분된다.
3. 각 client command는 적어도 client_id, sequence, client_tick 또는 timestamp를 가진다.
4. authoritative player state는 반드시 어떤 input sequence까지 반영했는지를 포함해야 한다. 그래야 클라가 rollback/reapply 가능하다.
5. 원격 엔티티 interpolation용 snapshot은 시간 순으로 저장되고, 렌더는 항상 과거 두 snapshot 사이를 보간한다.
6. world event는 authoritative 결과만 반영한다.
7. 서버가 처리한 tick보다 과거의 메시지를 현재 상태에 다시 적용하지 않는다.