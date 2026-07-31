#!/usr/bin/env bash
# Behavioural verification against an upstream project's own test suite.
#
# TokenPress verifies every rewrite internally (re-parse + AST/token
# equivalence). This script checks that claim from the outside: it takes a
# pinned corpus, makes two pristine copies, formats one of them at DEFAULT
# settings, and runs the project's real test suite against both. The run only
# succeeds if every test reaches the same outcome on both copies.
#
# Usage: verify-upstream.sh <requests|all>
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
  all        every implemented target (currently: requests)
  ripgrep    not implemented yet
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
    cp -a "$corpus/requests" "$baseline"
    cp -a "$corpus/requests" "$formatted"

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
    ripgrep)
        die "target 'ripgrep' is not implemented yet: verifying the Rust corpus \
needs a cargo-test harness, which a later iteration adds"
        ;;
    requests | all) ;;
    *)
        usage
        exit 2
        ;;
    esac

    command -v python3 >/dev/null || die "python3 is required"
    command -v cargo >/dev/null || die "cargo is required"
    work_dir="$(mktemp -d "${TMPDIR:-/tmp}/tokenpress-verify-XXXXXX")"

    verify_requests
}

main "$@"
