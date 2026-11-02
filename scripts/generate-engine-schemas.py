#!/usr/bin/env python3
"""Generates `shared/data/engine-schemas.json` – the form schema of every engine action.

The frontend's generic tool UI (`EngineToolRunner`) renders proper inputs
(file pickers, numbers, switches, selects, colour pickers, lists…) from this
file instead of asking the user for a raw JSON payload.

The schema is derived from the Rust sources so it can never drift from the
engines: for every crate in the workspace the `export_engine!(handler, Enum)`
line identifies the action enum; variants, their fields, `#[serde(...)]`
attributes, `Default` impls / `default = "fn"` helpers and the `///` doc
comments are parsed. Doc comments drive the widgets:

* `` `a` (default), `b` or `c` `` style docs become `<select>` options,
* ranges such as `0–1`, `1–100` become slider/number bounds,
* names like `color`/`background` become colour pickers,
* `text`, `html`, `markdown`, `sql`… become multi-line editors.

Usage: python3 scripts/generate-engine-schemas.py [--check]
"""

from __future__ import annotations

import glob
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ENGINES_DIR = os.path.join(ROOT, "engines")
OUT = os.path.join(ROOT, "shared", "data", "engine-schemas.json")

# crate dir -> engine id (must match scripts/build-engines.mjs)
ENGINE_IDS = {
    "text": "text-engine",
    "pdf": "pdf-engine",
    "image": "image-engine",
    "video": "video-engine",
    "audio": "audio-engine",
    "archive": "archive-engine",
    "json": "json-engine",
    "csv": "csv-engine",
    "spreadsheet": "spreadsheet-engine",
    "crypto": "crypto-engine",
    "encoding": "encoding-engine",
    "math": "math-engine",
    "units": "units-engine",
    "datetime": "datetime-engine",
    "finance": "finance-engine",
    "indiatax": "indiatax-engine",
    "construction": "construction-engine",
    "solar": "solar-engine",
    "business": "business-engine",
    "developer": "developer-engine",
    "qrbarcode": "qrbarcode-engine",
    "geo": "geo-engine",
    "colordesign": "colordesign-engine",
    "dataviz": "dataviz-engine",
    "document": "document-engine",
    "font": "font-engine",
    "mesh3d": "mesh3d-engine",
}

# Library crates whose structs may be referenced by engines.
SHARED_DIRS = ["shared", "tabular", "colorcore"]

FILE_ACCEPT = {
    "pdf-engine": ".pdf,application/pdf",
    "image-engine": "image/*",
    "video-engine": "video/*,.mp4,.m4v,.mov,.mkv,.webm,.avi",
    "audio-engine": "audio/*,.mp3,.wav,.flac,.ogg,.m4a,.aac,.mp4,.mkv",
    "archive-engine": ".zip,.tar,.gz,.tgz,.7z,.rar",
    "spreadsheet-engine": ".xlsx,.xlsm,.xlsb,.xls,.ods,.csv",
    "csv-engine": ".csv,.tsv,.txt",
    "document-engine": ".docx,.pptx,.odt,.rtf,.epub,.mobi,.azw,.md,.txt",
    "font-engine": ".ttf,.otf,.woff,.woff2,.ttc",
    "mesh3d-engine": ".stl,.obj,.gltf,.glb",
    "qrbarcode-engine": "image/*",
    "geo-engine": ".geojson,.json,.kml,.csv",
    "colordesign-engine": "image/*",
    "dataviz-engine": ".csv,.json",
    "json-engine": ".json,.yaml,.yml,.xml,.toml,.csv,.ini,.plist",
}
IMAGE_FIELDS = {"image", "overlay", "logo", "frames", "images", "stamp", "sample"}
TEXTAREA_NAMES = {
    "text", "html", "markdown", "sql", "css", "js", "source", "json", "yaml", "toml", "xml", "csv",
    "code", "content", "template", "patch", "schema", "data", "body", "notes", "terms", "wkt", "kml",
    "svg", "dotenv", "ini", "plist", "books", "portal", "old", "new", "a", "b", "codes", "expression",
    "ciphertext", "pem", "certificate", "private_key_pem", "public_key_pem", "token", "statement",
    "query", "url", "user_agent", "description", "message", "ua", "equation", "hash", "signature",
}
COLOR_RE = re.compile(r"(^|_)(colou?r|background|foreground|fill|stroke|highlight|background_end|title_color|subtitle_color|label_color)$")
AUTO_FIELDS = {
    "entropy": "entropy",
    "now": "now_iso",
    "now_unix": "now_unix",
    "timestamp_unix": "now_unix",
    "timestamp_ms": "now_ms",
    "today_serial": "today_serial",
}
INT_TYPES = {"u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "i128", "u128"}
FLOAT_TYPES = {"f32", "f64"}

DOC_OPTION_RE = re.compile(r"`([A-Za-z0-9_+\-./:%#*]+)`")
RANGE_RE = re.compile(r"(?<![\w.])(-?\d+(?:\.\d+)?)\s*[–-]\s*(-?\d+(?:\.\d+)?)(?![\w]|\.\d)")


# ----------------------------------------------------------------------------- parsing


def strip_comments_keep_docs(src: str) -> str:
    """Removes `//` comments that are not doc comments (keeps string literals intact)."""
    out = []
    i = 0
    n = len(src)
    while i < n:
        c = src[i]
        if c == '"':
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == '"':
                    break
                j += 1
            out.append(src[i : j + 1])
            i = j + 1
            continue
        if src.startswith("///", i) or src.startswith("//!", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            out.append(src[i:j])
            i = j
            continue
        if src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j
            continue
        out.append(c)
        i += 1
    return "".join(out)


def _skip_token(src: str, i: int) -> int:
    """If a string/char literal or a line comment starts at `i`, returns the index after it."""
    n = len(src)
    c = src[i]
    if c == '"':
        j = i + 1
        while j < n:
            if src[j] == "\\":
                j += 2
                continue
            if src[j] == '"':
                return j + 1
            j += 1
        return n
    if c == "'":
        m = re.match(r"'(\\u\{[0-9A-Fa-f]+\}|\\.|[^\\'])'", src[i : i + 12])
        if m:
            return i + m.end()
        return i
    if src.startswith("//", i):
        j = src.find("\n", i)
        return n if j < 0 else j
    return i


def find_block(src: str, open_idx: int) -> int:
    """Index of the `}` matching the `{` at `open_idx` (strings, chars and comments are skipped)."""
    depth = 0
    i = open_idx
    n = len(src)
    while i < n:
        k = _skip_token(src, i)
        if k != i:
            i = k
            continue
        c = src[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    raise ValueError("unbalanced braces")


def split_top(body: str, sep: str = ",") -> list[str]:
    """Splits at top-level separators (ignores brackets, strings, chars and comments)."""
    parts = []
    depth = 0
    last = 0
    i = 0
    n = len(body)
    while i < n:
        k = _skip_token(body, i)
        if k != i:
            i = k
            continue
        c = body[i]
        if c in "([{<":
            depth += 1
        elif c in ")]}>":
            depth -= 1
        elif c == sep and depth == 0:
            parts.append(body[last:i])
            last = i + 1
        i += 1
    parts.append(body[last:])
    return [p for p in parts if p.strip()]


ATTR_RE = re.compile(r"#\[(serde)\(([^\]]*)\)\]")


def parse_members(body: str):
    """Parses struct fields / enum variants: yields (doc, serde attrs, text).

    Leading `///` lines and `#[...]` attributes belong to the member; everything
    after them (which may contain nested docs/attrs of inner fields) is the
    member text.
    """
    for item in split_top(body, ","):
        lines = item.strip().split("\n")
        docs = []
        attrs = []
        rest = []
        for line in lines:
            s = line.strip()
            if rest:
                rest.append(s)
            elif s.startswith("///"):
                docs.append(s[3:].strip())
            elif s.startswith("#["):
                attrs.append(s)
            elif s:
                rest.append(s)
        text = "\n".join(rest).strip()
        if not text:
            continue
        serde = {}
        for a in attrs:
            m = ATTR_RE.search(a)
            if not m:
                continue
            for part in split_top(m.group(2), ","):
                part = part.strip()
                if "=" in part:
                    k, v = part.split("=", 1)
                    serde[k.strip()] = v.strip().strip('"')
                else:
                    serde[part] = True
        yield " ".join(docs).strip(), serde, text


class Crate:
    def __init__(self, name: str, files: list[str]):
        self.name = name
        self.src = {f: strip_comments_keep_docs(open(f, encoding="utf-8").read()) for f in files}
        self.structs: dict[str, dict] = {}
        self.enums: dict[str, dict] = {}
        self.default_fns: dict[str, str] = {}
        self.default_impls: dict[str, dict[str, str]] = {}
        for f, text in self.src.items():
            self._parse_items(text)

    def _parse_items(self, text: str):
        # default helper fns: fn name() -> T { expr }
        for m in re.finditer(r"fn\s+([a-z_0-9]+)\s*\(\)\s*->\s*[^{]+\{", text):
            end = find_block(text, m.end() - 1)
            body = text[m.end() : end].strip()
            if "\n" not in body or len(body) < 200:
                self.default_fns[m.group(1)] = body.strip().rstrip(";").strip()
        # impl Default for X { fn default() -> Self { Self { a: 1, ... } } }
        for m in re.finditer(r"impl\s+Default\s+for\s+([A-Za-z0-9_]+)\s*\{", text):
            end = find_block(text, m.end() - 1)
            body = text[m.end() : end]
            fn = re.search(r"fn\s+default\s*\(\)\s*->\s*Self\s*\{", body)
            if not fn:
                continue
            lit = re.compile(r"(?:Self|%s)\s*\{" % m.group(1)).search(body, fn.end())
            if not lit:
                continue
            lend = find_block(body, lit.end() - 1)
            fields = {}
            for part in split_top(body[lit.end() : lend], ","):
                if ":" in part:
                    k, v = part.split(":", 1)
                    fields[k.strip()] = v.strip()
            self.default_impls[m.group(1)] = fields
        # structs / enums with preceding docs and attrs
        for m in re.finditer(r"((?:\s*(?:///[^\n]*|#\[[^\n]*)\n)*)\s*pub\s+(struct|enum)\s+([A-Za-z0-9_]+)(?:<[^{;(]*>)?\s*(\{|;|\()", text):
            kind, name, opener = m.group(2), m.group(3), m.group(4)
            head = m.group(1)
            docs = " ".join(l.strip()[3:].strip() for l in head.split("\n") if l.strip().startswith("///"))
            serde = {}
            derive_default = "Default" in "".join(re.findall(r"#\[derive\(([^)]*)\)\]", head))
            for a in re.findall(r"#\[serde\(([^\]]*)\)\]", head):
                for part in split_top(a, ","):
                    part = part.strip()
                    if "=" in part:
                        k, v = part.split("=", 1)
                        serde[k.strip()] = v.strip().strip('"')
                    else:
                        serde[part] = True
            if opener != "{":
                continue
            end = find_block(text, m.end() - 1)
            body = text[m.end() : end]
            entry = {"doc": docs, "serde": serde, "derive_default": derive_default, "body": body}
            if kind == "struct":
                self.structs[name] = entry
            else:
                self.enums[name] = entry


# --------------------------------------------------------------------------- type model


def parse_type(t: str):
    t = " ".join(t.split())
    m = re.match(r"^([A-Za-z0-9_:]+)\s*<(.*)>$", t)
    if m:
        name = m.group(1).split("::")[-1]
        args = [parse_type(a) for a in split_top(m.group(2), ",")]
        return (name, args)
    m = re.match(r"^\[(.*);\s*(\d+)\]$", t)
    if m:
        return ("Array", [parse_type(m.group(1)), int(m.group(2))])
    m = re.match(r"^\((.*)\)$", t)
    if m:
        return ("Tuple", [parse_type(a) for a in split_top(m.group(1), ",")])
    return (t.split("::")[-1], [])


def rename(name: str, style: str | None) -> str:
    if style == "snake_case":
        out = re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()
        return out
    if style == "kebab-case":
        return re.sub(r"(?<!^)(?=[A-Z])", "-", name).lower()
    if style == "lowercase":
        return name.lower()
    if style == "UPPERCASE":
        return name.upper()
    if style == "camelCase":
        return name[0].lower() + name[1:]
    return name


def label_for(name: str) -> str:
    words = name.replace("_", " ").split()
    out = []
    for w in words:
        if w.lower() in {"dpi", "url", "html", "json", "pdf", "id", "css", "svg", "png", "jpeg", "gst", "tds", "pan", "hsn", "sac", "ppf", "emi", "roi", "npv", "irr", "cagr", "ctc", "hra", "pf", "utm", "wkt", "kml", "csv", "sql", "jwt", "rsa", "aes", "gcm", "aad", "otp", "ean", "upc", "qr", "ascii", "ocr", "api", "ssn", "fps", "bpm", "db", "ms", "px", "kb", "mb", "ip", "ipv4", "uuid", "ulid", "hmac", "rgb", "hsl", "cmyk", "lab", "xml", "yaml", "toml", "ini", "jsonl", "stl", "obj", "glb", "gltf", "ttf", "otf", "woff", "eq", "iso", "utc", "3d", "2d", "tts", "hoa", "pmi", "sip", "fd", "rd", "ltcg", "stcg", "ebitda", "pe", "cac", "ltv", "eoq", "itc", "igst", "cgst", "sgst", "gstr", "rcm", "b2b", "b2c"}:
            out.append(w.upper())
        else:
            out.append(w)
    s = " ".join(out)
    return s[:1].upper() + s[1:]


def option_label(value: str) -> str:
    if re.match(r"^[a-z][a-z0-9]*([_-][a-z0-9]+)*$", value):
        return label_for(value.replace("-", "_"))
    return value


DOC_DEFAULT_RE = re.compile(r"`([^`]+)`\s*(?:\((?:[^)]*,\s*)?default\)|default\b(?!\s*:))")


def doc_default(doc: str):
    m = DOC_DEFAULT_RE.search(doc or "")
    return m.group(1) if m else None


def literal_value(expr: str):
    """Best-effort evaluation of a Rust default expression to JSON."""
    e = expr.strip()
    e = re.sub(r"\.(into|to_string|to_owned|to_vec)\(\)$", "", e)
    e = re.sub(r"^String::from\((.*)\)$", r"\1", e)
    e = re.sub(r"^Some\((.*)\)$", r"\1", e)
    if e in ("true", "false"):
        return e == "true"
    if e in ("None", "String::new()", "Vec::new()", "vec![]", "Default::default()", "Map::new()"):
        return None
    if re.match(r"^-?\d+(_\d+)*$", e):
        return int(e.replace("_", ""))
    if re.match(r"^-?\d+(_\d+)*\.\d*(e-?\d+)?$", e):
        return float(e.replace("_", ""))
    m = re.match(r'^"(.*)"$', e, re.S)
    if m:
        return m.group(1).encode().decode("unicode_escape") if "\\" in m.group(1) else m.group(1)
    m = re.match(r"^vec!\[(.*)\]$", e, re.S)
    if m:
        return [literal_value(p) for p in split_top(m.group(1), ",")]
    m = re.match(r"^\[(.*)\]$", e, re.S)
    if m:
        return [literal_value(p) for p in split_top(m.group(1), ",")]
    m = re.match(r"^\[(.*);\s*(\d+)\]$", e, re.S)
    if m:
        return [literal_value(m.group(1))] * int(m.group(2))
    return None


class SchemaBuilder:
    def __init__(self, crate: Crate, shared: list[Crate], engine_id: str):
        self.crate = crate
        self.shared = shared
        self.engine_id = engine_id
        self.depth = 0

    # lookups -----------------------------------------------------------
    def find_struct(self, name: str):
        for c in [self.crate] + self.shared:
            if name in c.structs:
                return c, c.structs[name]
        return None, None

    def find_enum(self, name: str):
        for c in [self.crate] + self.shared:
            if name in c.enums:
                return c, c.enums[name]
        return None, None

    def default_fn(self, name: str):
        for c in [self.crate] + self.shared:
            if name in c.default_fns:
                return literal_value(c.default_fns[name])
        return None

    # fields ------------------------------------------------------------
    def struct_fields(self, name: str, prefix_doc: str = "") -> list[dict]:
        crate, st = self.find_struct(name)
        if st is None:
            return []
        struct_default = bool(st["serde"].get("default")) and st["derive_default"] or bool(st["serde"].get("default"))
        impl_defaults = crate.default_impls.get(name, {})
        fields = []
        for doc, serde, text in parse_members(st["body"]):
            m = re.match(r"^(?:pub(?:\([a-z]+\))?\s+)?(r#)?([A-Za-z0-9_]+)\s*:\s*(.+)$", text, re.S)
            if not m:
                continue
            fname = m.group(2)
            ftype = m.group(3).strip()
            if serde.get("skip"):
                continue
            f = self.field(fname, ftype, doc, serde, struct_default, impl_defaults.get(fname))
            if f is not None:
                fields.extend(f if isinstance(f, list) else [f])
        return fields

    def enum_options(self, name: str):
        crate, en = self.find_enum(name)
        if en is None:
            return None
        style = en["serde"].get("rename_all")
        if en["serde"].get("untagged"):
            return {"untagged": True}
        options = []
        tagged = en["serde"].get("tag")
        default_variant = None
        for doc, serde, text in parse_members(en["body"]):
            m = re.match(r"^([A-Za-z0-9_]+)\s*(\{.*\})?\s*(\(.*\))?$", text, re.S)
            if not m:
                continue
            vname = m.group(1)
            value = serde.get("rename") or rename(vname, style)
            if serde.get("alias"):
                pass
            opt = {"value": value, "label": label_for(rename(vname, "snake_case")), "doc": doc}
            if m.group(2):
                inner = m.group(2)[1:-1]
                opt["fields"] = self._inline_fields(inner, False, {})
            elif m.group(3):
                opt["tuple"] = True
            options.append(opt)
            if "#[default]" in en["body"] and re.search(r"#\[default\]\s*(///[^\n]*\s*)*%s\b" % vname, en["body"]):
                default_variant = value
        return {"tag": tagged, "options": options, "default": default_variant, "content": en["serde"].get("content")}

    def _inline_fields(self, body: str, struct_default: bool, impl_defaults: dict) -> list[dict]:
        fields = []
        for doc, serde, text in parse_members(body):
            m = re.match(r"^(?:pub\s+)?(r#)?([A-Za-z0-9_]+)\s*:\s*(.+)$", text, re.S)
            if not m:
                continue
            f = self.field(m.group(2), m.group(3).strip(), doc, serde, struct_default, impl_defaults.get(m.group(2)))
            if f is not None:
                fields.extend(f if isinstance(f, list) else [f])
        return fields

    def field(self, name: str, rust_type: str, doc: str, serde: dict, struct_default: bool, impl_default: str | None):
        if self.depth > 6:
            return None
        ty = parse_type(rust_type)
        wire = serde.get("rename", name)
        has_default = "default" in serde or struct_default
        default = None
        if isinstance(serde.get("default"), str):
            default = self.default_fn(serde["default"])
        elif impl_default is not None:
            default = literal_value(impl_default)
        if serde.get("flatten"):
            inner = self.struct_fields(ty[0])
            return inner
        base = {"name": wire, "label": label_for(name), "doc": doc}
        if wire in AUTO_FIELDS:
            base["auto"] = AUTO_FIELDS[wire]
        required = not has_default
        optional_wrapper = False
        if ty[0] == "Option":
            optional_wrapper = True
            required = False
            ty = ty[1][0]
        base["required"] = required
        if default is not None:
            base["default"] = default
        kind, extra = self.widget(name, ty, doc, serde, optional_wrapper)
        if kind is None:
            return None
        base["kind"] = kind
        base.update(extra)
        if "default" not in base and not optional_wrapper and has_default and kind in ("number", "integer"):
            base["default"] = 0
        if kind in ("number", "integer") and isinstance(base.get("default"), (int, float)):
            if "min" in base and base["default"] < base["min"]:
                base["min"] = base["default"]
            if "max" in base and base["default"] > base["max"]:
                base["max"] = base["default"]
        if kind in ("file", "files"):
            base["doc"] = re.sub(r"\s*\((?:base64|[^)]*base64[^)]*)\)", "", base["doc"]).replace("base64", "").strip()
        if kind == "select" and "default" not in base:
            dd = doc_default(doc)
            if dd and any(o["value"] == dd for o in base.get("options", [])):
                base["default"] = dd
        if "default" not in base and not optional_wrapper and has_default and kind in ("boolean",):
            base["default"] = False
        if "default" not in base and not optional_wrapper and has_default and kind in ("number", "integer"):
            base["default"] = 0
        if "default" not in base and not optional_wrapper and has_default and kind in ("text", "textarea"):
            base["default"] = ""
        return base

    def widget(self, name: str, ty, doc: str, serde: dict, optional: bool):
        tname, args = ty
        lname = name.lower()
        with_ = serde.get("with", "")
        # files --------------------------------------------------------
        if tname == "Vec" and args and args[0][0] == "u8" or (tname == "Vec" and "b64" in with_ and args and args[0][0] != "Vec"):
            return "file", {"accept": self.accept_for(lname)}
        if tname == "Vec" and args and args[0][0] == "Vec" and args[0][1] and args[0][1][0][0] == "u8":
            return "files", {"accept": self.accept_for(lname)}
        if tname == "String" and ("b64" in with_ or ("base64" in doc and (lname in IMAGE_FIELDS or "image" in lname or "file" in lname))):
            return "file", {"accept": self.accept_for(lname)}
        # scalars ------------------------------------------------------
        if tname == "bool":
            return "boolean", {}
        if tname in INT_TYPES or tname in FLOAT_TYPES:
            extra = {}
            rng = RANGE_RE.search(doc)
            if rng:
                lo, hi = float(rng.group(1)), float(rng.group(2))
                if lo < hi and hi - lo >= 1 and not (lo == 0 and hi > 1000):
                    extra["min"], extra["max"] = lo, hi
                    if (hi - lo) <= 100 and ("opacity" in lname or "quality" in lname or "strength" in lname or "percent" in lname or hi in (1.0, 100.0, 255.0)):
                        extra["slider"] = True
            if "percent" in lname:
                extra.setdefault("unit", "%")
            if lname.endswith("_db") or lname.endswith("db"):
                extra.setdefault("unit", "dB")
            if lname.endswith("_mm"):
                extra.setdefault("unit", "mm")
            if lname.endswith("_px") or lname == "width" or lname == "height":
                pass
            if lname.endswith("_seconds") or lname == "seconds":
                extra.setdefault("unit", "s")
            if lname.endswith("_ms"):
                extra.setdefault("unit", "ms")
            if "frequency" in lname:
                extra.setdefault("unit", "Hz")
            return ("integer" if tname in INT_TYPES else "number"), extra
        if tname == "char":
            return "text", {"maxLength": 1}
        if tname == "String" or tname == "str":
            if COLOR_RE.search(lname):
                return "color", {}
            options = self.doc_options(doc)
            if options:
                return "select", {"options": options, "allowCustom": "…" in doc or "e.g." in doc or "etc" in doc}
            if lname in TEXTAREA_NAMES or re.search(r"\b(source|text|JSON|CSV|YAML|XML|Markdown|HTML|SQL|snippet|paragraph|lines)\b", doc):
                if lname in {"name", "title", "label", "separator", "prefix", "suffix", "pattern", "format", "delimiter", "unit", "kind", "mode"}:
                    return "text", {}
                return "textarea", {}
            return "text", {}
        # containers ---------------------------------------------------
        if tname == "Value":
            return "json", {}
        if tname in ("Map", "HashMap", "BTreeMap"):
            if args and len(args) == 2 and args[1][0] in INT_TYPES | FLOAT_TYPES:
                return "keyvalues", {"valueKind": "number"}
            if args and len(args) == 2 and args[1][0] == "String":
                return "keyvalues", {"valueKind": "text"}
            if args and len(args) == 2 and args[1][0] == "Value":
                return "keyvalues", {"valueKind": "any"}
            return "json", {}
        if tname == "Array" and args:
            inner, n = args
            if inner[0] in FLOAT_TYPES or inner[0] in INT_TYPES:
                return "numbers", {"fixed": n}
            return "json", {}
        if tname == "Vec" and args:
            inner = args[0]
            if inner[0] == "String":
                opts = self.doc_options(doc)
                extra = {"suggestions": [o["value"] for o in opts]} if opts else {}
                return "tags", extra
            if inner[0] in FLOAT_TYPES or inner[0] in INT_TYPES:
                return "numbers", {}
            if inner[0] == "Vec" and inner[1] and (inner[1][0][0] in FLOAT_TYPES or inner[1][0][0] in INT_TYPES):
                return "matrix", {}
            if inner[0] == "Vec" and inner[1] and inner[1][0][0] == "Option":
                return "matrix", {}
            if inner[0] == "Value":
                return "json", {"array": True}
            if inner[0] == "Tuple":
                return "json", {"array": True}
            # list of structs / tagged enums
            c, st = self.find_struct(inner[0])
            if st is not None:
                if self.depth >= 2:
                    return None, {}  # recursive structures (bookmark children) stop here
                self.depth += 1
                fields = self.struct_fields(inner[0])
                self.depth -= 1
                return "list", {"item": {"kind": "group", "fields": fields}}
            en = self.enum_options(inner[0])
            if en and en.get("options") is not None:
                if en.get("tag"):
                    return "list", {"item": {"kind": "variant", "tag": en["tag"], "variants": en["options"]}}
                return "tags", {"suggestions": [o["value"] for o in en["options"]]}
            return "json", {"array": True}
        # structs / enums ----------------------------------------------
        c, st = self.find_struct(tname)
        if st is not None:
            self.depth += 1
            fields = self.struct_fields(tname)
            self.depth -= 1
            return "group", {"fields": fields, "collapsible": True}
        en = self.enum_options(tname)
        if en is not None:
            if en.get("untagged"):
                return "text", {"placeholder": "e.g. 1-3,5"}
            if en.get("tag"):
                return "variant", {"tag": en["tag"], "variants": en["options"]}
            opts = [{"value": o["value"], "label": o["label"], "doc": o.get("doc", "")} for o in en["options"]]
            extra = {"options": opts}
            if en.get("default"):
                extra["default"] = en["default"]
            return "select", extra
        return "json", {}

    def accept_for(self, lname: str) -> str:
        if lname in IMAGE_FIELDS or lname.startswith("image"):
            return "image/*"
        if lname == "overlay_file":
            return ".pdf,application/pdf"
        return FILE_ACCEPT.get(self.engine_id, "*/*")

    @staticmethod
    def doc_options(doc: str):
        if not doc:
            return None
        tokens = DOC_OPTION_RE.findall(doc)
        tokens = [t for t in tokens if not t.startswith("E_") and "(" not in t and len(t) <= 24]
        tokens = [t for t in tokens if not re.match(r"^[A-Z][A-Za-z]+::", t)]
        if len(tokens) < 2:
            return None
        listing = "(default)" in doc or " or " in doc or "/" in doc.replace("`", "") or "," in doc
        if not listing:
            return None
        # Docs like "`WIFI:` payload" or "supports `$1`" are not listings.
        if re.search(r"\b(such as|like|e\.g\.|for example|supports?|contains?|uses?|matching)\b", doc) and "(default)" not in doc:
            return None
        seen = []
        for t in tokens:
            if t not in seen:
                seen.append(t)
        return [{"value": t, "label": option_label(t)} for t in seen]


def units_hook(actions: dict):
    """Adds the unit ids from `units/src/tables.rs` as suggestions / category options."""
    src = open(os.path.join(ENGINES_DIR, "units", "src", "tables.rs"), encoding="utf-8").read()
    categories = []
    units = []
    for m in re.finditer(r"pub static [A-Z_]+: Category = Category \{(.*?)\n\};", src, re.S):
        block = m.group(1)
        cid = re.search(r'id:\s*"([^"]+)"', block)
        cname = re.search(r'name:\s*"([^"]+)"', block)
        if not cid:
            continue
        categories.append({"value": cid.group(1), "label": cname.group(1) if cname else cid.group(1)})
        for u in re.finditer(r'u!\(\s*"([^"]+)",\s*"([^"]+)"', block):
            units.append(u.group(1))
    seen = []
    for u in units:
        if u not in seen:
            seen.append(u)
    for name in ("convert", "units"):
        for f in actions.get(name, {}).get("fields", []):
            if f["name"] in ("from", "to"):
                f["suggestions"] = seen
            if f["name"] == "category":
                f["kind"] = "select"
                f["options"] = categories
                f["allowCustom"] = False


ENGINE_HOOKS = {"units-engine": units_hook}


def action_enum_name(crate: Crate):
    for text in crate.src.values():
        m = re.search(r"export_engine!\s*\(\s*[A-Za-z0-9_:]+\s*,\s*([A-Za-z0-9_:]+)\s*\)", text)
        if m:
            return m.group(1)
    return None


def build_engine(crate_dir: str, engine_id: str, shared: list[Crate]):
    files = sorted(glob.glob(os.path.join(ENGINES_DIR, crate_dir, "src", "**", "*.rs"), recursive=True))
    crate = Crate(crate_dir, files)
    enum_name = action_enum_name(crate)
    if enum_name:
        enum_name = enum_name.split("::")[-1]
    holder = next((c for c in [crate] + shared if enum_name in c.enums), None) if enum_name else None
    if holder is None:
        print(f"warning: no action enum for {crate_dir}", file=sys.stderr)
        return None
    en = holder.enums[enum_name]
    builder = SchemaBuilder(holder, [crate] + shared, engine_id)
    style = en["serde"].get("rename_all", "snake_case")
    actions = {}
    for doc, serde, text in parse_members(en["body"]):
        m = re.match(r"^([A-Za-z0-9_]+)\s*(\{.*\})?$", text, re.S)
        if not m:
            continue
        name = serde.get("rename") or rename(m.group(1), style)
        fields = builder._inline_fields(m.group(2)[1:-1], False, {}) if m.group(2) else []
        actions[name] = {"doc": doc, "fields": fields}
    if engine_id in ENGINE_HOOKS:
        ENGINE_HOOKS[engine_id](actions)
    return {"actions": actions, "crate": crate_dir, "doc": crate_doc(crate)}


def crate_doc(crate: Crate) -> str:
    lib = next((t for f, t in crate.src.items() if f.endswith("lib.rs")), "")
    lines = []
    for line in lib.split("\n"):
        if line.startswith("//!"):
            lines.append(line[3:].strip())
        elif lines:
            break
    text = " ".join(l for l in lines if l and not l.startswith("#") and not l.startswith("|")).strip()
    return re.split(r"(?<=[.!?])\s", text, maxsplit=1)[0][:200]


def main() -> int:
    check = "--check" in sys.argv
    shared = [Crate(d, sorted(glob.glob(os.path.join(ENGINES_DIR, d, "src", "**", "*.rs"), recursive=True))) for d in SHARED_DIRS]
    engines = {}
    for crate_dir, engine_id in ENGINE_IDS.items():
        built = build_engine(crate_dir, engine_id, shared)
        if built:
            engines[engine_id] = built
    out = {"engines": engines}
    text = json.dumps(out, indent=2, ensure_ascii=False) + "\n"
    if check:
        current = open(OUT, encoding="utf-8").read() if os.path.exists(OUT) else ""
        if current != text:
            print("shared/data/engine-schemas.json is out of date; run scripts/generate-engine-schemas.py")
            return 1
        return 0
    with open(OUT, "w", encoding="utf-8") as fh:
        fh.write(text)
    n_actions = sum(len(e["actions"]) for e in engines.values())
    n_fields = sum(len(a["fields"]) for e in engines.values() for a in e["actions"].values())
    print(f"wrote {os.path.relpath(OUT, ROOT)}: {len(engines)} engines, {n_actions} actions, {n_fields} top-level fields")
    return 0


if __name__ == "__main__":
    sys.exit(main())
