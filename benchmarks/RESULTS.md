# TokenPress 벤치마크 결과

측정일: 2026-07-31 (오픈 모델 tokenizer 측정 추가: 2026-08-01)
측정 바이너리: 이 파일이 포함된 커밋의 `cargo build --release -p tokenpress-cli`
측정 명령: `tokenpress stats <corpus> --tokenizer <name> [옵션]`
플랫폼: Windows 11, rustc 1.95.0

## Corpus (커밋 고정, `benchmarks/fetch.ps1`)

| 프로젝트 | 버전 | 커밋 | 대상 파일 |
|---|---|---|---|
| psf/requests | v2.32.3 | `0e322af8` | `.py` 36개 (src + tests + docs 스크립트) |
| BurntSushi/ripgrep | 14.1.1 | `4649aa97` | `.rs` 98개 (전체 crate) |

**전 파일이 검증(재파싱 + 토큰/AST 동등성)을 통과**했다. 검증 실패로 제외된
파일은 0개다.

## 어떤 LLM 기준인가 (tokenizer ↔ 모델 매핑)

| Tokenizer | 해당 모델 | 획득 방법 | 비고 |
|---|---|---|---|
| `o200k_base` | OpenAI GPT-4o, GPT-4.1, o1/o3/o4 시리즈 (Codex 계열 포함) | tiktoken 내장 | 로컬 실측 |
| `cl100k_base` | OpenAI GPT-4, GPT-3.5-turbo | tiktoken 내장 | 로컬 실측 |
| Qwen3.6 | Qwen/Qwen3.6-35B-A3B (rev `995ad96e`) | `--tokenizer hf:` + tokenizer.json | 로컬 실측 |
| GLM-5.2 | zai-org/GLM-5.2 (rev `b4734de4`) | `--tokenizer hf:` + tokenizer.json | 로컬 실측 |
| Kimi K3 | moonshotai/Kimi-K3 (rev `9f62e4e9`) | `--tokenizer kimi:` + tiktoken.model | tiktoken ranks + Kimi pat_str 로더로 로컬 실측 |

Claude는 vocab 비공개라 아직 미측정 — `count_tokens` API 실측이 필요하다.
**비공개 tokenizer 절감률을 아래 수치에서 외삽하지 말 것** (설계 원칙).
tokenizer 파일은 `benchmarks/fetch.ps1`이 위 revision으로 고정해 내려받는다.

## 결과

### 기본 설정 (주석·docstring·어노테이션·doc comment 유지, 인접 import 병합)

컨텍스트 손실 없는 설정 — 공백·빈 줄·들여쓰기 최소화 + PY09 import 병합만.

| Corpus | Tokenizer | Before | After | 절감 |
|---|---|---|---|---|
| requests | o200k_base | 86,922 | 79,070 | **-9.0%** |
| requests | cl100k_base | 86,531 | 78,703 | **-9.0%** |
| requests | Qwen3.6 | 94,786 | 85,525 | **-9.8%** |
| requests | GLM-5.2 | 86,791 | 78,961 | **-9.0%** |
| requests | Kimi K3 | 87,235 | 79,622 | **-8.7%** |
| ripgrep | o200k_base | 420,944 | 341,242 | **-18.9%** |
| ripgrep | cl100k_base | 419,272 | 342,663 | **-18.3%** |
| ripgrep | Qwen3.6 | 458,041 | 351,962 | **-23.2%** |
| ripgrep | GLM-5.2 | 419,526 | 342,819 | **-18.3%** |
| ripgrep | Kimi K3 | 420,393 | 343,904 | **-18.2%** |

### 공격적 설정 (컨텍스트 손실 감수)

* Python: `--py-strip-comments --py-strip-annotations`
  (주의: `__annotations__` 기반 introspection 깨짐 — RESULTS의 수치는 상한 참고용)
* Rust: `--rs-strip-doc-comments` (rustdoc·doctest 소실)

| Corpus | Tokenizer | Before | After | 절감 |
|---|---|---|---|---|
| requests | o200k_base | 86,922 | 69,012 | **-20.6%** |
| requests | cl100k_base | 86,531 | 68,619 | **-20.7%** |
| requests | Qwen3.6 | 94,786 | 74,882 | **-21.0%** |
| requests | GLM-5.2 | 86,791 | 68,839 | **-20.7%** |
| requests | Kimi K3 | 87,235 | 69,599 | **-20.2%** |
| ripgrep | o200k_base | 420,944 | 260,047 | **-38.2%** |
| ripgrep | cl100k_base | 419,272 | 259,429 | **-38.1%** |
| ripgrep | Qwen3.6 | 458,041 | 262,670 | **-42.7%** |
| ripgrep | GLM-5.2 | 419,526 | 259,578 | **-38.1%** |
| ripgrep | Kimi K3 | 420,393 | 261,072 | **-37.9%** |

## 해석

* Rust 절감률이 Python보다 높다: Rust는 줄바꿈이 문법에 관여하지 않아
  줄바꿈·들여쓰기를 전부 제거할 수 있고, `syn`이 일반 주석을 항상 버리기
  때문이다(MVP 제약). ripgrep은 doc comment 비중이 커서 공격적 설정의 폭이 크다.
* requests 기본 설정 -9.0%는 주석·docstring을 전부 유지한 수치다. requests는
  문서화 밀도가 높은 프로젝트라 유지 비용이 크다.
* tokenizer 간 차이가 실재한다 — 특히 **Qwen3.6은 ripgrep에서 -23.2%로
  o200k(-18.9%)보다 4.3%p 더 절감**된다. Qwen tokenizer가 들여쓰기·공백 런을
  상대적으로 비싸게 인코딩하기 때문에(같은 corpus의 Before가 45.8만 vs 42.1만)
  공백 제거의 이득이 더 크다. "문자가 아니라 토큰을 최적화한다"는 전제의
  가장 강한 근거 데이터.
* 컨텍스트 환산: ripgrep 전체 소스는 기본 설정만으로 128k 컨텍스트 기준
  약 3.3개 → 2.7개 컨텍스트 분량으로 줄어든다 (o200k 기준).

## 재현 방법

```powershell
.\benchmarks\fetch.ps1     # corpus + tokenizer 파일 다운로드 (revision 고정)
cargo build --release -p tokenpress-cli
.\target\release\tokenpress.exe stats benchmarks\corpus\requests --tokenizer o200k_base
.\target\release\tokenpress.exe stats benchmarks\corpus\ripgrep --tokenizer o200k_base
# 오픈 모델 tokenizer
.\target\release\tokenpress.exe stats benchmarks\corpus\ripgrep --tokenizer hf:benchmarks\tokenizers\qwen3.6.json
.\target\release\tokenpress.exe stats benchmarks\corpus\ripgrep --tokenizer hf:benchmarks\tokenizers\glm-5.2.json
.\target\release\tokenpress.exe stats benchmarks\corpus\ripgrep --tokenizer kimi:benchmarks\tokenizers\kimi-k3.tiktoken
# 공격적 설정
.\target\release\tokenpress.exe stats benchmarks\corpus\requests --tokenizer o200k_base --py-strip-comments --py-strip-annotations
.\target\release\tokenpress.exe stats benchmarks\corpus\ripgrep --tokenizer o200k_base --rs-strip-doc-comments
```

## 이 측정이 잡아낸 버그 (수정 완료)

첫 측정에서 requests 8파일, ripgrep 10파일이 검증 실패로 제외됐었다.
검증 계층이 손상된 출력을 파일에 쓰지 않고 오류로 보고한 덕분에 발견된
formatter 버그 3건:

1. **Python 괄호 내 trailing 주석**: `f(a,  # note` 뒤 연속 줄이 주석에
   삼켜짐 → 인라인 주석 뒤 강제 줄바꿈으로 수정.
2. **Rust 오픈 레인지 패턴**: `128.. =>`가 `..=>`로 붙어 `..=` + `>`로
   재해석 → glue pair `('.','=')` 추가.
3. **Rust 매크로 본문 spacing 메타데이터**: `vec![1, -2]` → `vec![1,-2]`에서
   `,`의 Joint/Alone 플래그가 바뀌어 과잉 검증 실패 → 토큰 구조 비교로 완화
   (토큰 내용·구조는 여전히 완전 일치 요구).

TODO: upstream 테스트 스위트(pytest / cargo test) 실행으로 동작 보존을
공개 증명 — corpus를 변환 후 테스트 돌리는 스크립트는 다음 단계.
