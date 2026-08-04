#!/usr/bin/env bash
# Behavioural verification against an upstream project's own test suite.
#
# TokenPress verifies every rewrite internally (re-parse + AST/token
# equivalence). This script checks that claim from the outside: it takes a
# pinned corpus, makes two pristine copies, formats one of them at DEFAULT
# settings, and runs the project's real test suite against both. The run only
# succeeds if every test reaches the same outcome on both copies.
#
# Usage: verify-upstream.sh <requests|ripgrep|express|rack|go|java|all>
#
# Exit codes: 0 = outcomes identical, 1 = outcomes diverged, 2 = usage or
# infrastructure error (the comparison never ran).
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
corpus="$script_dir/corpus"

# Same pin as fetch.sh (psf/requests v2.32.3), asserted by commit SHA so a
# retagged upstream cannot silently change what is being verified.
requests_tag="v2.32.3"
requests_sha="0e322af87745eff34caffe4df68456ebc20d9068"

# Same pin as fetch.sh (BurntSushi/ripgrep 14.1.1), asserted the same way.
ripgrep_tag="14.1.1"
ripgrep_sha="4649aa9700619f94cf9c66876e9549d83420e16c"

# Same pin as fetch.sh (expressjs/express v5.2.1), asserted the same way.
express_tag="v5.2.1"
express_sha="dbac741a49a5a64336b70c06e85c2e2706e36336"

# Same pin as fetch.sh (rack/rack v3.2.6), asserted the same way. v3.2.6 is an
# annotated tag, so the SHA is the commit it peels to, which is what
# `git rev-parse HEAD` reports after the clone below.
rack_tag="v3.2.6"
rack_sha="e1f22fdbe99afd2126b6fbf05bb12399359574b7"

# Same pin as fetch.sh (gin-gonic/gin v1.11.0), asserted the same way. The
# target is spelled `go` rather than `gin`; the corpus directory, the pin
# variables and the work-directory names all keep the project name, so a second
# Go corpus can be added later without renaming anything.
gin_tag="v1.11.0"
gin_sha="6ad6205e9c94a4b8a320219e28c37c29d22a7a2c"

# Same pin as fetch.sh (apache/commons-lang rel/commons-lang-3.17.0), asserted
# the same way. Like rack's, this is an annotated tag, so the SHA is the commit
# it peels to, which is what `git rev-parse HEAD` reports after the clone below.
# The target is spelled `java` rather than `commons-lang`, matching `go`: the
# corpus directory, the pin variables and the work-directory names all keep the
# project name, so a second Java corpus can be added without renaming anything.
commons_lang_tag="rel/commons-lang-3.17.0"
commons_lang_sha="29ccc7665f3bc5d84155a3092ab2209a053324e6"

work_dir=""
cleanup() {
    status=$?
    if [ -n "$work_dir" ] && [ -d "$work_dir" ]; then
        if [ "$status" -eq 0 ]; then
            rm -rf "$work_dir"
        else
            echo "work directory kept for inspection: $work_dir" >&2
        fi
    fi
}
trap cleanup EXIT

usage() {
    cat >&2 <<'EOF'
usage: verify-upstream.sh <target>

targets:
  requests   psf/requests v2.32.3 - format it, run its pytest suite on the
             unformatted and the formatted copy, require identical outcomes
  ripgrep    BurntSushi/ripgrep 14.1.1 - the same comparison against its
             cargo test suite
  express    expressjs/express v5.2.1 - the same comparison against its mocha
             suite (needs node, npm and npm registry access)
  rack       rack/rack v3.2.6 - the same comparison against its minitest suite
             (needs ruby, bundler and rubygems.org access)
  go         gin-gonic/gin v1.11.0 - the same comparison against its `go test`
             suite (needs the go toolchain and Go module proxy access)
  java       apache/commons-lang 3.17.0 - the same comparison against its
             surefire suite (needs maven, a JDK and Maven Central access)
  all        every target (requests, ripgrep, express, rack, go, then java)
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

# --- corpus -----------------------------------------------------------------

# Clones the pinned requests corpus if benchmarks/fetch.sh has not already done
# so. fetch.sh also downloads corpora and tokenizer files this script does not
# need, so only the requests clone is reproduced here - with the identical pin.
ensure_requests_corpus() {
    local dest="$corpus/requests"
    if [ ! -e "$dest" ]; then
        mkdir -p "$corpus"
        git clone --quiet --depth 1 --branch "$requests_tag" \
            https://github.com/psf/requests "$dest"
    fi
    local head
    head="$(git -C "$dest" rev-parse HEAD)"
    if [ "$head" != "$requests_sha" ]; then
        die "corpus $dest is at $head, expected the pinned $requests_sha"
    fi
    echo "corpus: psf/requests $requests_tag ($requests_sha)"
}

# The same for the pinned ripgrep corpus.
ensure_ripgrep_corpus() {
    local dest="$corpus/ripgrep"
    if [ ! -e "$dest" ]; then
        mkdir -p "$corpus"
        git clone --quiet --depth 1 --branch "$ripgrep_tag" \
            https://github.com/BurntSushi/ripgrep "$dest"
    fi
    local head
    head="$(git -C "$dest" rev-parse HEAD)"
    if [ "$head" != "$ripgrep_sha" ]; then
        die "corpus $dest is at $head, expected the pinned $ripgrep_sha"
    fi
    echo "corpus: BurntSushi/ripgrep $ripgrep_tag ($ripgrep_sha)"
}

# The same for the pinned express corpus.
ensure_express_corpus() {
    local dest="$corpus/express"
    if [ ! -e "$dest" ]; then
        mkdir -p "$corpus"
        git clone --quiet --depth 1 --branch "$express_tag" \
            https://github.com/expressjs/express "$dest"
    fi
    local head
    head="$(git -C "$dest" rev-parse HEAD)"
    if [ "$head" != "$express_sha" ]; then
        die "corpus $dest is at $head, expected the pinned $express_sha"
    fi
    echo "corpus: expressjs/express $express_tag ($express_sha)"
}

# The same for the pinned rack corpus.
ensure_rack_corpus() {
    local dest="$corpus/rack"
    if [ ! -e "$dest" ]; then
        mkdir -p "$corpus"
        git clone --quiet --depth 1 --branch "$rack_tag" \
            https://github.com/rack/rack "$dest"
    fi
    local head
    head="$(git -C "$dest" rev-parse HEAD)"
    if [ "$head" != "$rack_sha" ]; then
        die "corpus $dest is at $head, expected the pinned $rack_sha"
    fi
    echo "corpus: rack/rack $rack_tag ($rack_sha)"
}

# The same for the pinned gin corpus.
ensure_gin_corpus() {
    local dest="$corpus/gin"
    if [ ! -e "$dest" ]; then
        mkdir -p "$corpus"
        git clone --quiet --depth 1 --branch "$gin_tag" \
            https://github.com/gin-gonic/gin "$dest"
    fi
    local head
    head="$(git -C "$dest" rev-parse HEAD)"
    if [ "$head" != "$gin_sha" ]; then
        die "corpus $dest is at $head, expected the pinned $gin_sha"
    fi
    echo "corpus: gin-gonic/gin $gin_tag ($gin_sha)"
}

# The same for the pinned commons-lang corpus.
ensure_commons_lang_corpus() {
    local dest="$corpus/commons-lang"
    if [ ! -e "$dest" ]; then
        mkdir -p "$corpus"
        git clone --quiet --depth 1 --branch "$commons_lang_tag" \
            https://github.com/apache/commons-lang "$dest"
    fi
    local head
    head="$(git -C "$dest" rev-parse HEAD)"
    if [ "$head" != "$commons_lang_sha" ]; then
        die "corpus $dest is at $head, expected the pinned $commons_lang_sha"
    fi
    echo "corpus: apache/commons-lang $commons_lang_tag ($commons_lang_sha)"
}

# --- helpers ----------------------------------------------------------------

# Builds the release binary and echoes its path.
build_tokenpress() {
    cargo build --release --quiet --manifest-path "$repo_root/Cargo.toml" >&2
    echo "$repo_root/target/release/tokenpress"
}

# Runs pytest in $1 with its junit report written to $2 and its console output
# to $3. Test failures are expected (they are the thing being compared), so
# exit code 1 is accepted; anything above it means pytest itself could not run
# and the comparison would be meaningless.
#
# Each run gets a private TMPDIR. Without it the second run inherits temporary
# files from the first: requests' own extract_zipped_paths() caches its output
# under tempfile.gettempdir() and skips the extraction when that file already
# exists, which makes the second run compare against the first run's sources.
#
# The proxy environment variables of the calling shell are dropped: parts of
# the requests suite assert on proxy environment handling, so an inherited
# HTTP(S)_PROXY/NO_PROXY would fail tests for reasons that have nothing to do
# with formatting. Both runs get the same stripped environment.
run_pytest() {
    local dir="$1" junit="$2" log="$3" venv="$4"
    local tmp="$log.tmpdir"
    mkdir -p "$tmp"
    local rc=0
    (
        cd "$dir"
        env -u http_proxy -u https_proxy -u all_proxy -u no_proxy \
            -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u NO_PROXY \
            TMPDIR="$tmp" \
            "$venv/bin/python" -m pytest tests \
            -q -p no:cacheprovider --junitxml="$junit"
    ) >"$log" 2>&1 || rc=$?
    if [ "$rc" -gt 1 ]; then
        tail -n 30 "$log" >&2
        die "pytest exited $rc in $dir (it never produced a comparable result)"
    fi
}

# Turns a junit XML report into a sorted "outcome<TAB>test id" listing, so the
# two runs can be compared with diff rather than on summary counts alone. The
# tree the run took place in ($3) is rewritten to a placeholder, because
# parametrized test ids can embed it (requests parametrizes over __file__).
outcomes_from_junit() {
    local junit="$1" python="$2" tree="$3"
    "$python" - "$junit" "$tree" <<'PY'
import sys
import xml.etree.ElementTree as ET

junit, tree = sys.argv[1], sys.argv[2]
rows = []
for case in ET.parse(junit).getroot().iter("testcase"):
    outcome = "passed"
    for child in case:
        if child.tag == "failure":
            outcome = "failed"
        elif child.tag == "error":
            outcome = "error"
        elif child.tag == "skipped":
            outcome = "xfailed" if child.get("type") == "pytest.xfail" else "skipped"
        else:
            continue
        break
    name = case.get("name", "").replace(tree, "<tree>")
    rows.append("{}\t{}::{}".format(outcome, case.get("classname"), name))
for row in sorted(rows):
    print(row)
PY
}

# Prints "<outcome> <count>" lines for an outcomes listing.
tally() {
    cut -f1 "$1" | sort | uniq -c | awk '{printf "  %-9s %s\n", $2, $1}'
}

# --- requests ---------------------------------------------------------------

verify_requests() {
    ensure_requests_corpus

    local tokenpress
    tokenpress="$(build_tokenpress)"

    local baseline="$work_dir/requests-baseline"
    local formatted="$work_dir/requests-formatted"
    # Explicitly guarded: main() collects a target's exit status, which turns
    # off errexit for everything the target calls.
    cp -a "$corpus/requests" "$baseline" || die "cannot copy the requests corpus"
    cp -a "$corpus/requests" "$formatted" || die "cannot copy the requests corpus"

    # Default settings only - no aggressive flags. Files that fail TokenPress's
    # own verification are reported and left untouched; they still take part in
    # the test run, unformatted.
    local format_log="$work_dir/format.log"
    local rc=0
    "$tokenpress" format "$formatted" >"$format_log" 2>&1 || rc=$?
    local refused
    refused="$(grep -c '^error: ' "$format_log" || true)"
    if [ "$rc" -ne 0 ] && [ "$refused" -eq 0 ]; then
        tail -n 30 "$format_log" >&2
        die "tokenpress format exited $rc"
    fi
    # The corpus copy is a git checkout, so git reports exactly which files the
    # formatter rewrote.
    local changed
    changed="$(git -C "$formatted" status --porcelain | wc -l)"
    local total
    total="$(find "$formatted" -name '*.py' -not -path '*/.git/*' | wc -l)"

    # One virtualenv, so both runs see byte-identical third-party dependencies.
    # requirements-dev.txt pulls in the suite's dev dependencies and installs
    # the copy itself in editable mode; before the second run the editable
    # install is repointed at the formatted copy without touching anything else.
    local venv="$work_dir/venv"
    python3 -m venv "$venv"
    "$venv/bin/pip" install --quiet --upgrade pip setuptools wheel
    (cd "$baseline" && "$venv/bin/pip" install --quiet -r requirements-dev.txt)
    assert_requests_from "$venv" "$baseline"
    run_pytest "$baseline" "$work_dir/baseline.xml" "$work_dir/baseline.log" "$venv"

    (cd "$formatted" && "$venv/bin/pip" install --quiet --no-deps -e ".[socks]")
    assert_requests_from "$venv" "$formatted"
    run_pytest "$formatted" "$work_dir/formatted.xml" "$work_dir/formatted.log" "$venv"

    outcomes_from_junit "$work_dir/baseline.xml" "$venv/bin/python" "$baseline" \
        >"$work_dir/baseline.outcomes"
    outcomes_from_junit "$work_dir/formatted.xml" "$venv/bin/python" "$formatted" \
        >"$work_dir/formatted.outcomes"

    echo
    echo "requests $requests_tag"
    echo "  .py files          $total"
    echo "  rewritten          $changed"
    echo "  refused by verify  $refused"
    echo "  unchanged          $((total - changed - refused))"
    echo "baseline outcomes:"
    tally "$work_dir/baseline.outcomes"
    echo "formatted outcomes:"
    tally "$work_dir/formatted.outcomes"

    if diff -u "$work_dir/baseline.outcomes" "$work_dir/formatted.outcomes" \
        >"$work_dir/outcomes.diff"; then
        echo "verdict: IDENTICAL - every test reached the same outcome on both copies"
        return 0
    fi
    echo "verdict: DIVERGED - the formatted copy behaves differently:"
    sed 's/^/  /' "$work_dir/outcomes.diff"
    return 1
}

# Fails unless the venv imports requests from the copy that is about to be
# tested - otherwise a run could silently measure the wrong tree.
assert_requests_from() {
    local venv="$1" expected="$2" actual
    actual="$("$venv/bin/python" -c 'import requests; print(requests.__file__)')"
    case "$actual" in
    "$expected"/*) ;;
    *) die "venv imports requests from $actual, expected it under $expected" ;;
    esac
}

# --- ripgrep ----------------------------------------------------------------

# Runs the upstream cargo suite in $1, console output (stdout and stderr
# interleaved, which is what carries the per-binary announcements) to $2. $3 is
# "baseline" or "formatted" and only decides how a build failure is reported.
#
# Isolation, mirroring the pytest runner:
#   * a private CARGO_TARGET_DIR, so neither run can pick up the other's
#     artifacts or fingerprints;
#   * a private TMPDIR, because ripgrep's integration tests build their fixture
#     directories under the temp dir and name them after the test, so a shared
#     TMPDIR would let the two runs collide;
#   * --offline, so both runs resolve against the registry cache warmed once
#     before the first run and no dependency can move in between;
#   * --workspace, matching what ripgrep's own CI runs (without --features
#     pcre2, like its non-pcre2 job);
#   * --no-fail-fast, because a failure in one test binary would otherwise stop
#     the remaining binaries and truncate the comparison.
#
# Both runs happen in sibling directories of the same work directory, so
# rustup resolves the same toolchain for each.
#
# Failing tests are the thing being measured, so a non-zero exit is expected;
# cargo reports both a failing test and a failing build as 101, so a build
# failure is detected from the log instead.
run_cargo_test() {
    local dir="$1" log="$2" role="$3"
    local tmp="$log.tmpdir" target="$log.target"
    mkdir -p "$tmp" "$target" || die "cannot create run directories for $log"
    local rc=0
    (
        cd "$dir"
        env TMPDIR="$tmp" CARGO_TARGET_DIR="$target" \
            cargo test --workspace --offline --no-fail-fast
    ) >"$log" 2>&1 || rc=$?
    if grep -qE '^error(\[|: could not compile)' "$log"; then
        tail -n 30 "$log" >&2
        if [ "$role" = "baseline" ]; then
            die "the unformatted copy does not build, so nothing can be compared"
        fi
        # A formatted copy that no longer compiles is the strongest divergence
        # there is, but it yields no per-test outcomes to diff, so the verdict
        # is reported here rather than by the caller.
        echo "verdict: DIVERGED - the formatted copy no longer compiles"
        exit 1
    fi
    if [ "$rc" -ne 0 ] && [ "$rc" -ne 101 ]; then
        tail -n 30 "$log" >&2
        die "cargo test exited $rc in $dir (it never produced a comparable result)"
    fi
}

# Turns a cargo test console log into a sorted "outcome<TAB>target<TAB>test id"
# listing. Stable libtest has no machine-readable output, so the human-readable
# per-test lines are parsed instead.
#
# cargo runs the test binaries one after another and announces each one before
# it starts, which is the only place the target name appears; that name is
# carried over to the test lines that follow so that same-named tests in
# different crates stay apart. The hash suffix of the binary is dropped - it is
# not stable across trees. The label is the test target's name, not the
# package's, so two packages with a same-named test target share one label;
# that only makes a label ambiguous, it cannot hide a difference, because the
# listings are compared as multisets.
#
# Doc test ids embed the line number of their code block ("lib.rs - f (line
# 42)", sometimes followed by a mode such as " - compile"). TokenPress drops
# `//` comments, which moves every doc comment below them, so those ids shift
# for reasons that have nothing to do with behaviour. The line number is
# therefore stripped and doc tests stay in the comparison: doc comments
# themselves survive formatting, so the doc tests must still run and still
# reach the same outcome. Ids that collide after stripping (several code blocks
# in one doc comment) are kept as duplicate rows, so a change in any one of
# them still shows up as a difference.
outcomes_from_cargo() {
    awk '
        $1 == "Running" {
            bin = $NF
            gsub(/[()]/, "", bin)
            sub(/^.*\//, "", bin)
            sub(/-[0-9a-f]+$/, "", bin)
            target = bin
            next
        }
        $1 == "Doc-tests" {
            target = "doc-tests/" $2
            next
        }
        /^test result:/ { next }
        /^test .* \.\.\. / {
            line = substr($0, 6)
            at = index(line, " ... ")
            name = substr(line, 1, at - 1)
            outcome = substr(line, at + 5)
            if (target ~ /^doc-tests\//) {
                sub(/ \(line [0-9]+\)/, "", name)
            }
            # "ignored" can carry a reason, "ok" nothing else; anything
            # unexpected is kept verbatim so it cannot be silently equated.
            if (outcome ~ /^ignored/) { outcome = "ignored" }
            printf "%s\t%s\t%s\n", outcome, target, name
        }
    ' "$1" | sort
}

verify_ripgrep() {
    ensure_ripgrep_corpus

    local tokenpress
    tokenpress="$(build_tokenpress)"

    local baseline="$work_dir/ripgrep-baseline"
    local formatted="$work_dir/ripgrep-formatted"
    cp -a "$corpus/ripgrep" "$baseline" || die "cannot copy the ripgrep corpus"
    cp -a "$corpus/ripgrep" "$formatted" || die "cannot copy the ripgrep corpus"

    # Default settings only. A .rs file makes TokenPress warn on stderr that
    # `//` comments are dropped - that is by design and not an error; only the
    # "error: " lines on stdout are refusals, and a refused file is left
    # untouched and takes part in the test run unformatted.
    local format_log="$work_dir/ripgrep-format.log"
    local rc=0
    "$tokenpress" format "$formatted" >"$format_log" 2>&1 || rc=$?
    local refused
    refused="$(grep -c '^error: ' "$format_log" || true)"
    if [ "$rc" -ne 0 ] && [ "$refused" -eq 0 ]; then
        tail -n 30 "$format_log" >&2
        die "tokenpress format exited $rc"
    fi
    local changed
    changed="$(git -C "$formatted" status --porcelain | wc -l)"
    local total
    total="$(find "$formatted" -name '*.rs' -not -path '*/.git/*' | wc -l)"

    # Warmed once, from the unformatted copy: formatting never touches
    # Cargo.toml or Cargo.lock, so both copies resolve to the same crates and
    # the two --offline runs below share one immutable registry cache.
    (cd "$baseline" && cargo fetch --quiet) ||
        die "cargo fetch failed - the dependencies of the pinned corpus are unavailable"

    run_cargo_test "$baseline" "$work_dir/ripgrep-baseline.log" baseline
    run_cargo_test "$formatted" "$work_dir/ripgrep-formatted.log" formatted

    outcomes_from_cargo "$work_dir/ripgrep-baseline.log" \
        >"$work_dir/ripgrep-baseline.outcomes"
    outcomes_from_cargo "$work_dir/ripgrep-formatted.log" \
        >"$work_dir/ripgrep-formatted.outcomes"
    if [ ! -s "$work_dir/ripgrep-baseline.outcomes" ]; then
        die "no test results were parsed from the baseline run"
    fi

    echo
    echo "ripgrep $ripgrep_tag"
    echo "  .rs files          $total"
    echo "  rewritten          $changed"
    echo "  refused by verify  $refused"
    echo "  unchanged          $((total - changed - refused))"
    echo "baseline outcomes:"
    tally "$work_dir/ripgrep-baseline.outcomes"
    echo "formatted outcomes:"
    tally "$work_dir/ripgrep-formatted.outcomes"

    if diff -u "$work_dir/ripgrep-baseline.outcomes" \
        "$work_dir/ripgrep-formatted.outcomes" \
        >"$work_dir/ripgrep-outcomes.diff"; then
        echo "verdict: IDENTICAL - every test reached the same outcome on both copies"
        return 0
    fi
    echo "verdict: DIVERGED - the formatted copy behaves differently:"
    sed 's/^/  /' "$work_dir/ripgrep-outcomes.diff"
    return 1
}

# --- express ----------------------------------------------------------------

# Runs the upstream mocha suite in $1, its JSON report to $2 and its console
# output to $3.
#
# The suite is invoked exactly as express's own `npm test` script does
# (`--require test/support/env --check-leaks test/ test/acceptance/`); only the
# reporter differs, because `spec` output cannot be reduced to per-test ids
# reliably. The JSON reporter is asked to write to a file rather than stdout so
# that anything the suite itself prints cannot corrupt the report.
#
# Each run gets a private TMPDIR, mirroring the pytest and cargo runners: parts
# of the suite write fixture files under the temp directory, and a shared one
# would let the two runs see each other's leftovers.
#
# mocha exits with the number of failing tests, so any exit code is expected -
# failures are the thing being compared. What is not acceptable is no report at
# all, which means mocha never ran and there is nothing to compare.
run_mocha() {
    local dir="$1" report="$2" log="$3"
    local tmp="$log.tmpdir"
    mkdir -p "$tmp" || die "cannot create the run directory for $log"
    local rc=0
    (
        cd "$dir"
        env TMPDIR="$tmp" ./node_modules/.bin/mocha \
            --require test/support/env \
            --reporter json --reporter-option "output=$report" \
            --check-leaks test/ test/acceptance/
    ) >"$log" 2>&1 || rc=$?
    if [ ! -s "$report" ]; then
        tail -n 30 "$log" >&2
        die "mocha exited $rc in $dir without writing a report (it never produced a comparable result)"
    fi
}

# Turns a mocha JSON report into a sorted "outcome<TAB>file<TAB>test id"
# listing. The rows are built from the passes/failures/pending arrays rather
# than from `tests`, because only those carry the outcome.
#
# The test file is part of the id because express's suite has same-named tests
# in different files (8 fullTitles are shared by two tests each), and the run
# tree is rewritten to a placeholder because the report records absolute paths.
# Rows that are still identical after that are kept as duplicates and compared
# as a multiset, exactly like the cargo doc-test ids.
#
# node does the parsing: the express target already requires it, so this adds
# no prerequisite that the target did not have.
outcomes_from_mocha() {
    local report="$1" tree="$2"
    node -e '
const fs = require("fs");
const [report, tree] = process.argv.slice(1);
const data = JSON.parse(fs.readFileSync(report, "utf8"));
const rows = [];
for (const [outcome, key] of [["passed", "passes"], ["failed", "failures"], ["pending", "pending"]]) {
    for (const test of data[key] || []) {
        const file = (test.file || "").split(tree).join("<tree>");
        rows.push(outcome + "\t" + file + "\t" + (test.fullTitle || ""));
    }
}
rows.sort();
process.stdout.write(rows.map((row) => row + "\n").join(""));
' "$report" "$tree"
}

verify_express() {
    ensure_express_corpus

    local tokenpress
    tokenpress="$(build_tokenpress)"

    local baseline="$work_dir/express-baseline"
    local formatted="$work_dir/express-formatted"
    cp -a "$corpus/express" "$baseline" || die "cannot copy the express corpus"
    cp -a "$corpus/express" "$formatted" || die "cannot copy the express corpus"

    # Default settings only - no --js-strip-comments. A JS/TS file makes
    # TokenPress warn on stderr that trailing and expression-position comments
    # are dropped unconditionally; that is by design and not an error, and no
    # test can observe it because comments do not run. Only the "error: " lines
    # on stdout are refusals, and a refused file is left untouched and takes
    # part in the test run unformatted.
    local format_log="$work_dir/express-format.log"
    local rc=0
    "$tokenpress" format "$formatted" >"$format_log" 2>&1 || rc=$?
    local refused
    refused="$(grep -c '^error: ' "$format_log" || true)"
    if [ "$rc" -ne 0 ] && [ "$refused" -eq 0 ]; then
        tail -n 30 "$format_log" >&2
        die "tokenpress format exited $rc"
    fi
    # Counted before the install below, so node_modules cannot contribute to
    # either number. All eight supported extensions are counted, not just .js,
    # so the figure stays honest if the pin ever moves to a mixed tree.
    local changed
    changed="$(git -C "$formatted" status --porcelain | wc -l)"
    local total
    total="$(find "$formatted" \
        \( -name '*.js' -o -name '*.mjs' -o -name '*.cjs' -o -name '*.jsx' \
        -o -name '*.ts' -o -name '*.mts' -o -name '*.cts' -o -name '*.tsx' \) \
        -not -path '*/.git/*' -not -path '*/node_modules/*' | wc -l)"

    # express sets package-lock=false in .npmrc and ships no lockfile, so two
    # independent `npm install` runs can resolve different versions inside the
    # declared semver ranges. The install therefore happens once and the
    # resulting tree is copied to the other side, which is the node equivalent
    # of the one shared venv and the one shared cargo fetch above: both runs
    # then execute against byte-identical dependencies.
    (cd "$baseline" && npm install --no-audit --no-fund --loglevel=error) ||
        die "npm install failed in $baseline - the express target needs npm registry access"
    cp -a "$baseline/node_modules" "$formatted/node_modules" ||
        die "cannot copy the installed dependencies to $formatted"

    run_mocha "$baseline" "$work_dir/express-baseline.json" \
        "$work_dir/express-baseline.log"
    run_mocha "$formatted" "$work_dir/express-formatted.json" \
        "$work_dir/express-formatted.log"

    outcomes_from_mocha "$work_dir/express-baseline.json" "$baseline" \
        >"$work_dir/express-baseline.outcomes"
    outcomes_from_mocha "$work_dir/express-formatted.json" "$formatted" \
        >"$work_dir/express-formatted.outcomes"
    if [ ! -s "$work_dir/express-baseline.outcomes" ]; then
        die "no test results were parsed from the baseline run"
    fi

    echo
    echo "express $express_tag"
    echo "  .js files          $total"
    echo "  rewritten          $changed"
    echo "  refused by verify  $refused"
    echo "  unchanged          $((total - changed - refused))"
    echo "baseline outcomes:"
    tally "$work_dir/express-baseline.outcomes"
    echo "formatted outcomes:"
    tally "$work_dir/express-formatted.outcomes"

    if diff -u "$work_dir/express-baseline.outcomes" \
        "$work_dir/express-formatted.outcomes" \
        >"$work_dir/express-outcomes.diff"; then
        echo "verdict: IDENTICAL - every test reached the same outcome on both copies"
        return 0
    fi
    echo "verdict: DIVERGED - the formatted copy behaves differently:"
    sed 's/^/  /' "$work_dir/express-outcomes.diff"
    return 1
}

# --- rack -------------------------------------------------------------------

# Writes the minitest plugin that records per-test outcomes into $1.
#
# minitest ships no machine-readable reporter, and its `--verbose` console
# output cannot be used instead: a suite that runs tests in threads interleaves
# the id and the result marker, which are printed separately, so lines are lost
# (measured on another candidate corpus: 1,095 parsable lines for 1,114 tests).
# A reporter object is told about every result exactly once, whichever thread
# produced it, so that is what is used here.
#
# The file is dropped at <dir>/minitest/tokenpress_plugin.rb and <dir> is put on
# the load path: minitest discovers plugins by globbing "minitest/*_plugin.rb"
# over $LOAD_PATH and requires them itself, after it is loaded. Requiring the
# file directly through RUBYOPT would not work - `-r` runs before bundler has
# set the load path up, so `require "minitest"` would pick the interpreter's
# default-gem copy rather than the bundled one. Nothing else lives under <dir>,
# so `require "minitest"` and `require "minitest/autorun"` still resolve to the
# real gem.
#
# Rows are appended, not truncated: the `test:separate` phase runs one process
# per test file and every one of them appends to the same report.
write_minitest_plugin() {
    local dir="$1/minitest"
    mkdir -p "$dir" || die "cannot create the minitest plugin directory $dir"
    cat >"$dir/tokenpress_plugin.rb" <<'RUBY'
# Records one "outcome<TAB>Class#test" row per test into $TOKENPRESS_REPORT.
module Minitest
  class TokenpressReporter < AbstractReporter
    CODES = { "." => "passed", "F" => "failed",
              "E" => "error", "S" => "skipped" }.freeze

    def initialize path
      super()
      @path = path
      @mutex = Mutex.new
      @rows = []
    end

    # Called once per test, possibly from a worker thread.
    def record result
      outcome = CODES.fetch result.result_code, result.result_code
      @mutex.synchronize { @rows << "#{outcome}\t#{result.klass}##{result.name}" }
    end

    def report
      @mutex.synchronize do
        File.open(@path, "a") { |io| @rows.each { |row| io.puts row } }
      end
    end
  end

  def self.plugin_tokenpress_init options
    path = ENV["TOKENPRESS_REPORT"].to_s
    reporter << TokenpressReporter.new(path) unless path.empty?
  end
end
RUBY
}

# Emits the corpus's Ruby paths under $1, NUL-separated.
#
# Every path class the backend claims: the four extensions plus the two exact
# file names. `vendor/` is excluded along with `.git/`, because the bundle is
# installed into the work directory and a stray vendored copy must never be
# counted or rewritten.
rack_ruby_paths() {
    find "$1" \
        \( -name '*.rb' -o -name '*.rake' -o -name '*.gemspec' -o -name '*.ru' \
        -o -name 'Gemfile' -o -name 'Rakefile' \) \
        -not -path '*/.git/*' -not -path '*/vendor/*' -print0
}

# Runs one rake test task in $1, its per-test report to $3 and its console
# output to $4. $5 is the shared bundle path, $6 the minitest plugin directory.
#
# Isolation, mirroring the other runners:
#   * a private TMPDIR per run, because parts of the suite write fixture files
#     under the temp directory;
#   * BUNDLE_PATH into the work directory, so the gems are vendored there and
#     the user's gem home is never written to;
#   * the proxy environment variables of the calling shell are dropped, exactly
#     as for requests: the suite drives ephemeral localhost servers and an
#     inherited HTTP(S)_PROXY would send those requests to a proxy. Both runs
#     get the same stripped environment.
#
# Failing tests are the thing being measured, so rake's non-zero exit is
# expected; what is not acceptable is an empty report, which means the suite
# never ran and there is nothing to compare.
run_rake() {
    local dir="$1" task="$2" report="$3" log="$4" bundle_path="$5" plugins="$6"
    local tmp="$log.tmpdir"
    mkdir -p "$tmp" || die "cannot create the run directory for $log"
    local rc=0
    (
        cd "$dir"
        env -u http_proxy -u https_proxy -u all_proxy -u no_proxy \
            -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u NO_PROXY \
            TMPDIR="$tmp" BUNDLE_PATH="$bundle_path" \
            RUBYOPT="-I$plugins" TOKENPRESS_REPORT="$report" \
            bundle exec rake "$task"
    ) >"$log" 2>&1 || rc=$?
    if [ ! -s "$report" ]; then
        tail -n 30 "$log" >&2
        die "rake $task exited $rc in $dir without writing a report (it never produced a comparable result)"
    fi
}

# Prefixes every row of the phase report $1 with the phase name $2, giving
# "outcome<TAB>phase<TAB>test id".
#
# The phase is part of the id for the same reason the cargo listing carries the
# test target: `test:separate` re-runs the very same test ids in a separate
# process per file, so without it the two phases would be indistinguishable and
# a change confined to one of them could cancel out against the other.
#
# Only the first tab is treated as the separator, so a test name that contains
# one is carried over whole rather than truncated.
tag_phase() {
    awk -v phase="$2" '{
        at = index($0, "\t")
        print substr($0, 1, at - 1) "\t" phase "\t" substr($0, at + 1)
    }' "$1"
}

# Fails unless the copy that is about to be tested is the one whose rack is
# loaded - otherwise a run could silently measure the wrong tree.
assert_rack_from() {
    local dir="$1" bundle_path="$2" actual
    actual="$(cd "$dir" && env BUNDLE_PATH="$bundle_path" bundle exec ruby -Ilib \
        -e 'require "rack"; print $LOADED_FEATURES.grep(%r{/rack\.rb$}).first')"
    case "$actual" in
    "$dir"/*) ;;
    *) die "the bundle loads rack from $actual, expected it under $dir" ;;
    esac
}

verify_rack() {
    ensure_rack_corpus

    local tokenpress
    tokenpress="$(build_tokenpress)"

    local baseline="$work_dir/rack-baseline"
    local formatted="$work_dir/rack-formatted"
    cp -a "$corpus/rack" "$baseline" || die "cannot copy the rack corpus"
    cp -a "$corpus/rack" "$formatted" || die "cannot copy the rack corpus"

    # Default settings only - no --ruby-strip-comments. Unlike Rust and JS/TS,
    # the Ruby backend drops nothing at this level: comments and embdocs all
    # survive, so the rewrite is whitespace only.
    #
    # The Ruby paths are formatted explicitly rather than by handing the whole
    # tree to the formatter: rack ships two files named *.js under
    # test/cgi/assets/ whose entire content is "### TestFile ###", so they are
    # placeholders for the CGI asset tests and not JavaScript at all. Passing
    # the tree would hand them to the JS backend, which reports them as parse
    # errors - noise from another language in a Ruby measurement.
    local file_list="$work_dir/rack-files.nul"
    rack_ruby_paths "$formatted" >"$file_list" || die "cannot list the rack Ruby files"
    if [ ! -s "$file_list" ]; then
        die "no Ruby files found under $formatted - the corpus is not what it should be"
    fi
    local total
    total="$(tr -dc '\0' <"$file_list" | wc -c | tr -d ' ')"
    local format_log="$work_dir/rack-format.log"
    local rc=0
    xargs -0 "$tokenpress" format <"$file_list" >"$format_log" 2>&1 || rc=$?
    local refused
    refused="$(grep -c '^error: ' "$format_log" || true)"
    if [ "$rc" -ne 0 ] && [ "$refused" -eq 0 ]; then
        tail -n 30 "$format_log" >&2
        die "tokenpress format exited $rc"
    fi
    local changed
    changed="$(git -C "$formatted" status --porcelain | wc -l)"

    # rack ships no lockfile, so two independent `bundle install` runs could
    # resolve different versions inside the declared ranges. The install
    # therefore happens once, in the baseline copy, and its Gemfile.lock is
    # copied to the other side; both copies then resolve through that one lock
    # against one shared vendored gem set (BUNDLE_PATH), which is the bundler
    # equivalent of the one shared venv, the one shared cargo fetch and the one
    # shared node_modules above. It also means the formatted Gemfile has to
    # still satisfy the lock resolved from the unformatted one, which is a
    # small check in its own right.
    local bundle_path="$work_dir/rack-bundle"
    (cd "$baseline" && env BUNDLE_PATH="$bundle_path" \
        bundle install --jobs 4 --quiet) ||
        die "bundle install failed in $baseline - the rack target needs rubygems.org access"
    cp "$baseline/Gemfile.lock" "$formatted/Gemfile.lock" ||
        die "cannot copy the resolved lockfile to $formatted"

    local plugins="$work_dir/rack-minitest"
    write_minitest_plugin "$plugins"

    assert_rack_from "$baseline" "$bundle_path"
    assert_rack_from "$formatted" "$bundle_path"

    # Both of rack's test tasks, each invoked on its own. rack's own `rake test`
    # chains spec -> test:regular -> test:separate, but `spec` only regenerates
    # SPEC.rdoc from the `##` comments in lib/rack/lint.rb and defines no tests,
    # and rake stops at the first failing task, so a single failing test in
    # test:regular would skip test:separate entirely. Running the two test tasks
    # separately reaches more of the suite than `rake test` does, not less.
    local phase
    for phase in regular separate; do
        run_rake "$baseline" "test:$phase" "$work_dir/rack-baseline-$phase.tsv" \
            "$work_dir/rack-baseline-$phase.log" "$bundle_path" "$plugins"
        run_rake "$formatted" "test:$phase" "$work_dir/rack-formatted-$phase.tsv" \
            "$work_dir/rack-formatted-$phase.log" "$bundle_path" "$plugins"
    done

    local side
    for side in baseline formatted; do
        {
            tag_phase "$work_dir/rack-$side-regular.tsv" regular
            tag_phase "$work_dir/rack-$side-separate.tsv" separate
        } | sort >"$work_dir/rack-$side.outcomes"
    done
    if [ ! -s "$work_dir/rack-baseline.outcomes" ]; then
        die "no test results were parsed from the baseline run"
    fi

    echo
    echo "rack $rack_tag"
    echo "  Ruby files         $total"
    echo "  rewritten          $changed"
    echo "  refused by verify  $refused"
    echo "  unchanged          $((total - changed - refused))"
    echo "baseline outcomes:"
    tally "$work_dir/rack-baseline.outcomes"
    echo "formatted outcomes:"
    tally "$work_dir/rack-formatted.outcomes"

    if diff -u "$work_dir/rack-baseline.outcomes" \
        "$work_dir/rack-formatted.outcomes" \
        >"$work_dir/rack-outcomes.diff"; then
        echo "verdict: IDENTICAL - every test reached the same outcome on both copies"
        return 0
    fi
    echo "verdict: DIVERGED - the formatted copy behaves differently:"
    sed 's/^/  /' "$work_dir/rack-outcomes.diff"
    return 1
}

# --- go ---------------------------------------------------------------------

# Writes the `go test -json` event reducer into $1 and echoes its path.
#
# Why -json rather than the console output: `go test` prints "--- PASS: Name"
# lines, but a package that runs tests in parallel interleaves the output of
# several tests between a test's start and its result line, and subtest results
# are indented under their parent. Reconstructing per-test outcomes from that
# text is the fragile exercise the ripgrep target has to do for libtest, which
# has no machine-readable mode. `go test` does have one, and it emits exactly
# one terminal event per test, whatever the concurrency - so it is used.
#
# The reducer is a Go program, run with `go run`. The go target already
# requires the Go toolchain, so this adds no prerequisite it did not have, the
# same reasoning the express target uses for parsing its report with node. It
# only imports the standard library, so `go run` needs no network and no
# module: it is compiled as a command-line-arguments package.
#
# Rows are "outcome<TAB>package<TAB>test id". Events with no Test field are the
# package's own verdict and are kept under the id "<package>" - an import path
# can never be that string, so the two cannot collide. Keeping them matters:
# a package that fails to build, or whose binary panics before any test
# reports, produces a package-level fail and no test rows at all, and that has
# to show up as a difference rather than as silence.
write_go_reducer() {
    local dir="$1"
    mkdir -p "$dir" || die "cannot create the go reducer directory $dir"
    cat >"$dir/reduce.go" <<'GO'
// Reduces a `go test -json` event stream into "outcome<TAB>package<TAB>test"
// rows, one per terminal event. Usage: go run reduce.go <events.json>
package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
)

type event struct {
	Action  string `json:"Action"`
	Package string `json:"Package"`
	Test    string `json:"Test"`
}

func main() {
	if len(os.Args) != 2 {
		fmt.Fprintln(os.Stderr, "usage: reduce.go <events.json>")
		os.Exit(2)
	}
	file, err := os.Open(os.Args[1])
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(2)
	}
	defer file.Close()

	out := bufio.NewWriter(os.Stdout)
	lines := bufio.NewScanner(file)
	// Test output is echoed into the stream verbatim, so a single event can be
	// far longer than the default 64 KiB token limit.
	lines.Buffer(make([]byte, 0, 64*1024), 16*1024*1024)
	for lines.Scan() {
		var ev event
		// `go test -json` can interleave non-JSON lines (a toolchain message
		// on stderr, for instance). They carry no outcome, so they are
		// skipped rather than treated as a parse failure.
		if json.Unmarshal(lines.Bytes(), &ev) != nil {
			continue
		}
		switch ev.Action {
		case "pass", "fail", "skip":
		default:
			continue
		}
		test := ev.Test
		if test == "" {
			test = "<package>"
		}
		fmt.Fprintf(out, "%s\t%s\t%s\n", ev.Action, ev.Package, test)
	}
	if err := lines.Err(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(2)
	}
	// Flushed explicitly, and its error checked: a silently truncated listing
	// would be a wrong answer rather than a missing one.
	if err := out.Flush(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(2)
	}
}
GO
    echo "$dir/reduce.go"
}

# Runs the upstream Go suite in $1, its event stream to $2 and any toolchain
# diagnostics to $3. $4 is "baseline" or "formatted" and only decides how a
# build failure is reported. $5 is the build cache shared by both runs.
#
# Isolation, mirroring the other runners:
#   * a private TMPDIR per run, because parts of the suite write fixture files
#     under the temp directory;
#   * `-count=1`, which disables Go's test result cache, so neither run can be
#     answered from a cached verdict;
#   * a GOCACHE inside the work directory, so the user's build cache is never
#     written to. Unlike cargo's target directory this one is *shared* by both
#     runs on purpose: Go's build cache is content-addressed, so a hit implies
#     byte-identical inputs and cannot carry one run's artifacts into the
#     other, while sharing it means the dependencies are compiled once;
#   * the proxy environment variables of the calling shell are dropped, exactly
#     as for requests and rack: the suite drives ephemeral localhost servers
#     and Go's http client honours HTTP(S)_PROXY, so an inherited proxy would
#     fail tests for reasons that have nothing to do with formatting. The
#     module download above happens before this and keeps the proxy.
#
# Failing tests are the thing being measured, so a non-zero exit is expected.
# `go test` reports a failing test and a package that does not build with the
# same exit code, so a build failure is detected from the stream instead: it
# emits `FAIL <pkg> [build failed]` as an output event and no test rows for
# that package. Matching on the marker is a heuristic - a test that printed the
# same text would trip it - but it errs in the safe direction only: on the
# baseline it aborts the comparison, on the formatted copy it reports a
# divergence. It can never turn a real difference into an IDENTICAL verdict.
run_go_test() {
    local dir="$1" events="$2" log="$3" role="$4" gocache="$5"
    local tmp="$log.tmpdir"
    mkdir -p "$tmp" || die "cannot create the run directory for $log"
    local rc=0
    (
        cd "$dir"
        env -u http_proxy -u https_proxy -u all_proxy -u no_proxy \
            -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u NO_PROXY \
            TMPDIR="$tmp" GOCACHE="$gocache" \
            go test -json -count=1 ./...
    ) >"$events" 2>"$log" || rc=$?
    if grep -q '\[build failed\]\|\[setup failed\]' "$events"; then
        tail -n 30 "$log" >&2
        if [ "$role" = "baseline" ]; then
            die "the unformatted copy does not build, so nothing can be compared"
        fi
        # A formatted copy that no longer builds is the strongest divergence
        # there is, but the packages that failed yield no per-test outcomes to
        # diff, so it is called out here rather than left to the caller.
        echo "verdict: DIVERGED - the formatted copy no longer builds"
        exit 1
    fi
    if [ ! -s "$events" ]; then
        tail -n 30 "$log" >&2
        die "go test exited $rc in $dir without emitting any events (it never produced a comparable result)"
    fi
}

verify_go() {
    ensure_gin_corpus

    local tokenpress
    tokenpress="$(build_tokenpress)"

    local baseline="$work_dir/gin-baseline"
    local formatted="$work_dir/gin-formatted"
    cp -a "$corpus/gin" "$baseline" || die "cannot copy the gin corpus"
    cp -a "$corpus/gin" "$formatted" || die "cannot copy the gin corpus"

    # Default settings only - no --go-strip-comments. Like Ruby and unlike Rust
    # and JS/TS, the Go backend drops nothing at this level: every comment
    # survives and the rewrite is whitespace only. The whole tree is handed to
    # the formatter rather than an explicit file list, because this pin holds
    # no file of any other language TokenPress claims - no `.js`, `.py`, `.rs`
    # or Ruby path anywhere - so nothing can be misrouted to another backend.
    local format_log="$work_dir/gin-format.log"
    local rc=0
    "$tokenpress" format "$formatted" >"$format_log" 2>&1 || rc=$?
    local refused
    refused="$(grep -c '^error: ' "$format_log" || true)"
    if [ "$rc" -ne 0 ] && [ "$refused" -eq 0 ]; then
        tail -n 30 "$format_log" >&2
        die "tokenpress format exited $rc"
    fi
    local changed
    changed="$(git -C "$formatted" status --porcelain | wc -l)"
    # -type f is deliberate: Go's own tree contains a *directory* named
    # `not_a_file.go`, so a bare -name test is not a file count anywhere in
    # this project.
    local total
    total="$(find "$formatted" -type f -name '*.go' -not -path '*/.git/*' | wc -l)"

    # Warmed once, from the unformatted copy, with the calling shell's proxy
    # settings intact - the test runs below drop them. Formatting never touches
    # go.mod or go.sum, so both copies resolve to the same module versions;
    # go.sum additionally pins each one by content hash, so the two runs share
    # provably identical dependencies without any of the copying the express
    # and rack targets need.
    (cd "$baseline" && go mod download) ||
        die "go mod download failed - the go target needs Go module proxy access"

    local gocache="$work_dir/gin-gocache"
    mkdir -p "$gocache" || die "cannot create the shared build cache $gocache"

    run_go_test "$baseline" "$work_dir/gin-baseline.json" \
        "$work_dir/gin-baseline.log" baseline "$gocache"
    run_go_test "$formatted" "$work_dir/gin-formatted.json" \
        "$work_dir/gin-formatted.log" formatted "$gocache"

    local reducer
    reducer="$(write_go_reducer "$work_dir/gin-reducer")"
    # Reduced first, sorted second: a pipeline would hide a failing reducer
    # behind sort's exit status, and an empty listing would then read as "no
    # tests" instead of "the reduction never ran".
    local side
    for side in baseline formatted; do
        (cd "$work_dir" && env GOCACHE="$gocache" go run "$reducer" \
            "$work_dir/gin-$side.json") >"$work_dir/gin-$side.rows" ||
            die "cannot reduce the $side event stream"
        sort "$work_dir/gin-$side.rows" >"$work_dir/gin-$side.outcomes" ||
            die "cannot sort the $side outcomes"
    done
    if [ ! -s "$work_dir/gin-baseline.outcomes" ]; then
        die "no test results were parsed from the baseline run"
    fi

    echo
    echo "gin $gin_tag"
    echo "  .go files          $total"
    echo "  rewritten          $changed"
    echo "  refused by verify  $refused"
    echo "  unchanged          $((total - changed - refused))"
    echo "baseline outcomes:"
    tally "$work_dir/gin-baseline.outcomes"
    echo "formatted outcomes:"
    tally "$work_dir/gin-formatted.outcomes"

    if diff -u "$work_dir/gin-baseline.outcomes" \
        "$work_dir/gin-formatted.outcomes" \
        >"$work_dir/gin-outcomes.diff"; then
        echo "verdict: IDENTICAL - every test reached the same outcome on both copies"
        return 0
    fi
    echo "verdict: DIVERGED - the formatted copy behaves differently:"
    sed 's/^/  /' "$work_dir/gin-outcomes.diff"
    return 1
}

# --- java -------------------------------------------------------------------

# Writes the surefire report reducer into $1 and echoes its path.
#
# Why a real XML parser: surefire's own console output prints only a per-class
# summary ("Tests run: 42, Failures: 0, ..."), which is exactly the aggregate
# this script refuses to compare - two runs can produce the same counts with
# different tests failing. The per-test outcomes are in the XML surefire writes
# to target/surefire-reports/TEST-*.xml, one file per test class, so those are
# what is read. Test ids there routinely contain `(`, `)` and `[n]` from
# parameterised tests, and attribute values may carry XML entity escapes, so
# the reports are parsed rather than pattern-matched out of the markup.
#
# The reducer is a Java program, run through the JDK's single-file source
# launcher (`java Reduce.java`), so nothing is compiled to disk first. The java
# target already requires a JDK, so this adds no prerequisite it did not have -
# the same reasoning the go target uses for its `go run` reducer and the
# express target for parsing its report with node. In particular it does not
# reach for xmllint or python, neither of which this script requires anywhere
# else. It uses only the JDK's bundled XML parser, so it needs no network.
#
# Rows are "outcome<TAB>class<TAB>test id". A test case with a <failure>,
# <error> or <skipped> child takes that outcome; anything else passed.
write_surefire_reducer() {
    local dir="$1"
    mkdir -p "$dir" || die "cannot create the surefire reducer directory $dir"
    cat >"$dir/Reduce.java" <<'JAVA'
// Reduces surefire's per-class XML reports into "outcome<TAB>class<TAB>test"
// rows, one per test case. Usage: java Reduce.java <surefire-reports-dir>
import java.io.File;
import java.io.PrintStream;
import java.util.Arrays;
import javax.xml.parsers.DocumentBuilder;
import javax.xml.parsers.DocumentBuilderFactory;
import org.w3c.dom.Document;
import org.w3c.dom.Element;
import org.w3c.dom.Node;
import org.w3c.dom.NodeList;

public class Reduce {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            System.err.println("usage: Reduce.java <surefire-reports-dir>");
            System.exit(2);
        }
        File dir = new File(args[0]);
        File[] reports = dir.listFiles((d, n) -> n.startsWith("TEST-") && n.endsWith(".xml"));
        if (reports == null) {
            System.err.println("not a directory: " + dir);
            System.exit(2);
        }
        // Sorted here so the listing is grouped per class; the caller sorts the
        // whole listing again before diffing.
        Arrays.sort(reports);
        DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
        // The reports are local machine-generated files; resolving an external
        // DTD would turn reading them into a network operation.
        factory.setFeature("http://apache.org/xml/features/nonvalidating/load-external-dtd", false);
        DocumentBuilder builder = factory.newDocumentBuilder();
        StringBuilder out = new StringBuilder();
        for (File report : reports) {
            Document doc = builder.parse(report);
            NodeList cases = doc.getElementsByTagName("testcase");
            for (int i = 0; i < cases.getLength(); i++) {
                Element testcase = (Element) cases.item(i);
                out.append(outcome(testcase)).append('\t')
                        .append(testcase.getAttribute("classname")).append('\t')
                        .append(testcase.getAttribute("name")).append('\n');
            }
        }
        PrintStream stdout = System.out;
        stdout.print(out);
        stdout.flush();
        // Checked explicitly, like the go reducer's flush: a silently truncated
        // listing would be a wrong answer rather than a missing one.
        if (stdout.checkError()) {
            System.err.println("cannot write the reduced listing");
            System.exit(2);
        }
    }

    private static String outcome(Element testcase) {
        for (Node n = testcase.getFirstChild(); n != null; n = n.getNextSibling()) {
            if (n.getNodeType() != Node.ELEMENT_NODE) {
                continue;
            }
            String name = n.getNodeName();
            if (name.equals("failure")) {
                return "fail";
            }
            if (name.equals("error")) {
                return "error";
            }
            if (name.equals("skipped")) {
                return "skip";
            }
        }
        return "pass";
    }
}
JAVA
    echo "$dir/Reduce.java"
}

# Folds the one outcome axis in commons-lang's suite that is not reproducible
# run to run, reading $1 and writing the folded rows to stdout: within
# FastDateParser_TimeZoneStrategyTest, `pass` and `skip` become the single
# outcome `pass-or-skip`. `fail` and `error` are left alone, in that class and
# everywhere else, so a formatting-caused failure anywhere still diverges.
#
# This is the same kind of narrow normalization as the ripgrep target's
# doc-test line-number stripping, and it is here for a measured reason rather
# than a suspected one. That test class is parameterised over every locale the
# JDK offers and deliberately converts an environment-dependent time-zone parse
# failure into an assumption abort - "Mark as an assumption failure instead of
# a hard fail", in commons-lang's own comment - so those invocations report
# `skip` instead of `pass` depending on JVM state the suite itself perturbs.
# Measured on one machine, one JDK, one pin: six pristine runs of the
# unformatted tree produced 19, 13, 19, 13, 19 and 13 skips, the difference
# being six `testTimeZoneStrategy_DateFormatSymbols(Locale)` invocations on
# Portuguese locales, all of them parsing "Hora padrao de Atyrau". Upstream
# knows: the class's own Javadoc says "Breaks randomly on GitHub for Locale
# pt_PT". Two of those six runs were the control - two copies of the
# *unformatted* tree, built and tested back to back exactly as this function
# does, with nothing formatted at all. The first reported 19 and the second
# 13, so the split occurs with the formatter removed from the experiment
# entirely. It is not tied to position either: a later end-to-end run had both
# sides at 19. The only reliable statement is that the number is unstable.
#
# Without the fold the target reports DIVERGED on that difference, which would
# be a false finding - the split reproduces with the formatter removed from
# the experiment entirely.
# What the fold costs is real and worth stating: a formatting change that moved
# a test in this one class between passing and being assumption-aborted would
# not be seen. That axis is the one upstream declared unstable; every other
# outcome in the corpus's 11,720 is compared exactly.
fold_unstable_assumptions() {
    awk -F'\t' -v OFS='\t' '
        $2 == "org.apache.commons.lang3.time.FastDateParser_TimeZoneStrategyTest" &&
            ($1 == "pass" || $1 == "skip") { $1 = "pass-or-skip" }
        { print }
    ' "$1"
}

# Runs the upstream Maven suite in $1, its console output to $2. $3 is
# "baseline" or "formatted" and only decides how a build failure is reported.
#
# `-Dmaven.test.failure.ignore=true` is what makes the two cases separable: a
# failing test is the thing being measured, so it must not fail the build,
# while a compile error or an unresolvable dependency must. With it, surefire
# records the failures in its reports and lets the build succeed, so a non-zero
# exit means Maven itself could not get as far as a comparable result.
#
# Nothing here is gated on stderr, deliberately. Every JVM start can write to
# stderr on a completely successful run - a container that exports
# JAVA_TOOL_OPTIONS makes the JVM announce it there on each launch - so a
# stderr-based check would report failure on every run. Exit code and the
# report files are the signals.
#
# Isolation, mirroring the other runners: a private TMPDIR per run, and each
# copy builds into its own target/ directory, so nothing is shared between the
# two beyond the Maven local repository, which is a coordinate-addressed
# download cache and cannot carry one run's build output into the other. Unlike
# the requests, rack and go runners this one keeps the calling shell's proxy
# environment: Maven needs it to reach Maven Central, and commons-lang's suite
# is a pure-library suite that drives no localhost servers and asserts nothing
# about proxy handling. Both runs get the same environment either way.
run_mvn_test() {
    local dir="$1" log="$2" role="$3"
    local tmp="$log.tmpdir"
    mkdir -p "$tmp" || die "cannot create the run directory for $log"
    local rc=0
    (
        cd "$dir"
        env TMPDIR="$tmp" mvn -B -Dmaven.test.failure.ignore=true test
    ) >"$log" 2>&1 || rc=$?
    if [ "$rc" -ne 0 ]; then
        tail -n 30 "$log" >&2
        if [ "$role" = "baseline" ]; then
            die "the unformatted copy does not build, so nothing can be compared (if the failure is a dependency that could not be resolved, this target needs Maven Central - see the note above verify_java)"
        fi
        # A formatted copy that no longer compiles is the strongest divergence
        # there is, but it yields no per-test outcomes to diff, so it is called
        # out here rather than left to the caller.
        echo "verdict: DIVERGED - the formatted copy no longer builds"
        exit 1
    fi
    if [ -z "$(find "$dir/target/surefire-reports" -name 'TEST-*.xml' -print -quit 2>/dev/null)" ]; then
        tail -n 30 "$log" >&2
        die "mvn test wrote no surefire reports in $dir (it never produced a comparable result)"
    fi
}

# Note on the network: unlike the go and rack targets, whose dependencies this
# script can warm once and then work from, this target needs Maven Central
# reachable when it runs. Maven resolves not only commons-lang's test
# dependencies but its build plugins from there, into the Maven local
# repository (~/.m2 by default), and there is no offline path that does not
# assume that repository is already populated. A run that dies resolving
# dependencies is a network problem, not a flake.
#
# The baseline runs first and warms that repository, so the formatted run
# resolves from an already-populated cache: formatting never touches pom.xml,
# so both copies resolve the same coordinates, and the second run takes them
# from the artifacts the first one downloaded.
verify_java() {
    ensure_commons_lang_corpus

    local tokenpress
    tokenpress="$(build_tokenpress)"

    local baseline="$work_dir/commons-lang-baseline"
    local formatted="$work_dir/commons-lang-formatted"
    cp -a "$corpus/commons-lang" "$baseline" || die "cannot copy the commons-lang corpus"
    cp -a "$corpus/commons-lang" "$formatted" || die "cannot copy the commons-lang corpus"

    # Default settings only - no --java-strip-comments. Like Ruby and Go and
    # unlike Rust and JS/TS, the Java backend drops nothing at this level:
    # every comment survives, Javadoc included, and the rewrite is whitespace
    # only. The whole tree is handed to the formatter rather than an explicit
    # file list, because this pin holds no file of any other language
    # TokenPress claims - no `.py`, `.rs`, `.js`/`.ts`, Ruby path or `.go`
    # anywhere - so nothing can be misrouted to another backend.
    #
    # Verification stays at the default `--verify ast`. `--verify external`
    # would be one probe plus three ~0.4 s `javac` spawns per file, which over
    # a 500-file tree makes the measurement JVM-startup-bound and says nothing
    # more about behaviour than the suite below already does.
    local format_log="$work_dir/commons-lang-format.log"
    local rc=0
    "$tokenpress" format "$formatted" >"$format_log" 2>&1 || rc=$?
    local refused
    refused="$(grep -c '^error: ' "$format_log" || true)"
    if [ "$rc" -ne 0 ] && [ "$refused" -eq 0 ]; then
        tail -n 30 "$format_log" >&2
        die "tokenpress format exited $rc"
    fi
    local changed
    changed="$(git -C "$formatted" status --porcelain | wc -l)"
    # -type f for the same reason as the go target, and target/ is excluded so
    # the count means the same thing whether or not a build has run in the tree
    # - a fresh clone has no target/, but a corpus someone has already built in
    # would otherwise contribute generated sources to the file count.
    local total
    total="$(find "$formatted" -type f -name '*.java' \
        -not -path '*/.git/*' -not -path '*/target/*' | wc -l)"

    run_mvn_test "$baseline" "$work_dir/commons-lang-baseline.log" baseline
    run_mvn_test "$formatted" "$work_dir/commons-lang-formatted.log" formatted

    local reducer
    reducer="$(write_surefire_reducer "$work_dir/commons-lang-reducer")"
    # Reduced first, sorted second, for the same reason as the go target: a
    # pipeline would hide a failing reducer behind sort's exit status.
    local side dir
    for side in baseline formatted; do
        dir="$work_dir/commons-lang-$side"
        java "$reducer" "$dir/target/surefire-reports" \
            >"$work_dir/commons-lang-$side.rows" \
            2>"$work_dir/commons-lang-$side-reduce.log" ||
            die "cannot reduce the $side surefire reports"
        sort "$work_dir/commons-lang-$side.rows" \
            >"$work_dir/commons-lang-$side.outcomes" ||
            die "cannot sort the $side outcomes"
        # The tally above reports the exact outcomes; the comparison below runs
        # on the folded listing. Kept as two files so the printed counts stay
        # honest about what surefire actually recorded.
        fold_unstable_assumptions "$work_dir/commons-lang-$side.outcomes" \
            >"$work_dir/commons-lang-$side.folded" ||
            die "cannot fold the $side outcomes"
        sort "$work_dir/commons-lang-$side.folded" \
            >"$work_dir/commons-lang-$side.comparable" ||
            die "cannot sort the folded $side outcomes"
    done
    if [ ! -s "$work_dir/commons-lang-baseline.outcomes" ]; then
        die "no test results were parsed from the baseline run"
    fi

    echo
    echo "commons-lang $commons_lang_tag"
    echo "  .java files        $total"
    echo "  rewritten          $changed"
    echo "  refused by verify  $refused"
    echo "  unchanged          $((total - changed - refused))"
    echo "baseline outcomes:"
    tally "$work_dir/commons-lang-baseline.outcomes"
    echo "formatted outcomes:"
    tally "$work_dir/commons-lang-formatted.outcomes"
    echo "comparison: pass and skip folded together inside"
    echo "  FastDateParser_TimeZoneStrategyTest, which upstream documents as"
    echo "  breaking randomly by locale; fail and error compare exactly"

    if diff -u "$work_dir/commons-lang-baseline.comparable" \
        "$work_dir/commons-lang-formatted.comparable" \
        >"$work_dir/commons-lang-outcomes.diff"; then
        echo "verdict: IDENTICAL - every test reached the same outcome on both copies, up to the fold above"
        return 0
    fi
    echo "verdict: DIVERGED - the formatted copy behaves differently:"
    sed 's/^/  /' "$work_dir/commons-lang-outcomes.diff"
    return 1
}

# --- main -------------------------------------------------------------------

main() {
    if [ "$#" -ne 1 ]; then
        usage
        exit 2
    fi
    case "$1" in
    -h | --help)
        usage
        exit 0
        ;;
    requests | ripgrep | express | rack | go | java | all) ;;
    *)
        usage
        exit 2
        ;;
    esac

    command -v cargo >/dev/null || die "cargo is required"
    case "$1" in
    requests | all) command -v python3 >/dev/null || die "python3 is required" ;;
    esac
    # Checked up front rather than at the install: the express target is the
    # only one with a network prerequisite beyond the git clone, and finding
    # that out after a full format and test run would waste the whole run.
    case "$1" in
    express | all)
        command -v node >/dev/null || die "node is required for the express target"
        command -v npm >/dev/null || die "npm is required for the express target"
        ;;
    esac
    case "$1" in
    rack | all)
        command -v ruby >/dev/null || die "ruby is required for the rack target"
        command -v bundle >/dev/null || die "bundler is required for the rack target"
        ;;
    esac
    case "$1" in
    go | all)
        command -v go >/dev/null || die "the go toolchain is required for the go target"
        ;;
    esac
    # `mvn` is what this target actually spawns; a JDK is the prerequisite
    # behind it, both for Maven itself and for the report reducer, so the
    # message names it even though the check is on the command.
    case "$1" in
    java | all)
        command -v mvn >/dev/null || die "maven (and a JDK) is required for the java target"
        ;;
    esac
    work_dir="$(mktemp -d "${TMPDIR:-/tmp}/tokenpress-verify-XXXXXX")"

    # A target returns 1 when the outcomes diverged; that is a result, not an
    # infrastructure failure, so it is collected instead of aborting the run.
    # Everything that makes a comparison impossible goes through die() and
    # leaves immediately.
    local diverged=0
    case "$1" in
    requests) verify_requests || diverged=1 ;;
    ripgrep) verify_ripgrep || diverged=1 ;;
    express) verify_express || diverged=1 ;;
    rack) verify_rack || diverged=1 ;;
    go) verify_go || diverged=1 ;;
    java) verify_java || diverged=1 ;;
    all)
        verify_requests || diverged=1
        verify_ripgrep || diverged=1
        verify_express || diverged=1
        verify_rack || diverged=1
        verify_go || diverged=1
        verify_java || diverged=1
        ;;
    esac
    return "$diverged"
}

main "$@"
