// Demo page wiring. Everything runs in the browser: the formatters and both
// tokenizer vocabularies live in the WebAssembly bundle under pkg/, so no
// source ever leaves the page and no network request is made after load.
import init, { formatPython, formatRust } from "./pkg/tokenpress_wasm.js";

// Indirection so the rendering paths can be exercised without the wasm bundle
// (see window.tokenpressDemo at the bottom of this file).
const backend = { formatPython, formatRust };

const SAMPLES = {
  python: `import os
import sys
from typing import Iterable


def total(values: Iterable[int], start: int = 0) -> int:
    """Sum the values, beginning at start."""
    # A running total is clearer than sum() here.
    result = start
    for value in values:
        result = result + value
    return result


if __name__ == "__main__":
    print(total([1, 2, 3], start=10), os.getpid(), sys.argv)
`,
  rust: `use std::collections::HashMap;

/// Counts how often each word appears.
pub fn word_counts(text: &str) -> HashMap<&str, usize> {
    // Regular comments like this one are always dropped.
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word).or_insert(0) += 1;
    }
    counts
}

fn main() {
    let counts = word_counts("a b a");
    println!("{}", counts["a"]);
}
`,
};

// Report order. The boundary emits the tokenizer stats as a JSON object,
// whose key order is not the order the library lists them in, so the page
// pins the documented order itself. Anything unrecognised is appended.
const TOKENIZER_ORDER = ["o200k_base", "cl100k_base"];

const el = (id) => document.getElementById(id);

const dom = {
  source: el("source"),
  format: el("format"),
  loadSample: el("load-sample"),
  clear: el("clear"),
  copy: el("copy"),
  status: el("status"),
  pythonOptions: el("python-options"),
  rustOptions: el("rust-options"),
  result: el("result"),
  placeholder: el("placeholder"),
  changed: el("changed"),
  output: el("output"),
  statsBody: el("stats-body"),
  error: el("error"),
  errorKind: el("error-kind"),
  errorMessage: el("error-message"),
};

function currentLanguage() {
  return document.querySelector('input[name="language"]:checked').value;
}

function currentOptions(language) {
  if (language === "rust") {
    return { strip_doc_comments: el("rs-strip-doc-comments").checked };
  }
  return {
    strip_comments: el("py-strip-comments").checked,
    strip_docstrings: el("py-strip-docstrings").checked,
    strip_annotations: el("py-strip-annotations").checked,
    merge_imports: el("py-merge-imports").checked,
  };
}

function syncLanguage() {
  const rust = currentLanguage() === "rust";
  dom.pythonOptions.hidden = rust;
  dom.rustOptions.hidden = !rust;
}

function percent(ratio) {
  return `${(ratio * 100).toFixed(1)}%`;
}

// A refusal shows the structured kind and message and nothing else: no
// partial, unverified output ever reaches the page.
function showError(error) {
  dom.result.hidden = true;
  dom.placeholder.hidden = true;
  dom.output.textContent = "";
  dom.statsBody.replaceChildren();
  dom.errorKind.textContent = error.kind;
  dom.errorMessage.textContent = error.message;
  dom.error.hidden = false;
}

function showResult(result) {
  dom.error.hidden = true;
  dom.placeholder.hidden = true;
  dom.output.textContent = result.code;
  dom.changed.textContent = result.changed
    ? "Output differs from the input."
    : "Input was already minimal — output is identical.";
  dom.changed.classList.toggle("unchanged", !result.changed);

  const rank = (name) => {
    const index = TOKENIZER_ORDER.indexOf(name);
    return index === -1 ? TOKENIZER_ORDER.length : index;
  };
  const entries = Object.entries(result.tokens).sort(
    ([a], [b]) => rank(a) - rank(b) || a.localeCompare(b),
  );
  const rows = entries.map(([tokenizer, stats]) => {
    const row = document.createElement("tr");
    const cells = [
      { text: tokenizer },
      { text: String(stats.original) },
      { text: String(stats.formatted) },
      { text: String(stats.saved) },
      { text: percent(stats.saving_ratio), className: "saving" },
    ];
    for (const [index, cell] of cells.entries()) {
      const node = document.createElement(index === 0 ? "th" : "td");
      if (index === 0) {
        node.scope = "row";
      }
      node.textContent = cell.text;
      if (cell.className) {
        node.className = cell.className;
      }
      row.append(node);
    }
    return row;
  });
  dom.statsBody.replaceChildren(...rows);
  dom.result.hidden = false;
}

// The wasm boundary rejects with the JSON text of {"kind", "message"}. Any
// other throw (a panic, say) is still reported as a refusal rather than being
// mistaken for output.
function asError(thrown) {
  try {
    const parsed = JSON.parse(String(thrown));
    if (typeof parsed.kind === "string" && typeof parsed.message === "string") {
      return parsed;
    }
  } catch {
    // Not the structured payload; fall through to the generic shape.
  }
  return { kind: "internal", message: String(thrown) };
}

function format() {
  const language = currentLanguage();
  const options = JSON.stringify(currentOptions(language));
  const run = language === "rust" ? backend.formatRust : backend.formatPython;
  try {
    showResult(JSON.parse(run(dom.source.value, options)));
  } catch (thrown) {
    showError(asError(thrown));
  }
}

for (const radio of document.querySelectorAll('input[name="language"]')) {
  radio.addEventListener("change", syncLanguage);
}
dom.format.addEventListener("click", format);
dom.loadSample.addEventListener("click", () => {
  dom.source.value = SAMPLES[currentLanguage()];
});
dom.clear.addEventListener("click", () => {
  dom.source.value = "";
  dom.result.hidden = true;
  dom.error.hidden = true;
  dom.placeholder.hidden = false;
});
dom.copy.addEventListener("click", async () => {
  await navigator.clipboard.writeText(dom.output.textContent);
  dom.status.textContent = "Output copied.";
});
syncLanguage();

// Exposed so the page can be driven from the console or a browser test, and
// so the refusal rendering can be exercised with a stubbed backend.
window.tokenpressDemo = { backend, format, showError, showResult };

dom.status.textContent = "Loading the WebAssembly bundle…";
init()
  .then(() => {
    dom.format.textContent = "Format";
    dom.format.disabled = false;
    dom.status.textContent = "Ready — formatting runs entirely in your browser.";
    document.body.dataset.ready = "true";
  })
  .catch((thrown) => {
    dom.status.textContent = `Failed to load the WebAssembly bundle: ${thrown}`;
  });
