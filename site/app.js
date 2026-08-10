// Demo page wiring. Everything runs in the browser: the formatters and both
// tokenizer vocabularies live in the WebAssembly bundle under pkg/, so no
// source ever leaves the page and no network request is made after load.
import init,{formatPython,formatRust,formatJs,formatGo,formatJava,formatCSharp}from"./pkg/tokenpress_wasm.js";
// Indirection so the rendering paths can be exercised without the wasm bundle
// (see window.tokenpressDemo at the bottom of this file).
const backend={formatPython,formatRust,formatJs,formatGo,formatJava,formatCSharp};
// The four JavaScript/TypeScript entries are one backend with one dialect
// each: formatJs has no file to read an extension from, so the dialect is
// passed in the options object.
const JS_DIALECTS={javascript:`js`,jsx:`jsx`,typescript:`ts`,tsx:`tsx`};const SAMPLES={python:`import os
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
`,rust:`use std::collections::HashMap;

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
`,javascript:`// A running total is clearer than reduce() here.
export function total(values, start = 0) {
  let result = start; // Trailing comments like this one are always dropped.
  for (const value of values) {
    result = result + value;
  }
  return result;
}

console.log(total([1, 2, 3], 10));
`,jsx:`// Only leading comments like this one survive.
export function Greeting({ name, items }) {
  return (
    <section className="greeting">
      <h1>Hello, {name}!</h1>
      <ul>
        {items.map((item) => (
          <li key={item.id}>{item.label}</li>
        ))}
      </ul>
    </section>
  );
}
`,typescript:`interface Point {
  x: number;
  y: number;
}

/** Distance between two points. */
export function distance(a: Point, b: Point): number {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return Math.sqrt(dx * dx + dy * dy);
}
`,tsx:`interface Props {
  name: string;
  count?: number;
}

export const Badge = ({ name, count = 0 }: Props): JSX.Element => (
  <span className="badge" title={name}>
    {name}: {count}
  </span>
);
`,go:`package main

import (
	"fmt"
	"strings"
)

// Counts how often each word appears.
func wordCounts(text string) map[string]int {
	counts := map[string]int{}
	for _, word := range strings.Fields(text) {
		counts[word]++ // Trailing comments survive too, at the defaults.
	}
	return counts
}

func main() {
	fmt.Println(wordCounts("a b a")["a"])
}
`,java:`import java.util.HashMap;
import java.util.Map;

/**
 * Word frequencies for a line of text.
 *
 * <p>Javadoc is an ordinary block comment to the grammar, so the strip flag
 * deletes this too — at the default settings not one byte of it is lost.
 */
public final class WordCounts {

    // Regular comments survive at the defaults as well.
    public static Map<String, Integer> of(String text) {
        Map<String, Integer> counts = new HashMap<>();
        for (String word : text.split("\\\\s+")) {
            counts.merge(word, 1, Integer::sum); /* inline comments too */
        }
        return counts;
    }

    public static void main(String[] args) {
        System.out.println(of("a b a").get("a"));
    }
}
`,csharp:`using System;
using System.Collections.Generic;

/// <summary>
/// Word frequencies for a line of text.
/// </summary>
/// <remarks>
/// C# gives XML documentation, line and block comments one node kind, so the
/// strip flag deletes this documentation too — at the default settings not one
/// byte of it is lost.
/// </remarks>
public static class WordCounts
{
    // Regular comments survive at the defaults as well.
    public static Dictionary<string, int> Of(string text)
    {
        var counts = new Dictionary<string, int>();
        foreach (var word in text.Split(' '))
        {
            counts[word] = counts.GetValueOrDefault(word) + 1; /* inline comments too */
        }

        return counts;
    }

    public static void Main()
    {
        Console.WriteLine(Of("a b a")["a"]);
    }
}
`};
// Report order. The boundary emits the tokenizer stats as a JSON object,
// whose key order is not the order the library lists them in, so the page
// pins the documented order itself. Anything unrecognised is appended.
const TOKENIZER_ORDER=[`o200k_base`,`cl100k_base`];const el=id=>document.getElementById(id);const dom={source:el(`source`),format:el(`format`),loadSample:el(`load-sample`),clear:el(`clear`),copy:el(`copy`),status:el(`status`),pythonOptions:el(`python-options`),rustOptions:el(`rust-options`),jsOptions:el(`js-options`),goOptions:el(`go-options`),javaOptions:el(`java-options`),csharpOptions:el(`csharp-options`),result:el(`result`),placeholder:el(`placeholder`),changed:el(`changed`),output:el(`output`),statsBody:el(`stats-body`),error:el(`error`),errorKind:el(`error-kind`),errorMessage:el(`error-message`)};function currentLanguage(){return document.querySelector(`input[name="language"]:checked`).value}function currentOptions(language){if(language===`rust`){return{strip_doc_comments:el(`rs-strip-doc-comments`).checked}}if(language===`go`){return{strip_comments:el(`go-strip-comments`).checked}}if(language===`java`){return{strip_comments:el(`java-strip-comments`).checked}}if(language===`csharp`){return{strip_comments:el(`csharp-strip-comments`).checked}}if(language in JS_DIALECTS){return{dialect:JS_DIALECTS[language],strip_comments:el(`js-strip-comments`).checked}}return{strip_comments:el(`py-strip-comments`).checked,strip_docstrings:el(`py-strip-docstrings`).checked,strip_annotations:el(`py-strip-annotations`).checked,merge_imports:el(`py-merge-imports`).checked}}function backendFor(language){if(language===`rust`){return backend.formatRust}if(language===`go`){return backend.formatGo}if(language===`java`){return backend.formatJava}if(language===`csharp`){return backend.formatCSharp}return language in JS_DIALECTS?backend.formatJs:backend.formatPython}function syncLanguage(){const language=currentLanguage();dom.pythonOptions.hidden=language!==`python`;dom.rustOptions.hidden=language!==`rust`;dom.jsOptions.hidden=!(language in JS_DIALECTS);dom.goOptions.hidden=language!==`go`;dom.javaOptions.hidden=language!==`java`;dom.csharpOptions.hidden=language!==`csharp`}function percent(ratio){return`${(ratio*100).toFixed(1)}%`}
// A refusal shows the structured kind and message and nothing else: no
// partial, unverified output ever reaches the page.
function showError(error){dom.result.hidden=true;dom.placeholder.hidden=true;dom.output.textContent=``;dom.statsBody.replaceChildren();dom.errorKind.textContent=error.kind;dom.errorMessage.textContent=error.message;dom.error.hidden=false}function showResult(result){dom.error.hidden=true;dom.placeholder.hidden=true;dom.output.textContent=result.code;dom.changed.textContent=result.changed?`Output differs from the input.`:`Input was already minimal — output is identical.`;dom.changed.classList.toggle(`unchanged`,!result.changed);const rank=name=>{const index=TOKENIZER_ORDER.indexOf(name);return index===-1?TOKENIZER_ORDER.length:index};const entries=Object.entries(result.tokens).sort(([a],[b])=>rank(a)-rank(b)||a.localeCompare(b));const rows=entries.map(([tokenizer,stats])=>{const row=document.createElement(`tr`);const cells=[{text:tokenizer},{text:String(stats.original)},{text:String(stats.formatted)},{text:String(stats.saved)},{text:percent(stats.saving_ratio),className:`saving`}];for(const[index,cell]of cells.entries()){const node=document.createElement(index===0?`th`:`td`);if(index===0){node.scope=`row`}node.textContent=cell.text;if(cell.className){node.className=cell.className}row.append(node)}return row});dom.statsBody.replaceChildren(...rows);dom.result.hidden=false}
// The wasm boundary rejects with the JSON text of {"kind", "message"}. Any
// other throw (a panic, say) is still reported as a refusal rather than being
// mistaken for output.
function asError(thrown){try{const parsed=JSON.parse(String(thrown));if(typeof parsed.kind===`string`&&typeof parsed.message===`string`){return parsed}}catch{}return{kind:`internal`,message:String(thrown)}}function format(){const language=currentLanguage();const options=JSON.stringify(currentOptions(language));const run=backendFor(language);try{showResult(JSON.parse(run(dom.source.value,options)))}catch(thrown){showError(asError(thrown))}}for(const radio of document.querySelectorAll(`input[name="language"]`)){radio.addEventListener(`change`,syncLanguage)}dom.format.addEventListener(`click`,format);dom.loadSample.addEventListener(`click`,()=>{dom.source.value=SAMPLES[currentLanguage()]});dom.clear.addEventListener(`click`,()=>{dom.source.value=``;dom.result.hidden=true;dom.error.hidden=true;dom.placeholder.hidden=false});dom.copy.addEventListener(`click`,async()=>{await navigator.clipboard.writeText(dom.output.textContent);dom.status.textContent=`Output copied.`});syncLanguage();
// Exposed so the page can be driven from the console or a browser test, and
// so the refusal rendering can be exercised with a stubbed backend.
window.tokenpressDemo={backend,format,showError,showResult};dom.status.textContent=`Loading the WebAssembly bundle…`;init().then(()=>{dom.format.textContent=`Format`;dom.format.disabled=false;dom.status.textContent=`Ready — formatting runs entirely in your browser.`;document.body.dataset.ready=`true`}).catch(thrown=>{dom.status.textContent=`Failed to load the WebAssembly bundle: ${thrown}`});