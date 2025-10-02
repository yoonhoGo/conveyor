# Module Field Reference

This document provides detailed field specifications for all sources, transforms, and sinks in Conveyor.

## Table of Contents

- [Sources](#sources)
  - [CSV Source](#csv-source)
  - [JSON Source](#json-source)
  - [MongoDB Source](#mongodb-source)
  - [Stdin Source](#stdin-source)
- [Transforms](#transforms)
  - [Filter Transform](#filter-transform)
  - [Map Transform](#map-transform)
  - [Validate Schema Transform](#validate-schema-transform)
- [Sinks](#sinks)
  - [CSV Sink](#csv-sink)
  - [JSON Sink](#json-sink)
  - [Stdout Sink](#stdout-sink)

---

## Sources

### CSV Source

**Type:** `csv`

CSV 파일에서 데이터를 읽어옵니다.

#### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `path` | string | ✓ | - | CSV 파일 경로 |
| `headers` | boolean | | `true` | 첫 번째 행이 헤더인지 여부 |
| `delimiter` | string | | `","` | 컬럼 구분자 (단일 문자) |
| `infer_schema_length` | integer | | auto | 스키마 추론에 사용할 행 수 |

#### Example

```toml
[[sources]]
name = "customer_data"
type = "csv"

[sources.config]
path = "data/customers.csv"
headers = true
delimiter = ","
infer_schema_length = 1000
```

---

### JSON Source

**Type:** `json`

JSON 파일에서 데이터를 읽어옵니다.

#### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `path` | string | ✓ | - | JSON 파일 경로 |
| `format` | string | | `"records"` | JSON 포맷: `records`, `jsonl`, `dataframe` |

#### Format Options

- **`records`**: JSON 배열 형식 `[{...}, {...}]`
- **`jsonl`**: 줄바꿈으로 구분된 JSON 객체
- **`dataframe`**: Polars 네이티브 DataFrame 형식

#### Example

```toml
[[sources]]
name = "orders"
type = "json"

[sources.config]
path = "data/orders.json"
format = "records"
```

---

### MongoDB Source

**Type:** `mongodb`

MongoDB 데이터베이스에서 데이터를 읽어옵니다. Cursor-based pagination을 지원하여 대용량 데이터를 효율적으로 처리할 수 있습니다.

#### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `connection_string` | string | ✓ | - | MongoDB 연결 문자열 (예: mongodb://localhost:27017) |
| `database` | string | ✓ | - | 데이터베이스 이름 |
| `collection` | string | ✓ | - | 컬렉션 이름 |
| `query` | string | | `"{}"` | MongoDB 쿼리 (JSON 문자열) |
| `projection` | string | | - | 필드 선택 (JSON 문자열) |
| `sort` | string | | - | 정렬 순서 (JSON 문자열) |
| `limit` | integer | | - | 최대 문서 수 |
| `batch_size` | integer | | `1000` | 한번에 읽어올 문서 수 |
| `cursor_field` | string | | - | 페이지네이션 기준 필드 (예: `_id`, `created_at`) |
| `cursor_value` | string | | - | 시작 커서 값 (다음 페이지 조회 시 사용) |

#### Cursor-based Pagination

Cursor-based pagination을 사용하면 대용량 데이터를 효율적으로 처리할 수 있습니다:

1. **첫 페이지**: `cursor_field`만 지정
2. **다음 페이지**: `cursor_field`와 `cursor_value` 모두 지정
3. **동작 방식**: `cursor_value`보다 큰 값부터 조회 (`$gt` 연산자 사용)

#### Example

**기본 사용 (페이지네이션 없음)**

```toml
[[sources]]
name = "users"
type = "mongodb"

[sources.config]
connection_string = "mongodb://localhost:27017"
database = "myapp"
collection = "users"
query = '{ "status": "active" }'
limit = 1000
```

**Cursor-based Pagination (첫 페이지)**

```toml
[[sources]]
name = "events"
type = "mongodb"

[sources.config]
connection_string = "mongodb://localhost:27017"
database = "analytics"
collection = "events"
query = '{ "event_type": "purchase" }'
cursor_field = "_id"
batch_size = 5000
```

**Cursor-based Pagination (다음 페이지)**

```toml
[[sources]]
name = "events_page2"
type = "mongodb"

[sources.config]
connection_string = "mongodb://localhost:27017"
database = "analytics"
collection = "events"
query = '{ "event_type": "purchase" }'
cursor_field = "_id"
cursor_value = "507f1f77bcf86cd799439011"  # 이전 배치의 마지막 _id
batch_size = 5000
```

**고급 쿼리 및 프로젝션**

```toml
[[sources]]
name = "user_analytics"
type = "mongodb"

[sources.config]
connection_string = "mongodb://user:pass@localhost:27017"
database = "analytics"
collection = "user_events"
query = '{ "event_date": { "$gte": "2024-01-01" }, "user_type": "premium" }'
projection = '{ "_id": 1, "user_id": 1, "event_type": 1, "amount": 1 }'
sort = '{ "event_date": 1 }'
limit = 10000
```

**시간 기반 Pagination**

```toml
[[sources]]
name = "recent_logs"
type = "mongodb"

[sources.config]
connection_string = "mongodb://localhost:27017"
database = "logs"
collection = "application_logs"
query = '{ "level": "error" }'
cursor_field = "created_at"
cursor_value = "2024-01-01T12:00:00Z"
batch_size = 1000
sort = '{ "created_at": 1 }'
```

#### Notes

- `cursor_field`는 인덱스가 설정된 필드를 사용하는 것을 권장합니다
- `cursor_value`는 자동으로 ObjectId, Integer, Float, DateTime, String으로 파싱됩니다
- Pagination 사용 시 `cursor_field` 기준으로 자동 정렬됩니다
- 연결 문자열에 인증 정보를 포함할 수 있습니다: `mongodb://user:password@host:port`

---

### Stdin Source

**Type:** `stdin`

표준 입력에서 데이터를 읽어옵니다. Unix 파이프라인과 연동하여 사용할 수 있습니다.

#### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `format` | string | | `"json"` | 입력 포맷: `json`, `jsonl`, `csv`, `raw` |
| `headers` | boolean | | `true` | (CSV 전용) 첫 번째 행이 헤더인지 여부 |
| `delimiter` | string | | `","` | (CSV 전용) 컬럼 구분자 (단일 문자) |

#### Format Options

- **`json`**: JSON 배열
- **`jsonl`**: 줄바꿈으로 구분된 JSON
- **`csv`**: CSV 형식
- **`raw`**: 원시 바이트

#### Example

```toml
[[sources]]
name = "piped_data"
type = "stdin"

[sources.config]
format = "jsonl"
```

```bash
cat data.jsonl | conveyor run -c pipeline.toml
```

---

## Transforms

### Filter Transform

**Function:** `filter`

조건에 따라 행을 필터링합니다.

#### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `column` | string | ✓ | - | 필터링할 컬럼명 |
| `operator` | string | | `"=="` | 비교 연산자 |
| `value` | any | ✓ | - | 비교할 값 |

#### Supported Operators

| Operator | Description | Value Type | Example |
|----------|-------------|------------|---------|
| `==`, `=` | 같음 | string, number, boolean | `value = 100` |
| `!=`, `<>` | 다름 | string, number, boolean | `value = "inactive"` |
| `>` | 초과 | number | `value = 50.0` |
| `>=` | 이상 | number | `value = 18` |
| `<` | 미만 | number | `value = 100` |
| `<=` | 이하 | number | `value = 65` |
| `contains` | 문자열 포함 | string | `value = "test"` |
| `in` | 배열 내 포함 | array | `value = ["A", "B", "C"]` |

#### Examples

```toml
# 숫자 비교
[[transforms]]
name = "filter_high_value"
function = "filter"

[transforms.config]
column = "amount"
operator = ">="
value = 1000.0

# 문자열 일치
[[transforms]]
name = "filter_active"
function = "filter"

[transforms.config]
column = "status"
operator = "=="
value = "active"

# 문자열 포함
[[transforms]]
name = "filter_contains"
function = "filter"

[transforms.config]
column = "description"
operator = "contains"
value = "important"

# 배열 내 포함 검사
[[transforms]]
name = "filter_categories"
function = "filter"

[transforms.config]
column = "category"
operator = "in"
value = ["A", "B", "C"]
```

---

### Map Transform

**Function:** `map`

컬럼에 수식을 적용하여 새로운 컬럼을 생성하거나 기존 컬럼을 변환합니다.

#### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `expression` | string | ✓ | - | 계산 수식 |
| `output_column` | string | ✓ | - | 결과를 저장할 컬럼명 |

#### Supported Operations

| Operation | Description | Example |
|-----------|-------------|---------|
| `column * number` | 곱셈 | `price * 1.1` |
| `column + number` | 덧셈 | `quantity + 10` |
| `column - number` | 뺄셈 | `total - discount` |
| `column / number` | 나눗셈 | `total / 100` |
| `column / column` | 컬럼 간 나눗셈 | `revenue / cost` |

#### Examples

```toml
# 가격에 세금 추가
[[transforms]]
name = "add_tax"
function = "map"

[transforms.config]
expression = "price * 1.1"
output_column = "price_with_tax"

# 할인 적용
[[transforms]]
name = "apply_discount"
function = "map"

[transforms.config]
expression = "price - 10"
output_column = "discounted_price"

# 이익률 계산
[[transforms]]
name = "calculate_margin"
function = "map"

[transforms.config]
expression = "revenue / cost"
output_column = "profit_margin"
```

---

### Validate Schema Transform

**Function:** `validate_schema`

데이터 스키마의 유효성을 검증합니다. 검증 실패 시 파이프라인이 중단됩니다.

#### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `required_fields` | array[string] | | - | 필수 컬럼 목록 |
| `field_types` | table | | - | 컬럼 타입 맵 |
| `non_nullable` | array[string] | | - | null 값을 허용하지 않는 컬럼 목록 |
| `date_fields` | array[string] | | - | 날짜/시간 타입이어야 하는 컬럼 목록 |
| `unique_fields` | array[string] | | - | 중복 값을 허용하지 않는 컬럼 목록 |

#### Supported Types

| Type | Aliases | Description |
|------|---------|-------------|
| `string` | `str`, `text` | 문자열 |
| `int` | `integer`, `int64`, `i64` | 정수 (8/16/32/64비트) |
| `float` | `double`, `f64`, `float64` | 실수 (32/64비트) |
| `bool` | `boolean` | 불린 |
| `date` | - | 날짜 |
| `datetime` | `timestamp` | 날짜/시간 |

#### Example

```toml
[[transforms]]
name = "validate_schema"
function = "validate_schema"

[transforms.config]
# 필수 컬럼
required_fields = ["id", "name", "email", "created_at"]

# 타입 검증
[transforms.config.field_types]
id = "int"
name = "string"
email = "string"
age = "int"
salary = "float"
is_active = "boolean"
created_at = "datetime"

# null 불가 컬럼
non_nullable = ["id", "email"]

# 날짜 필드
date_fields = ["created_at", "updated_at"]

# 중복 불가 컬럼
unique_fields = ["id", "email"]
```

---

## Sinks

### CSV Sink

**Type:** `csv`

데이터를 CSV 파일로 저장합니다.

#### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `path` | string | ✓ | - | 출력 CSV 파일 경로 |
| `headers` | boolean | | `true` | 헤더 행 포함 여부 |
| `delimiter` | string | | `","` | 컬럼 구분자 (단일 문자) |

#### Example

```toml
[[sinks]]
name = "export_csv"
type = "csv"

[sinks.config]
path = "output/result.csv"
headers = true
delimiter = ","
```

---

### JSON Sink

**Type:** `json`

데이터를 JSON 파일로 저장합니다.

#### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `path` | string | ✓ | - | 출력 JSON 파일 경로 |
| `format` | string | | `"records"` | 출력 포맷: `records`, `jsonl`, `dataframe` |
| `pretty` | boolean | | `false` | 들여쓰기된 포맷으로 출력 |

#### Format Options

- **`records`**: JSON 배열 `[{...}, {...}]`
- **`jsonl`**: 줄바꿈으로 구분된 JSON 객체 (스트리밍에 적합)
- **`dataframe`**: 컬럼 기반 JSON `{"col1": [...], "col2": [...]}`

#### Example

```toml
[[sinks]]
name = "export_json"
type = "json"

[sinks.config]
path = "output/result.json"
format = "records"
pretty = true
```

---

### Stdout Sink

**Type:** `stdout`

데이터를 표준 출력으로 출력합니다. 파이프라인 결과를 터미널에서 확인하거나 다른 프로그램으로 전달할 때 사용합니다.

#### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `format` | string | | `"table"` | 출력 포맷: `table`, `json`, `jsonl`, `csv` |
| `limit` | integer | | - | 출력할 최대 행 수 |
| `pretty` | boolean | | `true` | (JSON 전용) 들여쓰기된 포맷으로 출력 |
| `delimiter` | string | | `","` | (CSV 전용) 컬럼 구분자 (단일 문자) |

#### Format Options

- **`table`**: ASCII 테이블 형식 (사람이 읽기 좋음)
- **`json`**: JSON 배열 형식
- **`jsonl`**: 줄바꿈으로 구분된 JSON (파이프라인에 적합)
- **`csv`**: CSV 형식

#### Example

```toml
# 테이블 형식으로 상위 10개 행 출력
[[sinks]]
name = "preview"
type = "stdout"

[sinks.config]
format = "table"
limit = 10

# JSON 형식으로 전체 출력
[[sinks]]
name = "json_output"
type = "stdout"

[sinks.config]
format = "json"
pretty = true

# JSONL 형식으로 파이프라인 연결
[[sinks]]
name = "pipe_output"
type = "stdout"

[sinks.config]
format = "jsonl"
```

```bash
# 다른 프로그램으로 파이프
conveyor run -c pipeline.toml | jq '.[] | select(.amount > 100)'
```

---

## See Also

- [README.md](../README.md) - Project overview and quick start
- [Examples](../examples/) - Sample pipeline configurations
- [CLAUDE.md](../CLAUDE.md) - Development notes and architecture details
