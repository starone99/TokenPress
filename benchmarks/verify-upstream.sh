#!/usr/bin/env bash
# Behavioural verification against an upstream project's own test suite.
#
# TokenPress verifies every rewrite internally (re-parse + AST/token
# equivalence). This script checks that claim from the outside: it takes a
# pinned corpus, makes two pristine copies, formats one of them at DEFAULT
# settings, and runs the project's real test suite against both. The run only
# succeeds if every test reaches the same outcome on both copies.
#
# Usage: verify-upstream.sh <requests|ripgrep|express|all>
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
  all        every target (requests, then ripgrep, then express)
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
    requests | ripgrep | express | all) ;;
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
    all)
        verify_requests || diverged=1
        verify_ripgrep || diverged=1
        verify_express || diverged=1
        ;;
    esac
    return "$diverged"
}

main "$@"
