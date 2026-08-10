<p align="center">
  <strong>TokenPress</strong>
</p>

<p align="center">
  <strong>Un formateador para agentic coding: optimizado para el tokenizador, no para el lector humano.</strong>
</p>

<p align="center">
  <a href="https://github.com/starone99/TokenPress/actions"><img src="https://github.com/starone99/TokenPress/workflows/CI/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License"></a>
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="README_ko.md">한국어</a> ·
  <a href="README_ja.md">日本語</a> ·
  <a href="README_zh.md">中文</a> ·
  <b>Español</b> ·
  <a href="README_fr.md">Français</a> ·
  <a href="README_pt.md">Português</a>
</p>

> Esta traducción puede quedarse atrás. El documento de referencia es
> [README.md](README.md); si una cifra o una afirmación no coincide, la versión
> en inglés es la correcta.

---

Si haces agentic coding, ¿por qué sigues ejecutando un formateador construido
para un lector humano? Black, gofmt, rustfmt y Prettier optimizan todos para
los ojos de una persona: ancho de línea, alineación, líneas en blanco entre
cosas. Cuando el lector es un modelo, nada de eso es valor. Son tokens que se
facturan.

TokenPress emite el programa equivalente que cuesta menos tokens de entrada:

```text
minimize  tokenizer.encode(transformed_code)
s.t.      el código transformado se parsea, compila y se comporta igual
```

No es un minificador: el número de caracteres y el de tokens no coinciden, así
que las transformaciones se eligen contra un tokenizador real. **La salida que
no supera la verificación nunca se escribe**, y los identificadores y el
contenido de las cadenas no se tocan jamás.

## Cuánto ahorra

Cada fila es una **base de código open source real**, formateada entera en un
commit fijado, con todos los archivos superando la verificación. La barra
sólida es lo que ahorra *cualquier* tokenizador; la cola sombreada es cuánto
más llega el más favorable.

**Ajustes agresivos**: las banderas opcionales que además eliminan comentarios
y docstrings:

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

**Ajustes por defecto**: las mismas bases de código, sin ninguna bandera. Se
conservan comentarios, docstrings y anotaciones de tipo; solo desaparecen
espacios, líneas en blanco e indentación:

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

Fíjate en que el orden cambia. tokio encabeza el gráfico agresivo porque está
densamente comentado con doc comments —quítalos y desaparece medio
repositorio—, pero con los ajustes por defecto queda a media tabla, porque lo
único que queda por quitar son espacios. **Las cifras por defecto son las que
no cuestan nada**; las agresivas son un intercambio que eliges tú.

La amplitud dentro de cada fila es el otro punto: el ahorro depende del
tokenizador, y por eso el benchmark mide seis —GLM-5.2, Kimi K3, Gemma 4,
Qwen3.6, `o200k_base` y `cl100k_base`. Los otros cinco lenguajes soportados,
una base de código cada uno, agresivo, misma ejecución:

```text
target codebase       0%   10%  20%  30%  40%  50%  60%
──────────────────────├────┼────┼────┼────┼────┼────┤
commons-lang (Java)   █████████████████████░░           -42.9 … -46.6%
express (JS)          █████████████░░░░                 -25.4 … -33.3%
rack (Ruby)           █████████░                        -18.2 … -20.8%
gin (Go)              █████████░                        -18.7 … -20.0%
```

**No se ha medido ningún tokenizador privado o cerrado, y ninguna cifra de aquí
se extrapola a uno.** El ahorro sigue la proporción de prosa que tiene un
árbol, no el lenguaje en que está escrito; un corpus por lenguaje es un dato
puntual, nunca una expectativa para todo el lenguaje. Trece corpus, recuentos
de tokens en bruto, tablas por tokenizador y las advertencias sobre finales de
línea están en [benchmarks/RESULTS.md](benchmarks/RESULTS.md), resumidos en
[benchmarks/SHOWCASE.md](benchmarks/SHOWCASE.md).

**Eliminar comentarios y docstrings borra contexto que el modelo podría haber
usado, y si eso degrada sus respuestas no se ha medido.** El ahorro está
medido; el intercambio de calidad no. Trata las banderas agresivas como una
elección, no como algo gratis.

## ¿Lee alguien humano este código?

Una sola pregunta decide cómo usar esto.

**Sí: formatea la copia que le das al modelo y deja tu fuente en paz.** Pégala
en un prompt, pásasela a la ventana de contexto de un agente, dásela a un
índice RAG. Esto vale incluso con los ajustes por defecto: la ejecución por
defecto también quita líneas en blanco y comprime la indentación.
«Context-lossless» aquí es una afirmación sobre lo que un *modelo* puede
recuperar, nunca sobre lo que una persona disfruta leyendo.

**No: nadie lo lee, el repositorio lo escriben y mantienen agentes. Entonces
normalizar la propia fuente es coherente**, y para eso existen el hook de
pre-commit y la GitHub Action. Dos cosas que conviene saber antes, ninguna
sobre lectores humanos:

- **Rust une todas las líneas.** Con los ajustes por defecto, el backend de
  Rust reemite un archivo entero como una sola línea, así que las herramientas
  de edición que direccionan por línea, `git diff`, los conflictos de merge y
  los stack traces se degradan. Los demás backends conservan los saltos de
  línea.
- **Los comentarios se pierden ya por defecto, y cuánto depende del lenguaje.**
  Rust descarta todos los `//` y `/* */`: reemite desde el flujo de tokens de
  `syn`, así que solo sobreviven los comentarios de documentación `///` y
  `//!`. JS/TS conserva el comentario que ocupa su propia línea y descarta el
  que comparte línea con código. Los otros cinco conservan ambos. No hay
  des-formateo, y bajo un hook eso no es una conversión única: cada comentario
  que alguien escriba después se elimina en la siguiente ejecución.

No hay mapeo inverso: ni source map ni parche de vuelta. Un modelo puede leer
código formateado y responder sobre él, pero un diff contra esa copia no se
aplicará al original sin formatear. **Un archivo que un modelo vaya a editar
debe entregársele sin formatear.**

**TokenPress ejecuta TokenPress sobre sí mismo**, mediante el hook
`tokenpress-format` de su propio
[`.pre-commit-config.yaml`](.pre-commit-config.yaml), con los ajustes por
defecto: **-22.6%**, 253.666 → 196.415 tokens. Los costes son los que describe
esta sección y se pagaron a sabiendas: 1.941 líneas de comentarios simples
borradas, `git blame` y las trazas de pila degradadas, el razonamiento movido a
los mensajes de commit y a `docs/`. Las pruebas y la barrera de cobertura del
100% pasaron sin cambios. El antes y el después completo está en
[SHOWCASE.md](benchmarks/SHOWCASE.md#the-fourteenth-codebase-tokenpress-itself-which-does-use-it).

**Por eso tampoco hay plugin de editor ni format-on-save.** Así es como la
mayoría conoce Black, Prettier o rustfmt, y es la única integración que
TokenPress no debería tener: el archivo abierto en tu editor es, por
definición, uno que está leyendo una persona. Una extensión que ejecutara esto
al guardar estaría equivocada justo en el caso que plantea la pregunta de
arriba.

## Úsalo en tu proyecto

Como cualquier otro formateador, la versión pertenece al proyecto y no a tu
máquina; si no, dos personas con versiones distintas se reformatean los
archivos mutuamente para siempre. Fíjala en un hook o en una Action y nadie
tendrá que instalar nada.

**pre-commit**: el framework obtiene y compila él mismo la revisión fijada:

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/starone99/TokenPress
    rev: v0.1.0                  # el pin es el punto: súbelo deliberadamente
    hooks:
      - id: tokenpress-check     # no escribe nada; falla si algo cambiaría
    # - id: tokenpress-format    # reescribe en el sitio. Lee antes la pregunta de arriba.
```

**GitHub Action**: un paso en un workflow que ya tengas:

```yaml
- uses: starone99/TokenPress@v0.1.0
  with:
    paths: src tests
    mode: check                  # `format` reescribe el workspace
```

**`tokenpress.toml`**: banderas por lenguaje, tomadas del directorio padre más
cercano, de modo que el hook, la Action y tus propias ejecuciones coincidan:

```toml
[python]
strip_comments = true
[rust]
strip_doc_comments = true
```

`check` es el valor por defecto en ambas integraciones y no escribe nada. Usa
`format` solo en el lado «nadie lee este código» de la pregunta anterior. Las
opciones, la correspondencia completa entre banderas y configuración y las
cargo features están en [INTEGRATIONS.md](docs/INTEGRATIONS.md).

**Fija una etiqueta de release, no una rama.** En una etiqueta ambas
integraciones descargan el binario de esa release y lo comprueban contra el
`SHA256SUMS` de la release: unos segundos, y ningún toolchain de Rust,
compilador de C ni libclang. Una rama o un commit suelto no tienen binario de
release que les corresponda, así que el CLI se compila desde el checkout:
correcto, y minutos en lugar de segundos. Pedir un binario más pequeño del que
trae una release —`TOKENPRESS_NO_RUBY` y compañía en el hook, las entradas
`ruby`/`go`/`java`/`csharp` de la Action— compila por la misma razón, igual que
cualquier plataforma sin archivo publicado (Windows, macOS Intel y todo Linux
que no sea x86_64). `TOKENPRESS_NO_PREBUILT=1` fuerza la compilación.

## O ejecútalo tú

Para un uso puntual —medir un árbol, o generar la copia que vas a darle a un
modelo— instala la CLI.

```bash
# script de instalación: descarga la release para tu host y la verifica
# contra el SHA256SUMS de la release antes de extraer nada
curl -fsSL https://raw.githubusercontent.com/starone99/TokenPress/master/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/starone99/TokenPress/master/install.ps1 | iex
```

```bash
# o con un toolchain de Rust
cargo install --git https://github.com/starone99/TokenPress tokenpress-cli
```

Los archivos precompilados y `SHA256SUMS` están en
[la página de releases](https://github.com/starone99/TokenPress/releases) para
Linux x86_64, macOS (Apple Silicon) y Windows x86_64; cualquier otra, incluido macOS Intel,
plataforma compila desde el código. `TOKENPRESS_VERSION` fija un tag y
`TOKENPRESS_BIN_DIR` cambia dónde instala el script. Compilar los backends de
Ruby, Go, Java y C# necesita un compilador de C, y libclang para Ruby;
`--no-default-features` no necesita ninguno de los dos, y `--features go,java`
solo devuelve lo que nombres.

Después:

```bash
tokenpress stats  <PATH>...        # cuánto ahorraría — no escribe nada
tokenpress diff   <PATH>...        # diff unificado — no escribe nada
tokenpress format <PATH>...        # reescribe en el sitio (los directorios se recorren)
tokenpress check  <PATH>...        # exit 1 si algo cambiaría
```

Empieza por `stats`. No toca nada y te dice si esto merece la pena en tu árbol:

```bash
tokenpress stats . --tokenizer o200k_base            # GPT-4o / serie o (por defecto)
tokenpress stats . --tokenizer cl100k_base           # GPT-4 / GPT-3.5
tokenpress stats . --tokenizer hf:tokenizer.json     # cualquier tokenizador HF (Qwen, GLM, Gemma…)
tokenpress stats . --tokenizer kimi:tiktoken.model   # formato Kimi ranks
```

Todo lo que pierde información es una bandera opcional, y cada una dice qué
rompe:

```bash
--py-strip-comments        # elimina los comentarios #
--py-strip-docstrings      # vacía __doc__ — rompe help() y los doctests
--py-strip-annotations     # rompe la introspección de dataclass/pydantic
--py-no-merge-imports      # no fusiona imports adyacentes
--rs-strip-doc-comments    # elimina /// y //! — con ellos rustdoc y los doctests
--js-strip-comments        # elimina los comentarios JS/TS que aún sobreviven
--ruby-strip-comments      # conserva el shebang y los magic comments
--go-strip-comments        # conserva directivas //go:, build constraints y el preámbulo cgo
--java-strip-comments      # incluye Javadoc
--csharp-strip-comments    # incluye la documentación XML ///
```

Códigos de salida: `0` correcto · `1` check encontró cambios · `2` error. Los
fallos de parseo y verificación se informan por archivo, y nunca se escribe
nada corrupto.

## Cómo funciona

```text
  fuente ──▶ parseo ──▶ reemisión al mínimo coste en tokens ──▶ verificar ──▶ escribir
                                                                   │
                                                     ┌─────────────┴─────────────┐
                                                     │ reparseo                  │
                                                     │ equivalencia AST / tokens │
                                                     │ el toolchain del lenguaje │  ← --verify external
                                                     └─────────────┬─────────────┘
                                                                   │
                                                          falla ───┴──▶ el archivo queda intacto
```

El último paso es todo el diseño. Una transformación que no se puede demostrar
equivalente no se escribe, así que el peor caso es que un archivo se quede como
estaba, nunca que se corrompa.

## Lenguajes soportados

**Python y Rust son los objetivos principales**: es para lo que se construyó el
proyecto, lo que los benchmarks cubren con más profundidad y a donde va el
trabajo primero. Los otros cinco están soportados sobre el mismo invariante y
las mismas comprobaciones, pero cada uno se apoya en un solo corpus.

| Lenguaje | Extensiones | Conserva comentarios por defecto | Comprobación externa |
|---|---|---|---|
| **Python** | `.py` | ✅ | ❌ solo comprobación interna |
| **Rust** | `.rs` | ❌ `//` y `/* */` siempre se pierden | ❌ solo comprobación interna |
| JavaScript / TypeScript | `.js` `.mjs` `.cjs` `.jsx` `.ts` `.mts` `.cts` `.tsx` | ⚠️ parcial | ✅ `tsc --noEmit` |
| Ruby | `.rb` `.rake` `.gemspec` `.ru`, `Gemfile`, `Rakefile` | ✅ | ✅ `ruby -c` |
| Go | `.go` | ✅ | ✅ `gofmt -e` |
| Java | `.java` | ✅ | ✅ `javac`, detenido tras el parseo |
| C# | `.cs` | ✅ | ✅ Roslyn `csc` |

Esa última columna contradice el párrafo de encima, y se dice aquí en vez de
enterrarla: **los dos lenguajes principales son los dos que no tienen
verificación externa.** Python y Rust solo tienen la comprobación interna. Es
el punto más débil del proyecto, y cerrarlo es lo primero en la
[hoja de ruta](ROADMAP.md).

El detalle por lenguaje —qué conserva cada backend, qué no puede hacer y cómo
se invoca cada comprobador externo— está en [LANGUAGES.md](docs/LANGUAGES.md).

## Documentación

| | |
|---|---|
| [LANGUAGES.md](docs/LANGUAGES.md) | Soporte por lenguaje, advertencias y comprobadores externos |
| [INTEGRATIONS.md](docs/INTEGRATIONS.md) | pre-commit, GitHub Action, archivo de configuración, cargo features |
| [CHANGELOG.md](CHANGELOG.md) | Qué cambió, con las entradas que afectan a la salida marcadas |
| [benchmarks/RESULTS.md](benchmarks/RESULTS.md) | Metodología completa, trece corpus, seis tokenizadores |
| [benchmarks/SHOWCASE.md](benchmarks/SHOWCASE.md) | El resumen y los candidatos ≥40% por tokenizador |
| [ROADMAP.md](ROADMAP.md) | Qué viene, y las preguntas que siguen abiertas |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Compilar, probar, y los toolchains que necesita cada backend |
| [SECURITY.md](SECURITY.md) | Reportar una vulnerabilidad, el modelo de amenazas, integridad de las releases |

## Desarrollo

TDD con una puerta dura: `scripts/coverage.ps1` (Windows) /
`scripts/coverage.sh` hacen fallar la compilación por debajo del 100% de
cobertura de líneas. CI ejecuta clippy `-D warnings`, los tests en Linux y
Windows, y esa puerta: por eso que la insignia de CI de arriba esté en verde
*es* la afirmación de cobertura, en lugar de una insignia que afirma un número
que nadie comprueba.

**No ejecutes `cargo fmt` aquí.** Este repositorio formatea sus propias fuentes
con TokenPress, así que rustfmt no está en CI y solo produciría un diff que el
hook deshace. Las reglas están en [CONTRIBUTING.md](CONTRIBUTING.md), que
también dice dónde poner el razonamiento dado que los comentarios `//` no
sobreviven.

## Licencia

Licenciado bajo la Apache License, Version 2.0 ([LICENSE](LICENSE) o
<https://www.apache.org/licenses/LICENSE-2.0>).

Salvo que indiques expresamente lo contrario, cualquier contribución que envíes
intencionadamente para su inclusión en la obra, tal como se define en la
licencia Apache-2.0, se licenciará como arriba, sin términos ni condiciones
adicionales.
