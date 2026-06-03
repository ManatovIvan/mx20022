# mx20022-explorer (M1, read-only)

A developer tool that links every field of a converted ISO 20022 message back
to the Rust snippet in `mx20022-translate` that produced it, and shows the
field's ISO 20022 type and XSD constraints.

It does **not** change the translation logic. It reads:

- `// @maps <path>` provenance markers placed in the mapping source, and
- the doc comments the code generator emits on the model types.

This first milestone covers **MT103 → pacs.008** and is **read-only** (no
editing yet).

## Run

```bash
# from the repository root
cargo run -p mx20022-explorer
# -> writes target/explorer/index.html, then open it in a browser
```

Optional arguments:

```bash
cargo run -p mx20022-explorer -- path/to/message.mt103   # use your own MT103
cargo run -p mx20022-explorer -- path/to/message.mt103 out.html
```

With no arguments it uses the bundled sample (`testdata/mt/mt103.txt`).

## How it works

1. Converts the input MT103 to pacs.008 with the real `mt103_to_pacs008`
   mapping.
2. Renders the produced XML as a clickable tree (every element and attribute
   carries its path).
3. Builds a provenance index from the `// @maps` markers in
   `crates/mx20022-translate/src/mappings/mt103_to_pacs008.rs`.
4. Builds a field catalog (type + constraints) from the generated model
   `crates/mx20022-model/src/generated/pacs/pacs_008_001_13.rs`.
5. Clicking a field shows: the field path, its ISO 20022 type, the XSD
   constraints, and the exact Rust snippet (`file:line`) that produced it.

On generation it prints a diagnostic for any `@maps` marker that does not match
a node in the sample output (e.g. `RmtInf` when the sample has no `:70:`).

## Adding markers

Annotate a mapping statement with the target field path:

```rust
// @maps GrpHdr/MsgId
.msg_id(pacs008::Max35Text(msg_id.to_string()))
```

Paths are matched as a contiguous segment run, so `Dbtr` matches both the
`Dbtr` element and its descendants; use `IntrBkSttlmAmt/@Ccy` for attributes.

## Roadmap

- M2: in-browser editing of the snippet with `cargo build` feedback and, in a
  dev environment only (`MX20022_DEV_EDIT=1`), automatic rebuild + restart.
  The production path stays git/PR/CI.
- Cover the remaining five mapping directions.
