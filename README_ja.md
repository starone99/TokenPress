<p align="center">
  <strong>TokenPress</strong>
</p>

<p align="center">
  <strong>エージェンティックコーディングのためのフォーマッタ — 人間ではなくトークナイザに最適化します。</strong>
</p>

<p align="center">
  <a href="https://github.com/starone99/TokenPress/actions"><img src="https://github.com/starone99/TokenPress/workflows/CI/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/coverage-100%25-brightgreen.svg" alt="Coverage">
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="README_ko.md">한국어</a> ·
  <b>日本語</b> ·
  <a href="README_zh.md">中文</a> ·
  <a href="README_es.md">Español</a> ·
  <a href="README_fr.md">Français</a> ·
  <a href="README_pt.md">Português</a>
</p>

> この翻訳は古くなっている場合があります。正典は [README.md](README.md) です。
> 数値や記述が食い違う場合は英語版が正しいものとしてください。

---

エージェンティックコーディングをしているのに、なぜ人間の読者向けに作られた
フォーマッタを使い続けているのでしょうか。Black、gofmt、rustfmt、Prettier は
すべて人間の目に最適化しています — 行幅、揃え、要素間の空行。読み手がモデルで
あれば、そのどれも価値ではありません。課金されるトークンです。

TokenPress は、同じプログラムを最小の入力トークンで表現します:

```text
minimize  tokenizer.encode(transformed_code)
s.t.      変換後のコードがパースでき、コンパイルでき、同一に振る舞うこと
```

ミニファイアではありません — 文字数とトークン数は一致しないため、変換は実際の
トークナイザを基準に選ばれます。**検証に失敗した出力は決して書き込まれません**。
識別子と文字列の内容には一切触れません。

## どれだけ削減できるか

各行は**実在のオープンソースコードベース**で、コミットを固定した状態で全体を
フォーマットし、すべてのファイルが検証を通過した結果です。塗りつぶしの棒は
*すべての*トークナイザが保証する削減量、薄い部分は最も有利なトークナイザが
さらに伸ばす分です。

**Aggressive 設定** — コメントと docstring も削除するオプトインのフラグ:

```text
target codebase       0%   10%  20%  30%  40%  50%  60%
──────────────────────├────┼────┼────┼────┼────┼────┤
tokio (Rust)          █████████████████████████░░░      -50.5 … -55.2%
ripgrep (Rust)        ███████████████████░░             -37.3 … -42.7%
langchain (Python)    ███████████████████░░             -37.1 … -41.1%
fastapi (Python)      ██████████████████░░              -36.1 … -40.1%
requests (Python)     ████████████████░░                -31.9 … -36.5%
transformers (Python) ███████████████░░░                -30.3 … -36.1%
uv (Rust + Python)    ███████████░                      -21.4 … -24.7%
django (Python)       ██████████░░                      -20.7 … -24.8%
```

**デフォルト設定** — 同じコードベース、フラグなし。コメント、docstring、型
アノテーションはすべて保持され、空白・空行・インデントだけが取り除かれます:

```text
target codebase       0%   10%  20%  30%  40%  50%  60%
──────────────────────├────┼────┼────┼────┼────┼────┤
fastapi (Python)      ███████████░░                     -21.6 … -26.7%
ripgrep (Rust)        ████████░░░                       -16.6 … -22.8%
uv (Rust + Python)    ███████░                          -13.1 … -16.8%
langchain (Python)    ██████░░                          -12.2 … -15.6%
tokio (Rust)          ██████░░░░                        -11.5 … -19.1%
django (Python)       █████░                            -9.8 … -12.6%
requests (Python)     ████░                             -7.3 … -9.7%
transformers (Python) ████░                             -7.0 … -10.3%
```

順位が入れ替わっている点に注目してください。tokio が aggressive で首位なのは
doc コメントが密だからで、それを削ればリポジトリの半分が消えます。しかしデフォルト
設定では中位です。残っているのが空白だけだからです。**デフォルトの数値は何も
代償を払わない削減**であり、aggressive の数値はあなたが選ぶトレードオフです。

各行の幅もまた要点です。削減率はトークナイザごとに異なり、だからこそベンチマークは
6 種類を測定します — GLM-5.2、Kimi K3、Gemma 4、Qwen3.6、`o200k_base`、
`cl100k_base`。残る 5 言語はコードベース 1 つずつ、aggressive、同一実行:

```text
target codebase       0%   10%  20%  30%  40%  50%  60%
──────────────────────├────┼────┼────┼────┼────┼────┤
commons-lang (Java)   █████████████████████░░           -42.9 … -46.6%
express (JS)          █████████████░░░░                 -25.4 … -33.3%
rack (Ruby)           █████████░                        -18.2 … -20.8%
gin (Go)              █████████░                        -18.7 … -20.0%
```

**非公開・クローズドなトークナイザは測定しておらず、ここのどの数値もそちらへ
外挿していません。**削減率は言語ではなく、ツリーに占める散文の割合に従います。
言語あたり 1 コーパスはデータ点であって、言語全体への期待値ではありません。
13 コーパス、生のトークン数、トークナイザ別の表、改行コードに関する注意は
[benchmarks/RESULTS.md](benchmarks/RESULTS.md) にあり、
[benchmarks/SHOWCASE.md](benchmarks/SHOWCASE.md) が要約しています。

**コメントと docstring を削除すると、モデルが参照できたはずの文脈が失われます。
それが回答の品質を下げるかどうかは測定していません。**削減は測定済み、品質の
トレードオフは未測定です。aggressive フラグは無料ではなく、選択として扱って
ください。

## このコードを人間が読みますか？

この 1 つの問いが使い方を決めます。

**読む — モデルに渡すコピーにだけ実行し、ソースはそのままにしてください。**
プロンプトに貼り、エージェントのコンテキストウィンドウに渡し、RAG インデックスに
入れる。デフォルト設定でも同じです。デフォルトの実行でも空行は消え、インデントは
詰められます。ここでの "context-lossless" は*モデル*が復元できる情報についての
主張であって、人間が読みやすいという意味ではありません。

**読まない — 誰も読まず、リポジトリをエージェントが書き、保守している — なら
ソース自体を正規化するのは筋が通ります。**pre-commit フックと GitHub Action は
そのためにあります。ただし先に知っておくべきことが 2 つあり、どちらも人間の
読者とは無関係です:

- **Rust はすべての行をつなげます。**デフォルト設定で Rust バックエンドは
  ファイル全体を 1 行として再出力します。行単位でアドレスする編集ツール、
  `git diff`、マージコンフリクト、スタックトレースがいずれも役に立たなくなります。
  他のバックエンドは改行を保ちます。
- **Rust と JS/TS はデフォルト設定でもコメントを失い**、元に戻す機能はありません。
  フックの下では一度きりの変換ではなく、以後に書かれたコメントも次の実行で
  消えます。

逆マッピングはありません。ソースマップもパッチバックもありません。モデルが
フォーマット済みのコードを読んで答えることはできますが、そのコピーに対する差分は
未フォーマットの原本には適用できません。**モデルが編集するファイルは、
フォーマットせずに渡してください。**

**エディタプラグインと format-on-save がないのも同じ理由です。**多くの人が
Black・Prettier・rustfmt に出会う経路がそれですが、TokenPress にあってはならない
統合です。エディタで開いているファイルは、定義上、人間が読んでいるファイルです。
保存のたびにこれを走らせる拡張は、上の問いが尋ねているまさにその場合に誤りです。

## プロジェクトに組み込む

他のフォーマッタと同じく、バージョンはあなたのマシンではなくプロジェクトに
属します — さもなければ異なるバージョンを使う二人が、互いのファイルを永遠に
再フォーマットし続けます。フックか Action に固定すれば、誰も個別にインストール
する必要はありません。

**pre-commit** — フレームワークが固定されたリビジョンを自ら取得してビルドします:

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/starone99/TokenPress
    rev: v0.1.0                  # 固定こそが要点 — 意図的にだけ上げること
    hooks:
      - id: tokenpress-check     # 何も書かず、変更が生じるなら失敗
    # - id: tokenpress-format    # 上書きします。上の問いを先に読んでください。
```

**GitHub Action** — 既存のワークフローに 1 ステップ:

```yaml
- uses: starone99/TokenPress@v0.1.0
  with:
    paths: src tests
    mode: check                  # `format` はワークスペースを書き換えます
```

**`tokenpress.toml`** — 言語ごとのフラグ。親ディレクトリをたどって見つけるので、
フックと Action と手元の実行がすべて同じ設定を見ます:

```toml
[python]
strip_comments = true
[rust]
strip_doc_comments = true
```

どちらの統合もデフォルトは `check` で、何も書きません。`format` は上の問いの
「誰も読まない」側でのみ使ってください。オプション、フラグと設定の対応、cargo
フィーチャは [INTEGRATIONS.md](docs/INTEGRATIONS.md) にあります。

## 自分で実行する

一度きりの用途 — ツリーを測る、モデルに渡すコピーを作る — には CLI を
インストールします。

```bash
# インストールスクリプト: ホストに合うリリースを取得し、展開前にリリースの SHA256SUMS で検証します
curl -fsSL https://raw.githubusercontent.com/starone99/TokenPress/master/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/starone99/TokenPress/master/install.ps1 | iex
```

```bash
# Rust ツールチェインがあるなら
cargo install --git https://github.com/starone99/TokenPress tokenpress-cli
```

ビルド済みアーカイブと `SHA256SUMS` は
[リリースページ](https://github.com/starone99/TokenPress/releases) に Linux
x86_64、macOS（Apple Silicon）、Windows x86_64 向けがあります。Intel macOS を含むその他の
プラットフォームはソースからビルドします。`TOKENPRESS_VERSION` でタグを固定し、
`TOKENPRESS_BIN_DIR` でインストール先を変更できます。Ruby・Go・Java・C#
バックエンドのビルドには C コンパイラが、Ruby にはさらに libclang が必要です。
`--no-default-features` はどちらも不要で、`--features go,java` で必要なものだけ
戻せます。

そのうえで:

```bash
tokenpress stats  <PATH>...        # どれだけ減るか — 何も書きません
tokenpress diff   <PATH>...        # unified diff — 何も書きません
tokenpress format <PATH>...        # その場で書き換え（ディレクトリは再帰）
tokenpress check  <PATH>...        # 変更が生じるなら exit 1
```

まず `stats` から。何にも触れず、あなたのツリーで割に合うかを教えてくれます:

```bash
tokenpress stats . --tokenizer o200k_base            # GPT-4o / o-series（既定）
tokenpress stats . --tokenizer cl100k_base           # GPT-4 / GPT-3.5
tokenpress stats . --tokenizer hf:tokenizer.json     # 任意の HF トークナイザ（Qwen, GLM, Gemma…）
tokenpress stats . --tokenizer kimi:tiktoken.model   # Kimi ranks 形式
```

情報を失うものはすべてオプトインのフラグで、それぞれ何が壊れるかを明示します:

```bash
--py-strip-comments        # # コメントを削除
--py-strip-docstrings      # __doc__ が空に — help() と doctest が壊れます
--py-strip-annotations     # dataclass/pydantic のランタイム内省が壊れます
--py-no-merge-imports      # 隣接する import をまとめない
--rs-strip-doc-comments    # /// と //! を削除 — rustdoc と doctest も一緒に
--js-strip-comments        # 生き残る JS/TS コメントも削除
--ruby-strip-comments      # シバンとマジックコメントは保持
--go-strip-comments        # //go: ディレクティブ、ビルド制約、cgo プリアンブルは保持
--java-strip-comments      # Javadoc を含む
--csharp-strip-comments    # /// XML ドキュメントを含む
```

終了コード: `0` 正常 · `1` check が変更を検出 · `2` エラー。パースと検証の失敗は
ファイル単位で報告され、壊れたものが書き込まれることは決してありません。

## 仕組み

```text
  ソース ──▶ パース ──▶ 最小トークンで再出力 ──▶ 検証 ──▶ 書き込み
                                                   │
                                     ┌─────────────┴─────────────┐
                                     │ 再パース                  │
                                     │ AST / トークン等価性      │
                                     │ その言語自身のツール      │  ← --verify external
                                     └─────────────┬─────────────┘
                                                   │
                                          失敗 ────┴──▶ ファイルはそのまま
```

最後の段階がこの設計のすべてです。等価だと証明できない変換は書き込まれないので、
最悪の場合でもファイルが手つかずで残るだけであり、壊れることはありません。

## 言語サポート

**Python と Rust が主力です** — このプロジェクトはそのために作られ、ベンチマークが
最も深く扱い、作業が最初に向かう対象です。残る 5 つも同じ不変条件と同じ検査の上で
サポートされますが、それぞれコーパス 1 つに依っています。

| 言語 | 拡張子 | 既定でコメント保持 | 外部検証 |
|---|---|---|---|
| **Python** | `.py` | ✅ | ❌ 内蔵チェックのみ |
| **Rust** | `.rs` | ❌ `//` と `/* */` は常に失われる | ❌ 内蔵チェックのみ |
| JavaScript / TypeScript | `.js` `.mjs` `.cjs` `.jsx` `.ts` `.mts` `.cts` `.tsx` | ⚠️ 部分的 | ✅ `tsc --noEmit` |
| Ruby | `.rb` `.rake` `.gemspec` `.ru`, `Gemfile`, `Rakefile` | ✅ | ✅ `ruby -c` |
| Go | `.go` | ✅ | ✅ `gofmt -e` |
| Java | `.java` | ✅ | ✅ `javac`（パース後に停止） |
| C# | `.cs` | ✅ | ✅ Roslyn `csc` |

最後の列は直前の段落と噛み合いません。それを埋もれさせずここに書きます:
**主力の 2 言語が、外部検証を持たない 2 言語です。**Python と Rust は内蔵チェック
のみです。これがこのプロジェクトで最も弱い点であり、それを埋めることが
[ロードマップ](ROADMAP.md)の最初の項目です。

言語ごとの詳細 — 各バックエンドが何を保ち何ができないか、外部チェッカがどう
呼ばれるか — は [LANGUAGES.md](docs/LANGUAGES.md) にあります。

## ドキュメント

| | |
|---|---|
| [LANGUAGES.md](docs/LANGUAGES.md) | 言語別のサポート、注意点、外部チェッカ |
| [INTEGRATIONS.md](docs/INTEGRATIONS.md) | pre-commit、GitHub Action、設定ファイル、cargo フィーチャ |
| [CHANGELOG.md](CHANGELOG.md) | 変更履歴。出力に影響する項目には印 |
| [benchmarks/RESULTS.md](benchmarks/RESULTS.md) | 方法論の全体、13 コーパス、6 トークナイザ |
| [benchmarks/SHOWCASE.md](benchmarks/SHOWCASE.md) | 要約と、トークナイザ別の ≥40% 候補 |
| [ROADMAP.md](ROADMAP.md) | 次に何をするか、開いている問い |
| [CONTRIBUTING.md](CONTRIBUTING.md) | ビルドとテスト、バックエンドごとに必要なツールチェイン |

## 開発

TDD とハードゲート: `scripts/coverage.ps1`（Windows）/ `scripts/coverage.sh` は
行カバレッジ 100% 未満でビルドを失敗させます。CI は fmt、clippy `-D warnings`、
Linux と Windows でのテスト、カバレッジゲートを実行します。規則は
[CONTRIBUTING.md](CONTRIBUTING.md) にあります。

## ライセンス

Apache License, Version 2.0（[LICENSE](LICENSE) または
<https://www.apache.org/licenses/LICENSE-2.0>）。

明示的に別段の意思表示をしない限り、Apache-2.0 ライセンスに定義されるとおり、
本著作物に含めることを意図して提出されたあなたの貢献は、追加の条件なしに上記の
とおりライセンスされます。
