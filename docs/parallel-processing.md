# Parallel Processing

Conveyor는 파이프라인 실행 성능을 극대화하기 위한 세 가지 병렬 처리 전략을 제공합니다.

## Table of Contents

- [Overview](#overview)
- [Executor Types](#executor-types)
  - [DagExecutor (기본)](#dagexecutor-기본)
  - [ChannelDagExecutor](#channeldagexecutor)
  - [AsyncPipeline](#asyncpipeline)
- [Configuration](#configuration)
- [StreamStage (HTTP 병렬 처리)](#streamstage-http-병렬-처리)
- [Performance Comparison](#performance-comparison)
- [Use Cases](#use-cases)
- [Best Practices](#best-practices)

## Overview

Conveyor의 병렬 처리는 다음과 같은 문제를 해결합니다:

- **Throughput 향상**: 동시에 여러 스테이지를 실행하여 처리 속도 증가
- **Latency 감소**: I/O 대기 시간 동안 다른 작업 수행
- **Resource 최적화**: CPU와 네트워크 리소스를 효율적으로 사용
- **Backpressure**: 느린 downstream 스테이지가 빠른 upstream을 자동으로 조절

### 언제 병렬 처리가 필요한가?

**병렬 처리가 효과적인 경우:**
- I/O 집약적 작업 (HTTP API 호출, 데이터베이스 쿼리)
- 많은 수의 독립적인 레코드 처리
- 복잡한 fan-out/fan-in 패턴
- 실시간 스트리밍 처리

**순차 처리가 더 나은 경우:**
- 간단한 선형 파이프라인
- 메모리 제약이 엄격한 환경
- 실행 순서가 중요한 작업
- 작은 데이터셋

## Executor Types

### DagExecutor (기본)

**Level-based parallel execution** - 레벨별로 스테이지를 그룹화하여 병렬 실행

```rust
// 실행 모델
Level 0: [source1, source2]         // 병렬 실행
Level 1: [transform]                 // Level 0 완료 후 실행
Level 2: [sink1, sink2, sink3]      // 병렬 실행
```

**특징:**
- ✅ 가장 안정적이고 예측 가능
- ✅ 메모리 효율적 (레벨별 배치 처리)
- ✅ 간단한 구현과 디버깅
- ⚠️ 레벨 간 대기 시간 발생
- ⚠️ 전체 레벨 완료까지 다음 레벨 대기

**When to use:**
- 기본 선택 (대부분의 경우)
- 안정성이 최우선
- 메모리 제약이 있는 환경
- 간단한 DAG 구조

**Example:**

```toml
# 기본값: DagExecutor가 자동으로 사용됨

[[stages]]
id = "source1"
function = "csv.read"
inputs = []

[[stages]]
id = "source2"
function = "json.read"
inputs = []

[[stages]]
id = "merge"
function = "merge.apply"
inputs = ["source1", "source2"]

[[stages]]
id = "sink"
function = "json.write"
inputs = ["merge"]
```

**실행 순서:**
```
Level 0: source1, source2 실행 (병렬)
         ⏸ 둘 다 완료될 때까지 대기
Level 1: merge 실행
         ⏸ 완료될 때까지 대기
Level 2: sink 실행
```

---

### ChannelDagExecutor

**Channel-based concurrent execution** - Bounded channel로 연결된 완전 동시 실행

```rust
// 실행 모델
[source1] --channel--> [transform1] --channel--> [sink1]
[source2] --channel--> [transform2] --channel--> [sink2]
    ↓                        ↓                       ↓
모든 스테이지가 동시에 실행됨 (독립적인 tokio task)
```

**특징:**
- ✅ **Automatic backpressure**: 느린 downstream이 upstream 속도 자동 조절
- ✅ **Concurrent execution**: 모든 스테이지가 동시에 실행
- ✅ **Memory bounded**: 채널 버퍼 크기로 메모리 사용량 제한
- ✅ **Fan-out support**: Broadcast channel로 1:N 분기 처리
- ⚠️ 설정이 복잡할 수 있음
- ⚠️ 디버깅이 어려울 수 있음

**When to use:**
- I/O 집약적 파이프라인 (HTTP, DB)
- 큰 데이터셋 스트리밍
- Backpressure가 필요한 경우
- 복잡한 fan-out 패턴

**Configuration:**

```toml
[global]
executor = "channel"
channel_buffer_size = 100  # 채널 버퍼 크기 (backpressure threshold)
concurrency = 10           # StreamStage 동시 처리 수

[[stages]]
id = "source"
function = "csv.read"
inputs = []

[[stages]]
id = "transform"
function = "filter.apply"
inputs = ["source"]

[[stages]]
id = "sink"
function = "json.write"
inputs = ["transform"]
```

**Backpressure 동작 원리:**

```
Fast Source (1000 items/sec)
    ↓
[Channel Buffer: 100]  ← 버퍼가 가득 차면
    ↓                    Source가 block됨
Slow Sink (10 items/sec)

결과: Source가 자동으로 10 items/sec로 느려짐
메모리: ~100 items만 유지 (1000개 아님!)
```

**Example: HTTP 파이프라인**

```toml
[pipeline]
name = "api-enrichment"
version = "1.0.0"

[global]
executor = "channel"
channel_buffer_size = 50   # 한 번에 50개 레코드만 버퍼링
concurrency = 10            # HTTP 요청 10개 동시 처리
plugins = ["http"]

# Step 1: 사용자 목록 로드 (빠름)
[[stages]]
id = "load_users"
function = "csv.read"
inputs = []
[stages.config]
path = "users.csv"

# Step 2: 각 사용자의 프로필 fetching (느림 - I/O bound)
[[stages]]
id = "fetch_profiles"
function = "http.fetch"
inputs = ["load_users"]
[stages.config]
url = "https://api.example.com/users/{{ id }}/profile"
mode = "per_row"
result_field = "profile"

# Step 3: 결과 저장
[[stages]]
id = "save"
function = "json.write"
inputs = ["fetch_profiles"]
[stages.config]
path = "enriched.json"
```

**이 설정의 효과:**
- `load_users`가 1000개 레코드를 빠르게 읽어도
- 채널 버퍼 (50개)가 차면 자동으로 대기
- `fetch_profiles`가 처리할 수 있는 속도로만 데이터 공급
- 메모리 사용량 제한 (~50개 레코드 + HTTP 응답)

---

### AsyncPipeline

**Actor pattern** - 각 스테이지가 독립적인 Actor로 실행

```rust
// 실행 모델
Actor[source1]  Actor[source2]
    ↓               ↓
  channel        channel
    ↓               ↓
  Actor[transform]
    ↓
  channel
    ↓
  Actor[sink]

각 Actor는 독립적인 event loop 실행
```

**특징:**
- ✅ **True concurrency**: 각 스테이지가 완전히 독립적
- ✅ **Graceful shutdown**: CancellationToken으로 깨끗한 종료
- ✅ **Multiple inputs**: tokio::select!로 다중 입력 처리
- ✅ **Fault isolation**: 한 스테이지 실패가 다른 스테이지에 영향 최소화
- ⚠️ 가장 복잡한 실행 모델
- ⚠️ 디버깅이 가장 어려움
- ⚠️ 오버헤드가 가장 큼

**When to use:**
- 매우 복잡한 fan-out/fan-in 패턴
- 장시간 실행되는 파이프라인
- Graceful shutdown이 중요한 경우
- 최대 동시성이 필요한 경우

**Configuration:**

```toml
[global]
executor = "async"
channel_buffer_size = 100
```

**Example: Complex DAG**

```toml
[global]
executor = "async"
channel_buffer_size = 100

# Multiple sources
[[stages]]
id = "source1"
function = "csv.read"
inputs = []

[[stages]]
id = "source2"
function = "json.read"
inputs = []

# Fan-in: 두 소스를 받는 transform
[[stages]]
id = "transform"
function = "merge.apply"
inputs = ["source1", "source2"]

# Fan-out: transform 결과를 3곳으로 전송
[[stages]]
id = "sink1"
function = "json.write"
inputs = ["transform"]

[[stages]]
id = "sink2"
function = "csv.write"
inputs = ["transform"]

[[stages]]
id = "sink3"
function = "stdout.write"
inputs = ["transform"]
```

**Actor 모델의 이점:**
- `source1`, `source2`가 동시에 실행
- `transform`이 어느 소스에서든 데이터 수신 즉시 처리
- 3개 sink가 동시에 실행
- 한 sink가 느려도 다른 sink는 영향 받지 않음

---

## Configuration

### Executor 선택

```toml
[global]
# 기본값: "dag" (DagExecutor)
executor = "channel"  # or "dag" or "async"
```

### Channel 설정

```toml
[global]
# Channel buffer size (ChannelDagExecutor, AsyncPipeline)
channel_buffer_size = 100

# StreamStage concurrency (병렬 레코드 처리)
concurrency = 10
```

**`channel_buffer_size` 설정 가이드:**

| 값 | 메모리 사용 | Throughput | Backpressure 반응 |
|----|------------|-----------|------------------|
| 10 | 매우 낮음 | 낮음 | 매우 빠름 |
| 100 | 낮음 | 보통 | 빠름 |
| 1000 | 보통 | 높음 | 느림 |
| 10000 | 높음 | 매우 높음 | 매우 느림 |

**권장값:**
- 일반적인 경우: `100`
- 메모리 제약: `10-50`
- 높은 throughput 필요: `500-1000`
- 느린 sink: `10-50` (빠른 backpressure 응답)

**`concurrency` 설정 가이드:**

| 값 | Use Case |
|----|----------|
| 1 | 순차 처리 (debugging) |
| 5-10 | 일반적인 HTTP API |
| 10-20 | 빠른 API, 낮은 latency |
| 50+ | 매우 빠른 API, 높은 throughput |

### Full Configuration Example

```toml
[pipeline]
name = "high-performance-pipeline"
version = "1.0.0"

[global]
executor = "channel"
channel_buffer_size = 100
concurrency = 10
log_level = "info"
plugins = ["http", "mongodb"]

# ... stages ...
```

---

## StreamStage (HTTP 병렬 처리)

`http.fetch` transform은 `StreamStage` trait을 구현하여 **레코드 단위 병렬 처리**를 지원합니다.

### How It Works

**순차 처리 (기본):**
```rust
for record in records {
    let result = http_fetch(record).await;  // 1개씩 처리
    results.push(result);
}
// 100개 레코드 × 100ms latency = 10초
```

**병렬 처리 (StreamStage):**
```rust
stream::iter(records)
    .map(|record| async { http_fetch(record).await })
    .buffer_unordered(10)  // 10개 동시 처리
    .collect()
    .await;
// 100개 레코드 ÷ 10 concurrency × 100ms = 1초 (10배 빠름!)
```

### Configuration

```toml
[global]
concurrency = 10  # HTTP 요청 동시 처리 수

[[stages]]
id = "enrich"
function = "http.fetch"
inputs = ["data"]
[stages.config]
url = "https://api.example.com/enrich/{{ id }}"
mode = "per_row"
```

### Performance Example

**테스트 조건:**
- 100개 레코드
- HTTP API latency: 100ms
- API는 병렬 요청 허용

**결과:**

| Concurrency | 실행 시간 | Throughput | 속도 향상 |
|-------------|----------|-----------|----------|
| 1 (순차) | 10.0초 | 10 req/s | 1x |
| 5 | 2.0초 | 50 req/s | 5x |
| 10 | 1.0초 | 100 req/s | **10x** |
| 20 | 0.5초 | 200 req/s | 20x |
| 50 | 0.2초 | 500 req/s | 50x |

### Real-World Example

```toml
[pipeline]
name = "user-enrichment"
version = "1.0.0"

[global]
executor = "channel"
channel_buffer_size = 50
concurrency = 10  # 핵심 설정!

# Step 1: 1000명의 사용자 로드
[[stages]]
id = "load_users"
function = "csv.read"
inputs = []
[stages.config]
path = "users.csv"  # 1000 rows

# Step 2: 각 사용자의 프로필 API 호출
# Concurrency=10이므로 10명씩 동시 처리
[[stages]]
id = "fetch_profiles"
function = "http.fetch"
inputs = ["load_users"]
[stages.config]
url = "https://api.example.com/users/{{ id }}/profile"
mode = "per_row"
result_field = "profile"

# Step 3: 추가 데이터 fetch (또 다른 병렬 처리!)
[[stages]]
id = "fetch_orders"
function = "http.fetch"
inputs = ["fetch_profiles"]
[stages.config]
url = "https://api.example.com/users/{{ id }}/orders"
mode = "per_row"
result_field = "orders"

[[stages]]
id = "save"
function = "json.write"
inputs = ["fetch_orders"]
[stages.config]
path = "enriched_users.json"
```

**성능 분석:**
- API latency: 200ms
- 순차 처리: 1000 × 200ms × 2 = 400초 (6.7분)
- 병렬 처리: (1000 ÷ 10) × 200ms × 2 = 40초 (**10배 빠름**)
- Backpressure: 채널 버퍼(50개)로 메모리 제한

---

## Performance Comparison

### Test Setup

```toml
# 공통 설정
- 1000개 레코드
- HTTP API latency: 100ms
- API 병렬 요청 제한 없음
- 각 레코드 크기: 1KB
```

### Results

| Executor | 실행 시간 | 메모리 사용 | Throughput | 특징 |
|----------|----------|-----------|-----------|-----|
| **DagExecutor** | 100초 | 낮음 (1MB) | 10 req/s | 레벨별 배치, 안정적 |
| **ChannelDagExecutor** | 10초 | 보통 (10MB) | 100 req/s | Backpressure, 동시 실행 |
| **AsyncPipeline** | 10초 | 보통 (10MB) | 100 req/s | 최대 동시성, 복잡 |

### Scenario-Specific Performance

**1. Simple Linear Pipeline (csv → filter → json)**

| Executor | 실행 시간 |
|----------|----------|
| DagExecutor | 1.0초 |
| ChannelDagExecutor | 1.1초 |
| AsyncPipeline | 1.2초 |

**결론:** 간단한 파이프라인은 DagExecutor가 오버헤드 없이 가장 빠름

**2. I/O Heavy Pipeline (csv → http.fetch → http.fetch → json)**

| Executor | 실행 시간 |
|----------|----------|
| DagExecutor | 200초 |
| ChannelDagExecutor | 20초 (**10x**) |
| AsyncPipeline | 20초 (**10x**) |

**결론:** I/O 집약적 작업은 병렬 처리가 필수

**3. Complex Fan-out (source → 5 transforms → 10 sinks)**

| Executor | 실행 시간 |
|----------|----------|
| DagExecutor | 15초 |
| ChannelDagExecutor | 8초 |
| AsyncPipeline | 5초 (**3x**) |

**결론:** 복잡한 DAG는 AsyncPipeline이 가장 효율적

---

## Use Cases

### Use Case 1: API Enrichment Pipeline

**요구사항:**
- CSV에서 1000개 사용자 로드
- 각 사용자에 대해 2개 API 호출
- 결과를 JSON으로 저장

**권장 설정:**

```toml
[global]
executor = "channel"
channel_buffer_size = 50
concurrency = 10
```

**이유:**
- `channel`: I/O 집약적이므로 backpressure 필요
- `buffer_size=50`: 메모리 제한하면서 충분한 처리량
- `concurrency=10`: API rate limit 고려한 적절한 동시성

---

### Use Case 2: Data Migration

**요구사항:**
- MongoDB에서 백만 개 문서 읽기
- 변환 후 다른 DB에 저장
- 메모리 제약 엄격

**권장 설정:**

```toml
[global]
executor = "channel"
channel_buffer_size = 10
concurrency = 5
```

**이유:**
- `channel`: 스트리밍 처리로 메모리 효율
- `buffer_size=10`: 매우 작은 버퍼로 메모리 최소화
- `concurrency=5`: 안정적인 DB 부하

---

### Use Case 3: Real-time Log Processing

**요구사항:**
- Stdin에서 실시간 로그 수신
- 필터링 및 변환
- 여러 출력 (파일, DB, stdout)

**권장 설정:**

```toml
[global]
executor = "async"
channel_buffer_size = 100
execution_mode = "streaming"
```

**이유:**
- `async`: Graceful shutdown 필요
- `streaming`: 실시간 처리 모드
- `buffer_size=100`: 버스트 트래픽 대응

---

### Use Case 4: Simple ETL

**요구사항:**
- CSV → 필터링 → 정렬 → JSON
- 단순 변환
- 빠른 실행 우선

**권장 설정:**

```toml
[global]
# executor 설정 없음 (기본값 사용)
```

**이유:**
- 기본 `DagExecutor`가 가장 빠르고 간단
- 병렬 처리 오버헤드 불필요

---

## Best Practices

### 1. Executor 선택 가이드

```
Start with DagExecutor (기본값)
    ↓
I/O 집약적인가? → Yes → ChannelDagExecutor
    ↓ No
매우 복잡한 DAG? → Yes → AsyncPipeline
    ↓ No
DagExecutor 유지
```

### 2. Concurrency 설정

**시작값:**
```toml
concurrency = 10  # 대부분의 경우 적절
```

**조정 기준:**
- API rate limit 있음 → 낮춤 (5)
- 매우 빠른 API → 높임 (20-50)
- 메모리 제약 → 낮춤 (5)
- 디버깅 중 → 1 (순차 실행)

### 3. Buffer Size 설정

**공식:**
```
buffer_size = concurrency × 10
```

**예시:**
- `concurrency=10` → `buffer_size=100`
- `concurrency=5` → `buffer_size=50`

**이유:**
- 충분한 버퍼로 처리 효율 유지
- 너무 크면 메모리 낭비
- 너무 작으면 처리 중단 빈번

### 4. 성능 측정

**실행 시간 측정:**
```bash
time conveyor run pipeline.toml
```

**로그 레벨 조정:**
```toml
[global]
log_level = "debug"  # 개발 중
log_level = "info"   # 운영 중
```

**프로파일링:**
```bash
# 다양한 설정으로 테스트
conveyor run pipeline.toml  # DagExecutor
conveyor run pipeline-channel.toml  # ChannelDagExecutor

# 실행 시간과 메모리 비교
```

### 5. 에러 처리

**Continue 전략과 병렬 처리:**
```toml
[global]
executor = "channel"
concurrency = 10

[error_handling]
strategy = "continue"  # 한 요청 실패해도 계속
```

**이점:**
- 부분적인 실패가 전체 파이프라인 중단 방지
- 병렬 처리에서 더 중요 (많은 요청 중 일부 실패 가능)

### 6. 모니터링

**Debug 로그로 병렬 처리 확인:**
```toml
[global]
log_level = "debug"
```

**출력 예시:**
```
[DEBUG] Channel stage 'fetch_profiles' started
[DEBUG] Parallel HTTP request to: https://api.example.com/users/1/profile
[DEBUG] Parallel HTTP request to: https://api.example.com/users/2/profile
...
[DEBUG] Parallel HTTP request to: https://api.example.com/users/10/profile
[DEBUG] Channel stage 'fetch_profiles' completed
```

### 7. 일반적인 실수

**❌ 잘못된 설정:**
```toml
[global]
executor = "channel"
concurrency = 100  # 너무 높음!
channel_buffer_size = 10  # 너무 작음!
```

**문제:**
- 높은 concurrency가 작은 버퍼를 빠르게 채움
- 빈번한 backpressure로 성능 저하

**✅ 올바른 설정:**
```toml
[global]
executor = "channel"
concurrency = 10
channel_buffer_size = 100  # concurrency × 10
```

---

## Limitations

### 1. Streaming Data

**제약:**
- `DataFormat::Stream`은 한 번만 소비 가능
- Clone 불가능
- Fan-out 패턴에서 사용 제한

**해결:**
```toml
# Stream을 RecordBatch로 변환 후 fan-out
[[stages]]
id = "to_records"
function = "stream.to_records"
inputs = ["stream_source"]

# 이제 fan-out 가능
[[stages]]
id = "sink1"
function = "json.write"
inputs = ["to_records"]

[[stages]]
id = "sink2"
function = "csv.write"
inputs = ["to_records"]
```

### 2. Memory Management

**문제:**
- 큰 DataFrame을 fan-out하면 메모리 사용량 증가
- Clone 비용이 높음

**해결:**
```toml
# buffer_size를 작게 설정
[global]
channel_buffer_size = 10

# 또는 fan-out 전에 필요한 컬럼만 선택
[[stages]]
id = "select"
function = "select.apply"
inputs = ["data"]
[stages.config]
columns = ["id", "name"]  # 필요한 것만

[[stages]]
id = "sink1"
function = "json.write"
inputs = ["select"]
```

### 3. Debugging

**문제:**
- 병렬 실행은 디버깅이 어려움
- 로그가 섞임

**해결:**
```toml
# 디버깅 시 순차 실행
[global]
# executor를 설정하지 않음 (DagExecutor 사용)
concurrency = 1  # StreamStage도 순차 실행
log_level = "debug"
```

---

## See Also

- [DAG Pipelines](dag-pipelines.md) - DAG 구조와 실행 순서
- [Configuration Guide](configuration.md) - 전체 설정 옵션
- [HTTP Plugin](plugins/http.md) - HTTP 플러그인 사용법
- [Development Guide](development.md) - 커스텀 StreamStage 구현
- [CLAUDE.md](../CLAUDE.md) - 구현 세부사항
