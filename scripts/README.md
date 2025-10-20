# Plugin Deployment Scripts

## Manual Plugin Deployment

GitHub Actions가 작동하지 않거나 수동 배포가 필요할 때 사용하는 스크립트입니다.

### 사용법

```bash
# 기본 사용 (현재 버전 v0.11.0)
./scripts/deploy-plugins.sh

# 특정 버전 지정
./scripts/deploy-plugins.sh v0.12.0
```

### 스크립트가 하는 일

1. **GitHub Release 확인/생성**
   - 지정된 버전의 릴리즈가 없으면 draft로 생성

2. **플러그인 빌드**
   - HTTP plugin (release 모드)
   - MongoDB plugin (release 모드)

3. **체크섬 생성**
   - SHA256 해시 계산
   - `.sha256` 파일 생성

4. **GitHub Release에 업로드**
   - `gh` CLI 사용
   - 플러그인 dylib 파일
   - 체크섬 파일

5. **registry.json 업데이트**
   - 체크섬 자동 삽입
   - 다운로드 URL 업데이트

### 실행 후 작업

스크립트 실행 후:

```bash
# 1. registry.json 확인
cat registry.json

# 2. 커밋 및 푸시
git add registry.json
git commit -m "chore: update registry with checksums for v0.11.0"
git push

# 3. GitHub에서 draft release를 publish
# https://github.com/yoonhoGo/conveyor/releases
```

### 필요한 도구

- `gh` CLI (GitHub CLI)
  ```bash
  brew install gh
  gh auth login
  ```

- Rust toolchain (cargo)

### 문제 해결

**gh CLI 인증 오류:**
```bash
gh auth login
```

**빌드 실패:**
```bash
# 의존성 확인
cargo check

# 캐시 정리
cargo clean
cargo build --release
```

**업로드 실패:**
```bash
# 권한 확인
gh auth status

# 수동 업로드
gh release upload v0.11.0 target/release/libconveyor_plugin_http.dylib --repo yoonhoGo/conveyor
```

## 자동화된 배포 (GitHub Actions)

정상적으로 작동하는 경우 GitHub Actions가 자동으로 처리합니다:

1. 태그 푸시: `git push --tags`
2. Actions가 자동 실행
3. 플러그인 빌드 및 업로드
4. registry.json을 수동으로 업데이트하여 커밋

`.github/workflows/release-plugins.yml` 참고
