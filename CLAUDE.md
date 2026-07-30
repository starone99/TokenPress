# TokenPress 개발 규칙

## TDD (필수)

1. 새 동작은 **실패하는 테스트를 먼저** 작성한다 (red).
2. 테스트를 통과하는 최소 구현을 한다 (green).
3. 한 번 통과한 테스트는 깨지지 않게 유지한다 — 동작 변경 시 테스트를 먼저 고친다.

## 커버리지 게이트 (필수)

- 커밋 전 반드시 실행: `.\scripts\coverage.ps1` (Windows) / `./scripts/coverage.sh`
- **라인 커버리지 100% 미만이면 실패한다.** 신규 코드는 테스트 없이는 머지 불가.
- 유일한 예외: `crates/tokenpress-cli/src/main.rs` (계측 불가능한 얇은 진입점,
  로직 금지 — 모든 로직은 `cli.rs` 라이브러리에 두고 거기서 테스트한다).
- 커버 불가능한 방어 코드(`unreachable!` 등)는 작성하지 않는 방향으로 설계를
  바꾼다. 정말 불가피하면 이유를 주석으로 남기고 리뷰에서 논의한다.

## 빌드/테스트

- `cargo build --workspace` / `cargo test --workspace`
- 툴체인: rustc 1.95.0 (`rust-toolchain.toml`). Windows 로컬은 MSVC 호스트
  override가 걸려 있다 (`rustup override` — gnu 호스트 dlltool 문제 회피).
- Python 파서는 ruff 내부 crate(`=0.0.6` 정확히 pin, semver 보장 없음).
  파서 API 접근은 `tokenpress-python/src/parser.rs`에만 둔다.

## 설계 문서

- `docs/DESIGN.md` — 기술 설계 (git 제외, 로컬 전용)
- `docs/transforms/{python,rust}.md` — 언어별 변환 규칙 레퍼런스 (규칙 ID 인용)
- 핵심 불변 조건: **검증(재파싱+동등성)을 통과하지 못한 출력은 절대 쓰지 않는다.**
