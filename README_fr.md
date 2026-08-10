<p align="center">
  <img src="assets/mascot-header.png" width="360"
       alt="Mascotte de TokenPress : un robot presse à vis ; de longues lignes de code entrent à gauche et en ressortent courtes à droite">
</p>

<p align="center">
  <strong>TokenPress</strong>
</p>

<p align="center">
  <strong>Un formateur pour l'agentic coding : optimisé pour le tokeniseur, pas pour le lecteur humain.</strong>
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
  <a href="README_es.md">Español</a> ·
  <b>Français</b> ·
  <a href="README_pt.md">Português</a>
</p>

> Cette traduction peut être en retard. Le document de référence est
> [README.md](README.md) ; en cas d'écart sur un chiffre ou une affirmation,
> c'est la version anglaise qui fait foi.

---

Si vous faites de l'agentic coding, pourquoi utilisez-vous encore un formateur
conçu pour un lecteur humain ? Black, gofmt, rustfmt et Prettier optimisent
tous pour l'œil d'une personne : largeur de ligne, alignement, lignes vides
entre les éléments. Quand le lecteur est un modèle, rien de tout cela n'est de
la valeur. Ce sont des tokens facturés.

TokenPress émet le programme équivalent qui coûte le moins de tokens d'entrée :

```text
minimize  tokenizer.encode(transformed_code)
s.t.      le code transformé se parse, compile et se comporte à l'identique
```

Ce n'est pas un minifieur : le nombre de caractères et le nombre de tokens ne
coïncident pas, donc les transformations sont choisies face à un vrai
tokeniseur. **Une sortie qui échoue à la vérification n'est jamais écrite**, et
les identifiants comme le contenu des chaînes ne sont jamais touchés.

## Combien cela économise

Chaque ligne est une **vraie base de code open source**, formatée entièrement à
un commit épinglé, tous les fichiers passant la vérification. La barre pleine
est ce que *chaque* tokeniseur économise ; la queue grisée est ce que le plus
favorable gagne en plus.

**Réglages agressifs** — les options facultatives qui suppriment aussi les
commentaires et les docstrings :

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

**Réglages par défaut** — mêmes bases de code, aucune option. Commentaires,
docstrings et annotations de type sont tous conservés ; seuls les espaces, les
lignes vides et l'indentation disparaissent :

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

Notez que l'ordre change. tokio est en tête du graphique agressif parce qu'il
est dense en doc comments — enlevez-les et la moitié du dépôt disparaît — mais
avec les réglages par défaut il est au milieu, car il ne reste que des espaces
à retirer. **Les chiffres par défaut sont ceux qui ne coûtent rien** ; les
chiffres agressifs sont un arbitrage que vous choisissez.

L'écart à l'intérieur de chaque ligne est l'autre point : les gains dépendent du
tokeniseur, et c'est pourquoi le benchmark en mesure six — GLM-5.2, Kimi K3,
Gemma 4, Qwen3.6, `o200k_base` et `cl100k_base`. Les cinq autres langages
supportés, une base de code chacun, agressif, même exécution :

```text
target codebase       0%   10%  20%  30%  40%  50%  60%
──────────────────────├────┼────┼────┼────┼────┼────┤
commons-lang (Java)   █████████████████████░░           -42.9 … -46.6%
express (JS)          █████████████░░░░                 -25.4 … -33.3%
rack (Ruby)           █████████░                        -18.2 … -20.8%
gin (Go)              █████████░                        -18.7 … -20.0%
```

**Aucun tokeniseur privé ou fermé n'a été mesuré, et aucun chiffre ici n'est
extrapolé vers l'un d'eux.** Les gains suivent la part de prose dans un arbre,
pas le langage dans lequel il est écrit ; un corpus par langage est un point de
mesure, jamais une attente valable pour tout le langage. Treize corpus, les
comptes de tokens bruts, les tableaux par tokeniseur et les mises en garde sur
les fins de ligne sont dans [benchmarks/RESULTS.md](benchmarks/RESULTS.md),
résumés dans [benchmarks/SHOWCASE.md](benchmarks/SHOWCASE.md).

**Supprimer commentaires et docstrings efface du contexte que le modèle aurait
pu utiliser, et savoir si cela dégrade ses réponses n'a pas été mesuré.** Les
gains sont mesurés ; l'arbitrage sur la qualité ne l'est pas. Traitez les
options agressives comme un choix, pas comme un gain gratuit.

## Un humain lit-il ce code ?

Une seule question décide de l'usage.

**Oui — formatez la copie que vous donnez au modèle et laissez votre source
tranquille.** Collez-la dans un prompt, passez-la à la fenêtre de contexte d'un
agent, indexez-la pour du RAG. Cela vaut même avec les réglages par défaut : le
passage par défaut supprime lui aussi les lignes vides et resserre
l'indentation. « Context-lossless » ici est une affirmation sur ce qu'un
*modèle* peut récupérer, jamais sur ce qu'une personne a plaisir à lire.

**Non — personne ne le lit, le dépôt est écrit et maintenu par des agents —
alors normaliser la source elle-même est cohérent**, et le hook pre-commit et
la GitHub Action existent pour cela. Deux choses à savoir d'abord, aucune ne
concernant les lecteurs humains :

- **Rust joint toutes les lignes.** Avec les réglages par défaut, le backend
  Rust réémet un fichier entier sur une seule ligne : les outils d'édition qui
  adressent par ligne, `git diff`, les conflits de merge et les stack traces se
  dégradent tous. Les autres backends conservent les retours à la ligne.
- **Les commentaires se perdent dès les réglages par défaut, et l'ampleur
  dépend du langage.** Rust supprime tous les `//` et `/* */` : il réémet
  depuis le flux de tokens de `syn`, donc seuls les commentaires de
  documentation `///` et `//!` survivent. JS/TS garde le commentaire qui occupe
  sa propre ligne et supprime celui qui partage une ligne avec du code. Les
  cinq autres gardent les deux. Il n'y a pas de dé-formatage, et sous un hook
  ce n'est pas une conversion unique : chaque commentaire écrit ensuite est
  supprimé à l'exécution suivante.

Il n'y a pas de correspondance inverse : ni source map, ni ré-application de
patch. Un modèle peut lire du code formaté et répondre à son sujet, mais un
diff produit contre cette copie ne s'appliquera pas à l'original non formaté.
**Un fichier qu'un modèle va modifier doit lui être donné non formaté.**

**TokenPress exécute TokenPress sur lui-même**, via le hook
`tokenpress-format` de son propre
[`.pre-commit-config.yaml`](.pre-commit-config.yaml), aux réglages par défaut :
**-22,6 %**, 253 666 → 196 415 tokens. Les coûts sont ceux que décrit cette
section et ils ont été payés en connaissance de cause — 1 941 lignes de
commentaires simples supprimées, `git blame` et les traces de pile dégradées,
le raisonnement déplacé vers les messages de commit et `docs/`. Les tests et la
barrière de couverture à 100 % sont passés inchangés. L'avant/après complet est
dans
[SHOWCASE.md](benchmarks/SHOWCASE.md#the-fourteenth-codebase-tokenpress-itself-which-does-use-it).

**C'est aussi pourquoi il n'y a ni plugin d'éditeur ni format-on-save.** C'est
ainsi que la plupart des gens rencontrent Black, Prettier ou rustfmt, et c'est
la seule intégration que TokenPress ne devrait pas avoir : le fichier ouvert
dans votre éditeur est, par définition, un fichier qu'un humain lit. Une
extension qui lancerait ceci à l'enregistrement aurait tort précisément dans le
cas que pose la question ci-dessus.

## L'utiliser dans votre projet

Comme tout autre formateur, la version appartient au projet et non à votre
machine — sinon deux personnes sur des versions différentes se reformatent
mutuellement leurs fichiers indéfiniment. Épinglez-le dans un hook ou une
Action et personne n'a rien à installer.

**pre-commit** — le framework récupère et compile lui-même la révision
épinglée :

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/starone99/TokenPress
    rev: v0.1.0                  # l'épinglage est l'essentiel — montez-le délibérément
    hooks:
      - id: tokenpress-check     # n'écrit rien ; échoue si quelque chose changerait
    # - id: tokenpress-format    # réécrit sur place. Lisez d'abord la question ci-dessus.
```

**GitHub Action** — une étape dans un workflow existant :

```yaml
- uses: starone99/TokenPress@v0.1.0
  with:
    paths: src tests
    mode: check                  # `format` réécrit le workspace
```

**`tokenpress.toml`** — des options par langage, lues depuis le répertoire
parent le plus proche, de sorte que le hook, l'Action et vos propres
exécutions soient d'accord :

```toml
[python]
strip_comments = true
[rust]
strip_doc_comments = true
```

`check` est la valeur par défaut des deux intégrations et n'écrit rien.
N'utilisez `format` que du côté « personne ne lit ce code » de la question
ci-dessus. Les options, la correspondance complète options/configuration et les
cargo features sont dans [INTEGRATIONS.md](docs/INTEGRATIONS.md).

**Épinglez un tag de release, pas une branche.** Sur un tag, les deux
intégrations téléchargent le binaire de cette release et le vérifient avec le
`SHA256SUMS` de la release : quelques secondes, et aucun toolchain Rust,
compilateur C ou libclang. Une branche ou un commit nu n'a pas de binaire de
release correspondant, donc le CLI est compilé depuis le checkout : correct, et
des minutes plutôt que des secondes. Demander un binaire plus petit que celui
d'une release — `TOKENPRESS_NO_RUBY` et consorts pour le hook, les entrées
`ruby`/`go`/`java`/`csharp` de l'Action — compile pour la même raison, tout
comme toute plateforme sans archive publiée (Windows, macOS Intel et tout Linux
non-x86_64). `TOKENPRESS_NO_PREBUILT=1` force la compilation depuis les sources.

## Ou l'exécuter vous-même

Pour un usage ponctuel — mesurer un arbre, ou produire la copie que vous allez
donner à un modèle — installez la CLI.

```bash
# script d'installation : télécharge la release correspondant à votre hôte et la
# vérifie contre le SHA256SUMS de la release avant toute extraction
curl -fsSL https://raw.githubusercontent.com/starone99/TokenPress/master/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/starone99/TokenPress/master/install.ps1 | iex
```

```bash
# ou avec un toolchain Rust
cargo install --git https://github.com/starone99/TokenPress tokenpress-cli
```

Les archives précompilées et `SHA256SUMS` sont sur
[la page des releases](https://github.com/starone99/TokenPress/releases) pour
Linux x86_64, macOS (Apple Silicon) et Windows x86_64 ; toute autre, y compris macOS Intel,
plateforme compile depuis les sources. `TOKENPRESS_VERSION` épingle un tag et
`TOKENPRESS_BIN_DIR` change l'emplacement d'installation. Compiler les backends
Ruby, Go, Java et C# demande un compilateur C, et libclang pour Ruby ;
`--no-default-features` n'a besoin d'aucun des deux, et `--features go,java` ne
remet que ce que vous nommez.

Ensuite :

```bash
tokenpress stats  <PATH>...        # ce que cela économiserait — n'écrit rien
tokenpress diff   <PATH>...        # diff unifié — n'écrit rien
tokenpress format <PATH>...        # réécrit sur place (les dossiers sont parcourus)
tokenpress check  <PATH>...        # exit 1 si quelque chose changerait
```

Commencez par `stats`. Il ne touche à rien et vous dit si le jeu en vaut la
chandelle pour votre arbre :

```bash
tokenpress stats . --tokenizer o200k_base            # GPT-4o / série o (par défaut)
tokenpress stats . --tokenizer cl100k_base           # GPT-4 / GPT-3.5
tokenpress stats . --tokenizer hf:tokenizer.json     # n'importe quel tokeniseur HF (Qwen, GLM, Gemma…)
tokenpress stats . --tokenizer kimi:tiktoken.model   # format Kimi ranks
```

Tout ce qui perd de l'information est une option explicite, et chacune dit ce
qu'elle casse :

```bash
--py-strip-comments        # supprime les commentaires #
--py-strip-docstrings      # vide __doc__ — casse help() et les doctests
--py-strip-annotations     # casse l'introspection dataclass/pydantic
--py-no-merge-imports      # ne fusionne pas les imports adjacents
--rs-strip-doc-comments    # supprime /// et //! — et avec eux rustdoc et les doctests
--js-strip-comments        # supprime les commentaires JS/TS qui survivaient encore
--ruby-strip-comments      # conserve le shebang et les magic comments
--go-strip-comments        # conserve les directives //go:, les build constraints et le préambule cgo
--java-strip-comments      # Javadoc inclus
--csharp-strip-comments    # documentation XML /// incluse
```

Codes de sortie : `0` ok · `1` check a trouvé des changements · `2` erreur. Les
échecs de parsing et de vérification sont signalés fichier par fichier, et rien
de corrompu n'est jamais écrit.

## Comment ça marche

```text
  source ──▶ parse ──▶ réémission au coût minimal en tokens ──▶ vérifier ──▶ écrire
                                                                   │
                                                     ┌─────────────┴─────────────┐
                                                     │ re-parse                  │
                                                     │ équivalence AST / tokens  │
                                                     │ le toolchain du langage   │  ← --verify external
                                                     └─────────────┬─────────────┘
                                                                   │
                                                          échec ───┴──▶ fichier laissé intact
```

La dernière étape, c'est tout le design. Une transformation qu'on ne peut pas
prouver équivalente n'est pas écrite : le pire cas est donc un fichier laissé
tel quel, jamais un fichier corrompu.

## Langages supportés

**Python et Rust sont les cibles principales** — ce pour quoi le projet a été
construit, ce que les benchmarks couvrent le plus en profondeur, et là où le
travail va d'abord. Les cinq autres sont supportés sur le même invariant et les
mêmes vérifications, mais chacun repose sur un seul corpus.

| Langage | Extensions | Garde les commentaires par défaut | Vérification externe |
|---|---|---|---|
| **Python** | `.py` | ✅ | ❌ vérification interne seule |
| **Rust** | `.rs` | ❌ `//` et `/* */` toujours perdus | ❌ vérification interne seule |
| JavaScript / TypeScript | `.js` `.mjs` `.cjs` `.jsx` `.ts` `.mts` `.cts` `.tsx` | ⚠️ partiel | ✅ `tsc --noEmit` |
| Ruby | `.rb` `.rake` `.gemspec` `.ru`, `Gemfile`, `Rakefile` | ✅ | ✅ `ruby -c` |
| Go | `.go` | ✅ | ✅ `gofmt -e` |
| Java | `.java` | ✅ | ✅ `javac`, arrêté après le parsing |
| C# | `.cs` | ✅ | ✅ Roslyn `csc` |

Cette dernière colonne contredit le paragraphe qui la précède, et c'est dit ici
plutôt qu'enterré : **les deux langages principaux sont les deux qui n'ont pas
de vérification externe.** Python et Rust n'ont que la vérification interne.
C'est le point le plus faible du projet, et le combler est le premier élément
de la [feuille de route](ROADMAP.md).

Le détail par langage — ce que chaque backend conserve, ce qu'il ne peut pas
faire, et comment chaque vérificateur externe est invoqué — est dans
[LANGUAGES.md](docs/LANGUAGES.md).

## Documentation

| | |
|---|---|
| [LANGUAGES.md](docs/LANGUAGES.md) | Support par langage, mises en garde et vérificateurs externes |
| [INTEGRATIONS.md](docs/INTEGRATIONS.md) | pre-commit, GitHub Action, fichier de configuration, cargo features |
| [CHANGELOG.md](CHANGELOG.md) | Ce qui a changé, avec les entrées affectant la sortie marquées |
| [benchmarks/RESULTS.md](benchmarks/RESULTS.md) | Méthodologie complète, treize corpus, six tokeniseurs |
| [benchmarks/SHOWCASE.md](benchmarks/SHOWCASE.md) | Le résumé, et les candidats ≥40% par tokeniseur |
| [ROADMAP.md](ROADMAP.md) | La suite, et les questions encore ouvertes |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Compiler, tester, et les toolchains dont chaque backend a besoin |
| [SECURITY.md](SECURITY.md) | Signaler une vulnérabilité, le modèle de menace, l'intégrité des releases |

## Développement

TDD avec une barrière stricte : `scripts/coverage.ps1` (Windows) /
`scripts/coverage.sh` font échouer la compilation sous 100 % de couverture de
lignes. La CI lance clippy `-D warnings`, les tests sur Linux et Windows,
et cette barrière — donc le badge CI ci-dessus au vert *est* l'affirmation de
couverture, plutôt qu'un badge affirmant un chiffre que rien ne vérifie.

**Ne lancez pas `cargo fmt` ici.** Ce dépôt formate ses propres sources avec
TokenPress, donc rustfmt n'est pas dans la CI et ne produirait qu'un diff que
le hook annule. Les règles sont dans [CONTRIBUTING.md](CONTRIBUTING.md), qui
indique aussi où mettre le raisonnement puisque les commentaires `//` ne
survivent pas.

## Licence

Sous licence Apache License, Version 2.0 ([LICENSE](LICENSE) ou
<https://www.apache.org/licenses/LICENSE-2.0>).

Sauf mention explicite contraire de votre part, toute contribution que vous
soumettez intentionnellement pour inclusion dans l'œuvre, telle que définie par
la licence Apache-2.0, sera licenciée comme ci-dessus, sans condition
supplémentaire.
