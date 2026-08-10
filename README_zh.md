<p align="center">
  <strong>TokenPress</strong>
</p>

<p align="center">
  <strong>为 agentic coding 而生的格式化工具 —— 面向分词器优化，而非面向人类读者。</strong>
</p>

<p align="center">
  <a href="https://github.com/starone99/TokenPress/actions"><img src="https://github.com/starone99/TokenPress/workflows/CI/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/coverage-100%25-brightgreen.svg" alt="Coverage">
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="README_ko.md">한국어</a> ·
  <a href="README_ja.md">日本語</a> ·
  <b>中文</b> ·
  <a href="README_es.md">Español</a> ·
  <a href="README_fr.md">Français</a> ·
  <a href="README_pt.md">Português</a>
</p>

> 本翻译可能滞后。基准文档是 [README.md](README.md)；若数字或说法不一致，以英文
> 版本为准。

---

如果你在做 agentic coding，为什么还在用一个为人类读者设计的格式化工具？Black、
gofmt、rustfmt、Prettier 都在为人的眼睛优化 —— 行宽、对齐、条目之间的空行。当读者
是模型时，这些都不是价值，而是要计费的 token。

TokenPress 输出等价的程序，并使其输入 token 数最小：

```text
minimize  tokenizer.encode(transformed_code)
s.t.      变换后的代码仍能解析、编译，并且行为完全相同
```

它不是压缩器（minifier）—— 字符数和 token 数并不一致，因此所有变换都是针对真实的
分词器挑选的。**未通过校验的输出永远不会被写入**，标识符和字符串内容从不改动。

## 能省多少

每一行都是一个**真实的开源代码库**，在固定的提交上整体格式化，且所有文件都通过了
校验。实心条是*每一个*分词器都能省下的部分，浅色尾部是最有利的那个分词器能多走的
距离。

**Aggressive 设置** —— 会一并删除注释和 docstring 的可选标志：

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

**默认设置** —— 同样的代码库，不加任何标志。注释、docstring 和类型注解全部保留，
只去掉空白、空行和缩进：

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

注意顺序变了。tokio 在 aggressive 图中居首，是因为它的文档注释非常密集 —— 删掉
它们，半个仓库就没了。但在默认设置下它只排中游，因为剩下能删的只有空白。
**默认设置的数字是不需要付出任何代价的**，aggressive 的数字则是你主动选择的
取舍。

每一行内部的跨度同样是重点：节省率因分词器而异，这正是基准要测量六个的原因 ——
GLM-5.2、Kimi K3、Gemma 4、Qwen3.6、`o200k_base` 和 `cl100k_base`。其余五种语言，
每种一个代码库，aggressive，同一次运行：

```text
target codebase       0%   10%  20%  30%  40%  50%  60%
──────────────────────├────┼────┼────┼────┼────┼────┤
commons-lang (Java)   █████████████████████░░           -42.9 … -46.6%
express (JS)          █████████████░░░░                 -25.4 … -33.3%
rack (Ruby)           █████████░                        -18.2 … -20.8%
gin (Go)              █████████░                        -18.7 … -20.0%
```

**没有测量任何私有或闭源的分词器，这里的数字也不会外推到那些分词器上。**节省率
取决于一棵代码树中散文所占的比例，而不是它用什么语言写成；每种语言一个语料库只是
一个数据点，绝不是对该语言的普遍预期。十三个语料库、原始 token 数、按分词器分列的
表格，以及换行符相关的注意事项，都在
[benchmarks/RESULTS.md](benchmarks/RESULTS.md)，并由
[benchmarks/SHOWCASE.md](benchmarks/SHOWCASE.md) 汇总。

**删除注释和 docstring 会抹掉模型本可以利用的上下文，而这是否会降低它的回答质量
尚未测量。**节省是测量过的；质量上的取舍没有。请把 aggressive 标志当作一个选择，
而不是白拿的收益。

## 有人类会读这份代码吗？

这一个问题决定了该怎么用。

**会 —— 那就只对交给模型的副本运行，源码保持原样。**把格式化后的副本粘进提示词、
交给 agent 的上下文窗口、喂给 RAG 索引。即使在默认设置下也是如此：默认运行同样会
删掉空行、压缩缩进。这里的 "context-lossless" 是关于*模型*还能恢复什么的主张，
从来不是说人读起来舒服。

**不会 —— 没人读，仓库由 agent 编写和维护 —— 那么规范化源码本身是自洽的**，
pre-commit 钩子和 GitHub Action 正是为此存在。但有两件事要先知道，且都与人类读者
无关：

- **Rust 会把所有行合并。**在默认设置下，Rust 后端会把整个文件重新输出为一行。
  按行寻址的编辑工具、`git diff`、合并冲突和堆栈跟踪都会随之退化。其他后端保留
  换行。
- **Rust 和 JS/TS 在默认设置下也会丢失注释**，并且没有反向还原。在钩子下这不是
  一次性的转换 —— 之后任何人写下的注释都会在下一次运行时被删掉。

没有反向映射：没有 source map，也没有回打补丁。模型可以读格式化后的代码并回答
问题，但基于该副本生成的 diff 无法应用到未格式化的原文件上。**要让模型编辑的
文件，应当以未格式化的形式交给它。**

**这也是为什么没有编辑器插件、没有保存时格式化。**多数人正是通过这种方式接触
Black、Prettier、rustfmt，但它恰恰是 TokenPress 不该有的集成：在编辑器里打开的
文件，按定义就是有人正在读的文件。一个在保存时运行它的扩展，在上面这个问题所问的
那种情形里就是错的。

## 在项目中使用

和其他格式化工具一样，版本属于项目而不属于你的机器 —— 否则用着不同版本的两个人会
永远互相重新格式化对方的文件。把它固定在钩子或 Action 里，谁都不必单独安装。

**pre-commit** —— 框架会自行获取并构建被固定的版本：

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/starone99/TokenPress
    rev: v0.1.0                  # 固定版本才是重点 —— 只在有意时升级
    hooks:
      - id: tokenpress-check     # 不写任何东西；若有改动则失败
    # - id: tokenpress-format    # 会就地改写。请先读上面那个问题。
```

**GitHub Action** —— 在已有工作流中加一步：

```yaml
- uses: starone99/TokenPress@v0.1.0
  with:
    paths: src tests
    mode: check                  # `format` 会改写工作区
```

**`tokenpress.toml`** —— 按语言配置标志，从最近的上级目录中读取，因此钩子、Action
和你自己的运行看到的是同一份配置：

```toml
[python]
strip_comments = true
[rust]
strip_doc_comments = true
```

两种集成的默认都是 `check`，什么也不写。只在上述问题的「没人读」那一侧才使用
`format`。选项、标志与配置的完整对应关系以及 cargo feature，见
[INTEGRATIONS.md](docs/INTEGRATIONS.md)。

## 或者自己运行

一次性的用途 —— 测量一棵代码树，或生成即将交给模型的副本 —— 就安装 CLI。

```bash
# 安装脚本：下载适配本机的发行版，并在解压前用发行版的 SHA256SUMS 校验
curl -fsSL https://raw.githubusercontent.com/starone99/TokenPress/master/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/starone99/TokenPress/master/install.ps1 | iex
```

```bash
# 或者用 Rust 工具链
cargo install --git https://github.com/starone99/TokenPress tokenpress-cli
```

预构建归档和 `SHA256SUMS` 在
[发布页面](https://github.com/starone99/TokenPress/releases)，覆盖 Linux x86_64、
macOS（Apple Silicon）和 Windows x86_64；包括 Intel macOS 在内的其他平台请从源码构建。
`TOKENPRESS_VERSION` 用于固定标签，`TOKENPRESS_BIN_DIR` 用于更改安装位置。构建
Ruby、Go、Java 和 C# 后端需要 C 编译器，Ruby 还需要 libclang ——
`--no-default-features` 两者都不需要，`--features go,java` 只加回你点名的部分。

然后：

```bash
tokenpress stats  <PATH>...        # 能省多少 —— 不写任何东西
tokenpress diff   <PATH>...        # unified diff —— 不写任何东西
tokenpress format <PATH>...        # 就地改写（目录会递归遍历）
tokenpress check  <PATH>...        # 若有改动则 exit 1
```

从 `stats` 开始。它不碰任何东西，只告诉你这在你的代码树上是否划算：

```bash
tokenpress stats . --tokenizer o200k_base            # GPT-4o / o 系列（默认）
tokenpress stats . --tokenizer cl100k_base           # GPT-4 / GPT-3.5
tokenpress stats . --tokenizer hf:tokenizer.json     # 任意 HF 分词器（Qwen、GLM、Gemma…）
tokenpress stats . --tokenizer kimi:tiktoken.model   # Kimi ranks 格式
```

所有会丢失信息的行为都是可选标志，且各自写明会破坏什么：

```bash
--py-strip-comments        # 删除 # 注释
--py-strip-docstrings      # 清空 __doc__ —— help() 和 doctest 会失效
--py-strip-annotations     # 破坏 dataclass/pydantic 的运行时内省
--py-no-merge-imports      # 不合并相邻的 import
--rs-strip-doc-comments    # 删除 /// 和 //! —— rustdoc 和 doctest 一并消失
--js-strip-comments        # 连尚能保留的 JS/TS 注释也删除
--ruby-strip-comments      # 保留 shebang 和 magic comment
--go-strip-comments        # 保留 //go: 指令、构建约束和 cgo 前导块
--java-strip-comments      # 包括 Javadoc
--csharp-strip-comments    # 包括 /// XML 文档
```

退出码：`0` 正常 · `1` check 发现改动 · `2` 错误。解析和校验失败会按文件报告，
损坏的内容永远不会被写入。

## 工作原理

```text
  源码 ──▶ 解析 ──▶ 以最小 token 代价重新输出 ──▶ 校验 ──▶ 写入
                                                   │
                                     ┌─────────────┴─────────────┐
                                     │ 重新解析                  │
                                     │ AST / token 等价性        │
                                     │ 该语言自己的工具链        │  ← --verify external
                                     └─────────────┬─────────────┘
                                                   │
                                            失败 ──┴──▶ 文件保持不变
```

最后一步就是整个设计的要义。无法被证明等价的变换不会被写入，所以最坏的情况是文件
原封不动，而不是被破坏。

## 语言支持

**Python 和 Rust 是主要目标** —— 项目为它们而建，基准对它们覆盖最深，工作也优先
投向它们。其余五种同样在相同的不变式和相同的检查之上受支持，但各自只依托一个
语料库。

| 语言 | 扩展名 | 默认保留注释 | 外部校验 |
|---|---|---|---|
| **Python** | `.py` | ✅ | ❌ 仅内置检查 |
| **Rust** | `.rs` | ❌ `//` 和 `/* */` 始终丢失 | ❌ 仅内置检查 |
| JavaScript / TypeScript | `.js` `.mjs` `.cjs` `.jsx` `.ts` `.mts` `.cts` `.tsx` | ⚠️ 部分 | ✅ `tsc --noEmit` |
| Ruby | `.rb` `.rake` `.gemspec` `.ru`、`Gemfile`、`Rakefile` | ✅ | ✅ `ruby -c` |
| Go | `.go` | ✅ | ✅ `gofmt -e` |
| Java | `.java` | ✅ | ✅ `javac`，解析后即停 |
| C# | `.cs` | ✅ | ✅ Roslyn `csc` |

最后一列与上面那段话相抵触，这一点写在这里而不是藏起来：**两个主要语言恰恰是没有
外部校验的两个。**Python 和 Rust 只有内置检查。这是本项目最薄弱之处，补上它是
[路线图](ROADMAP.md)的第一项。

各语言的细节 —— 每个后端保留什么、做不到什么，以及外部检查器如何被调用 —— 见
[LANGUAGES.md](docs/LANGUAGES.md)。

## 文档

| | |
|---|---|
| [LANGUAGES.md](docs/LANGUAGES.md) | 各语言的支持情况、注意事项与外部检查器 |
| [INTEGRATIONS.md](docs/INTEGRATIONS.md) | pre-commit、GitHub Action、配置文件、cargo feature |
| [CHANGELOG.md](CHANGELOG.md) | 变更记录，影响输出的条目已标注 |
| [benchmarks/RESULTS.md](benchmarks/RESULTS.md) | 完整方法论、十三个语料库、六个分词器 |
| [benchmarks/SHOWCASE.md](benchmarks/SHOWCASE.md) | 摘要，以及按分词器分列的 ≥40% 候选 |
| [ROADMAP.md](ROADMAP.md) | 接下来做什么，以及尚未决定的问题 |
| [CONTRIBUTING.md](CONTRIBUTING.md) | 构建、测试，以及各后端所需的工具链 |

## 开发

TDD 加硬性关卡：`scripts/coverage.ps1`（Windows）/ `scripts/coverage.sh` 会在行
覆盖率低于 100% 时让构建失败。CI 会运行 fmt、clippy `-D warnings`、Linux 与
Windows 上的测试，以及覆盖率关卡。规则见
[CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证

Apache License, Version 2.0（[LICENSE](LICENSE) 或
<https://www.apache.org/licenses/LICENSE-2.0>）。

除非你明确另行声明，否则按 Apache-2.0 许可证的定义，你有意提交并纳入本作品的任何
贡献，都将按上述方式授权，不附加任何其他条件。
