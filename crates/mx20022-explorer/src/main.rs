// Mapping Explorer (M1, read-only).
//
// Generates a self-contained HTML page that links every field of a converted
// `pacs.008` message back to the Rust snippet in `mx20022-translate` that
// produced it, together with the field's ISO 20022 type and XSD constraints
// extracted from the generated model.
//
// This is a developer tool. It does not change the translation logic: it reads
// `// @maps <path>` provenance markers from the mapping source and the doc
// comments emitted by the code generator.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use mx20022_translate::mappings::mt103_to_pacs008::mt103_to_pacs008;
use mx20022_translate::mt::fields::mt103::parse_mt103;
use mx20022_translate::mt::parser::parse;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

/// Mapping source that carries the `@maps` provenance markers.
const MAPPING_SRC: &str = include_str!("../../mx20022-translate/src/mappings/mt103_to_pacs008.rs");
/// Generated model, scanned for per-type XSD constraint docs.
const MODEL_SRC: &str = include_str!("../../mx20022-model/src/generated/pacs/pacs_008_001_13.rs");
/// Sample MT103 used when no input file is given on the command line.
const SAMPLE_MT103: &str = include_str!("../../../testdata/mt/mt103.txt");
/// Repository-relative path shown in the UI for "open in editor".
const MAPPING_FILE: &str = "crates/mx20022-translate/src/mappings/mt103_to_pacs008.rs";

/// One provenance entry: a target field path and the code that produced it.
struct MapEntry {
    /// Target field path (e.g. `GrpHdr/MsgId`, `IntrBkSttlmAmt/@Ccy`).
    path: String,
    /// 1-based first line of the snippet in the mapping source.
    line_start: usize,
    /// 1-based last line of the snippet in the mapping source.
    line_end: usize,
    /// The captured Rust snippet.
    snippet: String,
    /// The ISO 20022 model type referenced by the snippet, if detectable.
    ty: Option<String>,
}

/// Parse `// @maps <path>` markers and capture the following statement.
fn extract_provenance(src: &str) -> Vec<MapEntry> {
    let lines: Vec<&str> = src.lines().collect();
    let mut entries = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        if let Some(path) = trimmed.strip_prefix("// @maps ") {
            let path = path.trim().to_owned();
            let snippet_start = i + 1;
            let end = capture_statement_end(&lines, snippet_start);
            let snippet = dedent(&lines[snippet_start..=end]);
            let ty = detect_type(&snippet);
            entries.push(MapEntry {
                path,
                line_start: snippet_start + 1,
                line_end: end + 1,
                snippet,
                ty,
            });
            i = end + 1;
        } else {
            i += 1;
        }
    }
    entries
}

/// Find the last line index of the statement that starts at `start`.
///
/// A chain element (a line starting with `.`) is a single line; otherwise the
/// statement runs until a delimiter-balanced line ending in `;` or `,`.
fn capture_statement_end(lines: &[&str], start: usize) -> usize {
    if lines[start].trim_start().starts_with('.') {
        return start;
    }
    let mut depth: i32 = 0;
    let mut i = start;
    while i < lines.len() {
        for ch in lines[i].chars() {
            match ch {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                _ => {}
            }
        }
        let end = lines[i].trim_end();
        if depth <= 0 && (end.ends_with(';') || end.ends_with(',')) {
            return i;
        }
        i += 1;
    }
    lines.len() - 1
}

/// Remove the common leading indentation from a block of lines.
fn dedent(lines: &[&str]) -> String {
    let indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| if l.len() >= indent { &l[indent..] } else { l })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Detect the first `pacs008::Ident` type referenced by a snippet.
fn detect_type(snippet: &str) -> Option<String> {
    let marker = "pacs008::";
    let pos = snippet.find(marker)? + marker.len();
    let ident: String = snippet[pos..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if ident.is_empty() {
        None
    } else {
        Some(ident)
    }
}

/// Build a map of `type name -> doc/constraint lines` from the model source.
fn extract_type_docs(src: &str) -> BTreeMap<String, Vec<String>> {
    let mut docs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut buf: Vec<String> = Vec::new();
    for line in src.lines() {
        let trimmed = line.trim_start();
        if let Some(doc) = trimmed.strip_prefix("///") {
            buf.push(doc.trim().to_owned());
        } else if trimmed.starts_with("#[") || trimmed.is_empty() {
            // attribute or blank line between docs and the item: keep buffer
        } else {
            if let Some(name) = type_name(trimmed) {
                if !buf.is_empty() {
                    docs.insert(name, buf.clone());
                }
            }
            buf.clear();
        }
    }
    docs
}

/// Extract the identifier from a `pub struct X`/`pub enum X` declaration.
fn type_name(line: &str) -> Option<String> {
    for kw in ["pub struct ", "pub enum "] {
        if let Some(rest) = line.strip_prefix(kw) {
            let ident: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !ident.is_empty() {
                return Some(ident);
            }
        }
    }
    None
}

/// Render the XML into a clickable HTML tree, returning the markup and every
/// addressable node path (for diagnostics).
fn render_xml(xml: &str) -> Result<(String, Vec<String>), Box<dyn std::error::Error>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut html = String::new();
    let mut paths = Vec::new();
    let mut stack: Vec<String> = Vec::new();

    loop {
        match reader.read_event()? {
            Event::Start(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                stack.push(name.clone());
                let path = stack.join("/");
                paths.push(path.clone());
                write_open(&mut html, &name, &path);
                write_attrs(&mut html, &mut paths, &e, &path);
            }
            Event::Empty(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                stack.push(name.clone());
                let path = stack.join("/");
                stack.pop();
                paths.push(path.clone());
                write_open(&mut html, &name, &path);
                write_attrs(&mut html, &mut paths, &e, &path);
                html.push_str("</div>");
            }
            Event::Text(e) => {
                let txt = e.xml_content()?.into_owned();
                if !txt.trim().is_empty() {
                    write!(html, "<span class=\"val\">{}</span>", esc(txt.trim())).unwrap();
                }
            }
            Event::End(_) => {
                html.push_str("</div>");
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok((html, paths))
}

/// Emit the opening markup for an element node.
fn write_open(html: &mut String, name: &str, path: &str) {
    write!(
        html,
        "<div class=\"node\"><span class=\"tag\" data-path=\"{}\" onclick=\"sel(this)\">{}</span>",
        esc(path),
        esc(name)
    )
    .unwrap();
}

/// Emit clickable rows for an element's attributes and record their paths.
fn write_attrs(html: &mut String, paths: &mut Vec<String>, e: &BytesStart, path: &str) {
    for attr in e.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let val = String::from_utf8_lossy(&attr.value).into_owned();
        let apath = format!("{path}/@{key}");
        paths.push(apath.clone());
        write!(
            html,
            "<span class=\"attr\" data-path=\"{}\" onclick=\"sel(this)\">@{}={}</span>",
            esc(&apath),
            esc(&key),
            esc(&val)
        )
        .unwrap();
    }
}

/// HTML-escape a string.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let input = match args.get(1) {
        Some(p) => std::fs::read_to_string(p)?,
        None => SAMPLE_MT103.to_owned(),
    };
    let out_path = PathBuf::from(
        args.get(2)
            .cloned()
            .unwrap_or_else(|| "target/explorer/index.html".to_owned()),
    );

    // Convert MT103 -> pacs.008 using the real engine.
    let msg = parse(&input)?;
    let mt103 = parse_mt103(&msg.block4)?;
    let result = mt103_to_pacs008(&mt103, "DEMO1", "2026-06-03T10:00:00")?;
    let xml = mx20022_parse::ser::to_string_with_declaration(&result.message)?;

    let entries = extract_provenance(MAPPING_SRC);
    let type_docs = extract_type_docs(MODEL_SRC);
    let (tree_html, node_paths) = render_xml(&xml)?;

    // Diagnostics: report annotations that match no node in the sample output.
    for e in &entries {
        let matched = node_paths.iter().any(|p| seg_contains(p, &e.path));
        if !matched {
            eprintln!(
                "warning: @maps '{}' did not match any node in sample output",
                e.path
            );
        }
    }

    let page = build_page(&tree_html, &entries, &type_docs);
    if let Some(dir) = out_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&out_path, page)?;
    println!(
        "wrote {} ({} mapped fields, {} nodes)",
        out_path.display(),
        entries.len(),
        node_paths.len()
    );
    Ok(())
}

/// True if `entry` appears as a contiguous segment run inside `node`.
fn seg_contains(node: &str, entry: &str) -> bool {
    let n: Vec<&str> = node.split('/').collect();
    let e: Vec<&str> = entry.split('/').collect();
    if e.len() > n.len() {
        return false;
    }
    (0..=n.len() - e.len()).any(|i| n[i..i + e.len()] == e[..])
}

/// Assemble the final self-contained HTML page.
fn build_page(
    tree_html: &str,
    entries: &[MapEntry],
    type_docs: &BTreeMap<String, Vec<String>>,
) -> String {
    let index_json = serde_json::to_string(&entries_to_json(entries, type_docs))
        .unwrap_or_else(|_| "[]".to_owned());
    format!(
        "{HTML_HEAD}<main><section id=\"tree\">{tree_html}</section>\
         <aside id=\"panel\"><div class=\"hint\">Кликните поле слева</div></aside></main>\
         <script>const INDEX={index_json};{HTML_JS}</script></body></html>",
    )
}

/// Convert entries + type docs into the JSON consumed by the page script.
fn entries_to_json(
    entries: &[MapEntry],
    type_docs: &BTreeMap<String, Vec<String>>,
) -> serde_json::Value {
    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            let constraints =
                e.ty.as_ref()
                    .and_then(|t| type_docs.get(t))
                    .cloned()
                    .unwrap_or_default();
            serde_json::json!({
                "path": e.path,
                "file": MAPPING_FILE,
                "line_start": e.line_start,
                "line_end": e.line_end,
                "snippet": e.snippet,
                "type": e.ty,
                "constraints": constraints,
            })
        })
        .collect();
    serde_json::Value::Array(items)
}

const HTML_HEAD: &str = r#"<!doctype html><html lang="ru"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>mx20022 — Mapping Explorer (MT103 → pacs.008)</title>
<style>
:root{color-scheme:light dark}
body{font-family:system-ui,sans-serif;margin:0}
header{padding:.8rem 1rem;border-bottom:1px solid #8884;font-weight:600}
main{display:grid;grid-template-columns:1fr 1fr;gap:0;height:calc(100vh - 52px)}
#tree{overflow:auto;padding:1rem;border-right:1px solid #8884;font-family:ui-monospace,monospace;font-size:.82rem}
#panel{overflow:auto;padding:1rem}
.node{margin-left:1rem}
.tag{cursor:pointer;color:#2563eb;font-weight:600}
.tag:hover{text-decoration:underline}
.attr{cursor:pointer;color:#9333ea;margin-left:.4rem}
.val{margin-left:.4rem;opacity:.8}
.sel{background:#2563eb22;outline:1px solid #2563eb}
pre{background:#8881;padding:.6rem;border-radius:6px;overflow:auto;font-size:.8rem}
.k{opacity:.6;font-size:.8rem}
.hint{opacity:.6}
code{background:#8882;padding:.05rem .3rem;border-radius:4px}
li{margin:.15rem 0}
</style></head><body>
<header>mx20022 — Mapping Explorer · MT103 → pacs.008 <span class="k">(read-only, M1)</span></header>"#;

const HTML_JS: &str = r#"
function segContains(node, entry){
  const n=node.split('/'), e=entry.split('/');
  if(e.length>n.length) return -1;
  let best=-1;
  for(let i=0;i+e.length<=n.length;i++){
    let ok=true; for(let j=0;j<e.length;j++){ if(n[i+j]!==e[j]){ok=false;break;} }
    if(ok) best=i+e.length;
  }
  return best;
}
function lookup(path){
  let chosen=null,bestEnd=-1,bestLen=-1;
  for(const en of INDEX){
    const end=segContains(path,en.path);
    if(end<0) continue;
    const len=en.path.split('/').length;
    if(end>bestEnd||(end===bestEnd&&len>bestLen)){bestEnd=end;bestLen=len;chosen=en;}
  }
  return chosen;
}
let last=null;
function sel(el){
  const path=el.getAttribute('data-path');
  if(last) last.classList.remove('sel'); el.classList.add('sel'); last=el;
  const en=lookup(path);
  const p=document.getElementById('panel');
  if(!en){ p.innerHTML='<div class="hint">Для <code>'+path+'</code> провенанс пока не размечен.</div>'; return; }
  let h='<div class="k">Поле</div><div><code>'+path+'</code></div>';
  if(en.type){ h+='<div class="k" style="margin-top:.6rem">Тип ISO 20022</div><div><code>'+en.type+'</code></div>'; }
  if(en.constraints && en.constraints.length){
    h+='<div class="k" style="margin-top:.6rem">Ограничения / описание</div><ul>';
    for(const c of en.constraints) h+='<li>'+c+'</li>';
    h+='</ul>';
  }
  h+='<div class="k" style="margin-top:.6rem">Код, формирующий поле</div>';
  h+='<div><code>'+en.file+':'+en.line_start+'-'+en.line_end+'</code></div>';
  h+='<pre>'+en.snippet.replace(/&/g,'&amp;').replace(/</g,'&lt;')+'</pre>';
  p.innerHTML=h;
}
"#;
