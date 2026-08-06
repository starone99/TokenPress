<p align="center">
  <strong>TokenPress</strong>
</p>

<p align="center">
  <strong>Um formatador para agentic coding: otimizado para o tokenizador, não para o leitor humano.</strong>
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
  <a href="README_zh.md">中文</a> ·
  <a href="README_es.md">Español</a> ·
  <a href="README_fr.md">Français</a> ·
  <b>Português</b>
</p>

> Esta tradução pode estar defasada. O documento de referência é
> [README.md](README.md); se um número ou uma afirmação divergir, vale a versão
> em inglês.

---

Se você faz agentic coding, por que ainda usa um formatador construído para um
leitor humano? Black, gofmt, rustfmt e Prettier otimizam todos para os olhos de
uma pessoa: largura de linha, alinhamento, linhas em branco entre as coisas.
Quando o leitor é um modelo, nada disso é valor. São tokens cobrados.

O TokenPress emite o programa equivalente que custa menos tokens de entrada:

```text
minimize  tokenizer.encode(transformed_code)
s.t.      o código transformado analisa, compila e se comporta de forma idêntica
```

Não é um minificador: contagem de caracteres e contagem de tokens não
coincidem, então as transformações são escolhidas contra um tokenizador real.
**Saída que falha na verificação nunca é escrita**, e identificadores e o
conteúdo de strings jamais são tocados.

## Quanto economiza

Cada linha é uma **base de código open source real**, formatada por inteiro em
um commit fixado, com todos os arquivos passando na verificação. A barra sólida
é o que *qualquer* tokenizador economiza; a cauda sombreada é o quanto o mais
favorável vai além.

**Configurações agressivas** — as flags opcionais que também removem
comentários e docstrings:

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

**Configurações padrão** — as mesmas bases de código, sem flag nenhuma.
Comentários, docstrings e anotações de tipo são todos preservados; só somem
espaços, linhas em branco e indentação:

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

Repare que a ordem muda. O tokio lidera o gráfico agressivo porque é denso em
doc comments — tire-os e metade do repositório some — mas nas configurações
padrão fica no meio, porque o que sobra para remover é só espaço em branco.
**Os números padrão são os que não custam nada**; os agressivos são uma troca
que você escolhe.

A amplitude dentro de cada linha é o outro ponto: a economia depende do
tokenizador, e por isso o benchmark mede seis — GLM-5.2, Kimi K3, Gemma 4,
Qwen3.6, `o200k_base` e `cl100k_base`. As outras cinco linguagens suportadas,
uma base de código cada, agressivo, mesma execução:

```text
target codebase       0%   10%  20%  30%  40%  50%  60%
──────────────────────├────┼────┼────┼────┼────┼────┤
commons-lang (Java)   █████████████████████░░           -42.9 … -46.6%
express (JS)          █████████████░░░░                 -25.4 … -33.3%
rack (Ruby)           █████████░                        -18.2 … -20.8%
gin (Go)              █████████░                        -18.7 … -20.0%
```

**Nenhum tokenizador privado ou fechado foi medido, e nenhum número aqui é
extrapolado para um deles.** A economia acompanha quanto de uma árvore é prosa,
não a linguagem em que ela está escrita; um corpus por linguagem é um ponto de
dado, nunca uma expectativa para a linguagem inteira. Treze corpora, contagens
brutas de tokens, tabelas por tokenizador e as ressalvas sobre fim de linha
estão em [benchmarks/RESULTS.md](benchmarks/RESULTS.md), resumidos em
[benchmarks/SHOWCASE.md](benchmarks/SHOWCASE.md).

**Remover comentários e docstrings apaga contexto que o modelo poderia ter
usado, e se isso degrada as respostas dele não foi medido.** A economia está
medida; a troca em qualidade não. Trate as flags agressivas como uma escolha,
não como ganho de graça.

## Algum humano lê este código?

Uma única pergunta decide como usar isto.

**Sim — formate a cópia que você entrega ao modelo e deixe seu fonte em paz.**
Cole num prompt, passe para a janela de contexto de um agente, indexe para RAG.
Isso vale mesmo nas configurações padrão: a execução padrão também remove
linhas em branco e comprime a indentação. "Context-lossless" aqui é uma
afirmação sobre o que um *modelo* consegue recuperar, nunca sobre o que uma
pessoa gosta de ler.

**Não — ninguém lê, o repositório é escrito e mantido por agentes — então
normalizar o próprio fonte é coerente**, e o hook de pre-commit e a GitHub
Action existem para isso. Duas coisas para saber antes, nenhuma sobre leitores
humanos:

- **Rust junta todas as linhas.** Nas configurações padrão o backend de Rust
  reemite um arquivo inteiro como uma única linha, então ferramentas de edição
  que endereçam por linha, `git diff`, conflitos de merge e stack traces se
  degradam. Os outros backends preservam as quebras de linha.
- **Rust e JS/TS perdem comentários no padrão**, e não existe des-formatação.
  Sob um hook isso não é uma conversão única: todo comentário escrito depois é
  removido na execução seguinte.

Não há mapeamento reverso: nem source map, nem reaplicação de patch. Um modelo
pode ler código formatado e responder sobre ele, mas um diff contra essa cópia
não se aplica ao original não formatado. **Um arquivo que um modelo vai editar
deve ser entregue a ele não formatado.**

**É também por isso que não há plugin de editor nem format-on-save.** É assim
que a maioria conhece Black, Prettier ou rustfmt, e é a única integração que o
TokenPress não deveria ter: o arquivo aberto no seu editor é, por definição, um
que uma pessoa está lendo. Uma extensão que rodasse isto ao salvar estaria
errada exatamente no caso que a pergunta acima levanta.

## Use no seu projeto

Como qualquer outro formatador, a versão pertence ao projeto e não à sua
máquina — caso contrário duas pessoas em versões diferentes reformatam os
arquivos uma da outra para sempre. Fixe-o num hook ou numa Action e ninguém
precisa instalar nada.

**pre-commit** — o framework busca e compila sozinho a revisão fixada:

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/starone99/TokenPress
    rev: v0.1.0                  # o pin é o ponto — suba-o deliberadamente
    hooks:
      - id: tokenpress-check     # não escreve nada; falha se algo mudaria
    # - id: tokenpress-format    # reescreve no lugar. Leia antes a pergunta acima.
```

**GitHub Action** — um passo num workflow que você já tem:

```yaml
- uses: starone99/TokenPress@v0.1.0
  with:
    paths: src tests
    mode: check                  # `format` reescreve o workspace
```

**`tokenpress.toml`** — flags por linguagem, lidas do diretório pai mais
próximo, de modo que o hook, a Action e suas próprias execuções concordem:

```toml
[python]
strip_comments = true
[rust]
strip_doc_comments = true
```

`check` é o padrão nas duas integrações e não escreve nada. Use `format` apenas
no lado "ninguém lê este código" da pergunta acima. As opções, o mapeamento
completo entre flags e configuração e as cargo features estão em
[INTEGRATIONS.md](docs/INTEGRATIONS.md).

## Ou rode você mesmo

Para um uso pontual — medir uma árvore, ou gerar a cópia que você vai entregar
a um modelo — instale a CLI.

```bash
# script de instalação: baixa a release para o seu host e a verifica contra o
# SHA256SUMS da release antes de extrair qualquer coisa
curl -fsSL https://raw.githubusercontent.com/starone99/TokenPress/master/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/starone99/TokenPress/master/install.ps1 | iex
```

```bash
# ou com um toolchain Rust
cargo install --git https://github.com/starone99/TokenPress tokenpress-cli
```

Os arquivos pré-compilados e o `SHA256SUMS` estão na
[página de releases](https://github.com/starone99/TokenPress/releases) para
Linux x86_64, macOS (Apple Silicon e Intel) e Windows x86_64; qualquer outra
plataforma compila a partir do fonte. `TOKENPRESS_VERSION` fixa uma tag e
`TOKENPRESS_BIN_DIR` muda onde o script instala. Compilar os backends de Ruby,
Go, Java e C# exige um compilador C, e libclang para Ruby —
`--no-default-features` não precisa de nenhum dos dois, e `--features go,java`
traz de volta só o que você nomear.

Depois:

```bash
tokenpress stats  <PATH>...        # quanto economizaria — não escreve nada
tokenpress diff   <PATH>...        # diff unificado — não escreve nada
tokenpress format <PATH>...        # reescreve no lugar (diretórios são percorridos)
tokenpress check  <PATH>...        # exit 1 se algo mudaria
```

Comece pelo `stats`. Ele não toca em nada e diz se isto vale a pena na sua
árvore:

```bash
tokenpress stats . --tokenizer o200k_base            # GPT-4o / série o (padrão)
tokenpress stats . --tokenizer cl100k_base           # GPT-4 / GPT-3.5
tokenpress stats . --tokenizer hf:tokenizer.json     # qualquer tokenizador HF (Qwen, GLM, Gemma…)
tokenpress stats . --tokenizer kimi:tiktoken.model   # formato Kimi ranks
```

Tudo que perde informação é uma flag opcional, e cada uma diz o que quebra:

```bash
--py-strip-comments        # remove comentários #
--py-strip-docstrings      # esvazia __doc__ — quebra help() e doctests
--py-strip-annotations     # quebra a introspecção de dataclass/pydantic
--py-no-merge-imports      # não funde imports adjacentes
--rs-strip-doc-comments    # remove /// e //! — junto com rustdoc e doctests
--js-strip-comments        # remove até os comentários JS/TS que sobreviviam
--ruby-strip-comments      # preserva shebang e magic comments
--go-strip-comments        # preserva diretivas //go:, build constraints e o preâmbulo cgo
--java-strip-comments      # Javadoc incluído
--csharp-strip-comments    # documentação XML /// incluída
```

Códigos de saída: `0` ok · `1` o check encontrou mudanças · `2` erro. Falhas de
parsing e de verificação são reportadas por arquivo, e nada corrompido é
escrito.

## Como funciona

```text
  fonte ──▶ parse ──▶ reemissão ao menor custo em tokens ──▶ verificar ──▶ escrever
                                                                │
                                                  ┌─────────────┴─────────────┐
                                                  │ re-parse                  │
                                                  │ equivalência AST / tokens │
                                                  │ o toolchain da linguagem  │  ← --verify external
                                                  └─────────────┬─────────────┘
                                                                │
                                                       falha ───┴──▶ arquivo fica intacto
```

O último passo é todo o projeto. Uma transformação que não pode ser provada
equivalente não é escrita, então o pior caso é um arquivo deixado como estava,
nunca um arquivo corrompido.

## Linguagens suportadas

**Python e Rust são os alvos principais** — é para isso que o projeto foi
construído, é o que os benchmarks cobrem mais a fundo, e é para lá que o
trabalho vai primeiro. As outras cinco são suportadas sobre o mesmo invariante
e as mesmas verificações, mas cada uma se apoia num único corpus.

| Linguagem | Extensões | Mantém comentários no padrão | Verificação externa |
|---|---|---|---|
| **Python** | `.py` | ✅ | ❌ só a verificação interna |
| **Rust** | `.rs` | ❌ `//` e `/* */` sempre se perdem | ❌ só a verificação interna |
| JavaScript / TypeScript | `.js` `.mjs` `.cjs` `.jsx` `.ts` `.mts` `.cts` `.tsx` | ⚠️ parcial | ✅ `tsc --noEmit` |
| Ruby | `.rb` `.rake` `.gemspec` `.ru`, `Gemfile`, `Rakefile` | ✅ | ✅ `ruby -c` |
| Go | `.go` | ✅ | ✅ `gofmt -e` |
| Java | `.java` | ✅ | ✅ `javac`, parado após o parse |
| C# | `.cs` | ✅ | ✅ Roslyn `csc` |

Essa última coluna contradiz o parágrafo acima dela, e isso é dito aqui em vez
de ser enterrado: **as duas linguagens principais são as duas sem verificação
externa.** Python e Rust têm apenas a verificação interna. É o ponto mais fraco
do projeto, e fechá-lo é o primeiro item do [roadmap](ROADMAP.md).

O detalhe por linguagem — o que cada backend preserva, o que não consegue
fazer, e como cada verificador externo é invocado — está em
[LANGUAGES.md](docs/LANGUAGES.md).

## Documentação

| | |
|---|---|
| [docs/LANGUAGES.md](docs/LANGUAGES.md) | Suporte por linguagem, ressalvas e verificadores externos |
| [docs/INTEGRATIONS.md](docs/INTEGRATIONS.md) | pre-commit, GitHub Action, arquivo de configuração, cargo features |
| [CHANGELOG.md](CHANGELOG.md) | O que mudou, com as entradas que afetam a saída marcadas |
| [benchmarks/RESULTS.md](benchmarks/RESULTS.md) | Metodologia completa, treze corpora, seis tokenizadores |
| [benchmarks/SHOWCASE.md](benchmarks/SHOWCASE.md) | O resumo e os candidatos ≥40% por tokenizador |
| [ROADMAP.md](ROADMAP.md) | O que vem a seguir, e as questões ainda em aberto |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Compilar, testar, e os toolchains que cada backend exige |

## Desenvolvimento

TDD com um portão rígido: `scripts/coverage.ps1` (Windows) /
`scripts/coverage.sh` fazem a build falhar abaixo de 100% de cobertura de
linhas. A CI roda fmt, clippy `-D warnings`, os testes em Linux e Windows, e o
portão de cobertura. As regras estão em [CONTRIBUTING.md](CONTRIBUTING.md).

## Licença

Licenciado sob a Apache License, Version 2.0 ([LICENSE](LICENSE) ou
<https://www.apache.org/licenses/LICENSE-2.0>).

A menos que você declare explicitamente o contrário, qualquer contribuição
submetida intencionalmente para inclusão na obra, conforme definido pela
licença Apache-2.0, será licenciada como acima, sem termos ou condições
adicionais.
