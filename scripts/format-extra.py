#!/usr/bin/env python3
"""Formatting passes that stable `rustfmt` cannot do, for the engine workspace.

rustfmt (stable) leaves three things alone that otherwise produce 200–1000
column lines in the engines:

1. the body of `json!({ ... })` / `json!([ ... ])` macros – not Rust syntax,
2. long string literals – `format_strings` is nightly-only,
3. long `///` / `//!` doc comments – `wrap_comments` is nightly-only.

This tool fixes all three **without changing any token value**:

* `json!` bodies are re-flowed one entry per line (recursively) – whitespace
  only;
* over-long string literals are split with the `\\<newline>` continuation that
  the Rust lexer collapses (the literal *value* is identical; byte strings and
  raw strings are never touched);
* over-long doc comment lines are word-wrapped, skipping code fences, tables
  and lines that contain long unbreakable spans.

Every rewritten file is verified: the non-whitespace character stream outside
literals and the *decoded* values of all string literals must be unchanged,
otherwise the tool refuses to write.

Usage:
    python3 scripts/format-extra.py [--check] [--width 100] [paths...]

Run `cargo fmt --all` afterwards (the tool is idempotent and is run twice by
`pnpm fmt:engines`, once before and once after rustfmt, so that continuation
lines are aligned to the final column of the opening quote).
"""

from __future__ import annotations

import argparse
import bisect
import glob
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MACRO_RE = re.compile(r"(?<![A-Za-z0-9_])json!\s*\(")
CONTINUATION_RE = re.compile(r"\\\n[ \t]*")
MIN_SPLIT_LITERAL = 24  # do not bother splitting literals shorter than this
RAW_RE = re.compile(r'b?r(#*)"')
CHAR_RE = re.compile(r"(\\u\{[0-9A-Fa-f]+\}|\\x[0-9A-Fa-f]{2}|\\.|[^\\'])'")


# --------------------------------------------------------------------------- lexing


class Scanner:
    """Minimal Rust token walker that understands strings, chars and comments."""

    def __init__(self, text: str):
        self.t = text

    def skip_literal(self, i: int) -> int:
        """If a literal starts at `i`, returns the index after it, else `i`."""
        t = self.t
        c = t[i]
        m = RAW_RE.match(t, i) if c in "br" else None
        if m:
            hashes = m.group(1)
            end = t.find('"' + hashes, m.end())
            if end < 0:
                raise ValueError("unterminated raw string")
            return end + 1 + len(hashes)
        if c == '"' or (c == "b" and t.startswith('b"', i)):
            j = i + (2 if c == "b" else 1)
            while j < len(t):
                if t[j] == "\\":
                    j += 2
                    continue
                if t[j] == '"':
                    return j + 1
                j += 1
            raise ValueError("unterminated string")
        if c == "'" or (c == "b" and t.startswith("b'", i)):
            j = i + (2 if c == "b" else 1)
            m = CHAR_RE.match(t, j)
            if m:
                return m.end()
            return i  # lifetime or loop label
        return i

    def skip_comment(self, i: int) -> int:
        t = self.t
        if t.startswith("//", i):
            end = t.find("\n", i)
            return len(t) if end < 0 else end
        if t.startswith("/*", i):
            depth = 0
            j = i
            while j < len(t):
                if t.startswith("/*", j):
                    depth += 1
                    j += 2
                elif t.startswith("*/", j):
                    depth -= 1
                    j += 2
                    if depth == 0:
                        return j
                else:
                    j += 1
            raise ValueError("unterminated block comment")
        return i

    def matching(self, i: int) -> int:
        """Index of the bracket matching the opening bracket at `i`."""
        t = self.t
        pairs = {"(": ")", "[": "]", "{": "}"}
        stack = [pairs[t[i]]]
        j = i + 1
        while j < len(t):
            k = self.skip_literal(j)
            if k != j:
                j = k
                continue
            k = self.skip_comment(j)
            if k != j:
                j = k
                continue
            c = t[j]
            if c in pairs:
                stack.append(pairs[c])
            elif c in ")]}":
                if c != stack[-1]:
                    raise ValueError("mismatched brackets")
                stack.pop()
                if not stack:
                    return j
            j += 1
        raise ValueError("unbalanced brackets")

    def has_comment(self, start: int, end: int) -> bool:
        j = start
        while j < end:
            k = self.skip_literal(j)
            if k != j:
                j = k
                continue
            if self.t.startswith("//", j) or self.t.startswith("/*", j):
                return True
            j += 1
        return False

    def tokens(self):
        """Yields `(kind, start, end)` with kind in {"lit", "comment", "code"}."""
        t = self.t
        j = 0
        code_start = 0
        while j < len(t):
            k = self.skip_literal(j)
            if k != j:
                if code_start < j:
                    yield ("code", code_start, j)
                yield ("lit", j, k)
                j = k
                code_start = j
                continue
            k = self.skip_comment(j)
            if k != j:
                if code_start < j:
                    yield ("code", code_start, j)
                yield ("comment", j, k)
                j = k
                code_start = j
                continue
            j += 1
        if code_start < len(t):
            yield ("code", code_start, len(t))


# ------------------------------------------------------------------- json! reflow


def split_top_level(body: str, sep: str) -> list[str]:
    sc = Scanner(body)
    parts = []
    depth = 0
    last = 0
    j = 0
    while j < len(body):
        k = sc.skip_literal(j)
        if k != j:
            j = k
            continue
        c = body[j]
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif c == sep and depth == 0:
            parts.append(body[last:j])
            last = j + 1
        j += 1
    parts.append(body[last:])
    return parts


def collapse_ws(s: str) -> str:
    """Collapses whitespace runs outside string literals to a single space."""
    sc = Scanner(s)
    out = []
    j = 0
    while j < len(s):
        k = sc.skip_literal(j)
        if k != j:
            out.append(s[j:k])
            j = k
            continue
        if s[j].isspace():
            while j < len(s) and s[j].isspace():
                j += 1
            out.append(" ")
            continue
        out.append(s[j])
        j += 1
    return "".join(out).strip()


def split_key_value(entry: str):
    sc = Scanner(entry)
    depth = 0
    j = 0
    while j < len(entry):
        k = sc.skip_literal(j)
        if k != j:
            j = k
            continue
        c = entry[j]
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif c == ":" and depth == 0:
            if entry.startswith("::", j):
                j += 2
                continue
            return entry[:j], entry[j + 1 :]
        j += 1
    return None


def is_container(value: str):
    v = value.strip()
    if v and v[0] in "{[":
        try:
            end = Scanner(v).matching(0)
        except ValueError:
            return None
        if end == len(v) - 1:
            return v[0]
    return None


def visible_len(s: str) -> int:
    """Length of the longest physical line of `s`."""
    return max(len(line) for line in s.split("\n"))


def format_value(value: str, indent: int, width: int, col: int) -> str:
    flat = collapse_ws(value)
    kind = is_container(flat)
    if kind is None or ("\n" not in flat and col + len(flat) <= width):
        return flat
    inner = flat[1:-1].strip()
    if not inner:
        return flat
    items = [p.strip() for p in split_top_level(inner, ",")]
    trailing = False
    if items and items[-1] == "":
        items.pop()
        trailing = True
    pad = " " * (indent + 4)
    lines = [kind]
    for idx, item in enumerate(items):
        comma = "," if (idx < len(items) - 1 or trailing) else ""
        if kind == "{":
            kv = split_key_value(item)
            if kv is None:
                lines.append(pad + collapse_ws(item) + comma)
                continue
            key = collapse_ws(kv[0])
            val = format_value(kv[1], indent + 4, width, len(pad) + len(key) + 2)
            lines.append(f"{pad}{key}: {val}{comma}")
        else:
            lines.append(pad + format_value(item, indent + 4, width, len(pad)) + comma)
    lines.append(" " * indent + ("}" if kind == "{" else "]"))
    return "\n".join(lines)


def reflow_json_macros(text: str, width: int) -> str:
    out = []
    pos = 0
    sc = Scanner(text)
    j = 0
    while j < len(text):
        k = sc.skip_literal(j)
        if k != j:
            j = k
            continue
        k = sc.skip_comment(j)
        if k != j:
            j = k
            continue
        m = MACRO_RE.match(text, j)
        if not m:
            j += 1
            continue
        open_paren = m.end() - 1
        try:
            close_paren = sc.matching(open_paren)
        except ValueError:
            j = m.end()
            continue
        line_start = text.rfind("\n", 0, m.start()) + 1
        line_end = text.find("\n", close_paren)
        line_end = len(text) if line_end < 0 else line_end
        longest = visible_len(text[line_start:line_end])
        body = text[open_paren + 1 : close_paren]
        # Only object/array bodies are reflowed; `json!(expr)` is valid Rust that rustfmt handles.
        if longest <= width or is_container(body) is None or sc.has_comment(open_paren, close_paren):
            j = close_paren + 1
            continue
        indent = len(text[line_start:]) - len(text[line_start:].lstrip(" "))
        col = m.end() - line_start
        formatted = format_value(body, indent, width, col)
        out.append(text[pos : m.end()])
        out.append(formatted)
        pos = close_paren
        j = close_paren
    out.append(text[pos:])
    return "".join(out)


# ------------------------------------------------------------- string continuation


def is_plain_string(text: str, start: int) -> bool:
    """True for `"..."` (not raw, not byte) literals."""
    return text[start] == '"'


def join_continuations(text: str) -> str:
    """Removes `\\<newline><ws>` continuations from plain string literals (value-preserving)."""
    out = []
    for kind, s, e in Scanner(text).tokens():
        chunk = text[s:e]
        if kind == "lit" and is_plain_string(text, s) and "\\\n" in chunk:
            chunk = CONTINUATION_RE.sub("", chunk)
        out.append(chunk)
    return "".join(out)


def split_literal(body: str, first_room: int, rest_room: int) -> list[str] | None:
    """Splits the inside of a string literal at single spaces so each chunk fits."""
    if "\n" in body or "\\" == body[-1:]:
        return None
    chunks = []
    rest = body
    room = first_room
    while len(rest) + 1 > room:  # +1 for the trailing backslash
        # candidate split: last single space with a non-space follower within the room
        cut = -1
        i = rest.rfind(" ", 0, max(room - 1, 1))
        while i > 0:
            if rest[i - 1] != " " and rest[i - 1] != "\\" and i + 1 < len(rest) and rest[i + 1] != " ":
                cut = i + 1
                break
            i = rest.rfind(" ", 0, i)
        if cut <= 0 or cut >= len(rest):
            return None if not chunks else chunks + [rest]
        chunks.append(rest[:cut])
        rest = rest[cut:]
        room = rest_room
    chunks.append(rest)
    return chunks if len(chunks) > 1 else None


def wrap_long_strings(text: str, width: int) -> str:
    text = join_continuations(text)
    lines = text.split("\n")
    # Map absolute offsets to (line, column).
    starts = []
    off = 0
    for line in lines:
        starts.append(off)
        off += len(line) + 1
    edits = []  # (start, end, replacement)
    for kind, s, e in Scanner(text).tokens():
        if kind != "lit" or not is_plain_string(text, s):
            continue
        line_no = bisect.bisect_right(starts, s) - 1
        line = lines[line_no]
        if len(line) <= width or "\n" in text[s:e]:
            continue
        stripped = line.lstrip()
        if stripped.startswith("#[") or stripped.startswith("//"):
            continue
        body = text[s + 1 : e - 1]
        if len(body) < MIN_SPLIT_LITERAL:
            continue
        col = s - starts[line_no]  # column of the opening quote
        line_indent = len(line) - len(line.lstrip(" "))
        # Room on each physical line: width minus the prefix minus the trailing backslash.
        first_room = width - (col + 1) - 1
        rest_room = first_room
        pad = " " * (col + 1)
        if rest_room < 24:
            # The quote sits too far right: start the text on the next line at a
            # modest indent (`"\` + newline; the lexer drops the leading whitespace).
            pad = " " * (line_indent + 4)
            rest_room = width - len(pad) - 1
            chunks = split_literal(body, rest_room, rest_room)
            if not chunks:
                if len(body) + 2 > rest_room:
                    continue
                chunks = [body]
            chunks = [""] + chunks
        else:
            chunks = split_literal(body, first_room, rest_room)
            if not chunks:
                continue
        new = '"' + ("\\\n" + pad).join(chunks) + '"'
        edits.append((s, e, new))
    if not edits:
        return text
    out = []
    pos = 0
    for s, e, new in edits:
        out.append(text[pos:s])
        out.append(new)
        pos = e
    out.append(text[pos:])
    return "".join(out)


# ------------------------------------------------------------------ doc comments


DOC_RE = re.compile(r"^(\s*)(///|//!)( ?)(.*)$")
LIST_RE = re.compile(r"^(\s*)([-*+]|\d+\.)\s+")


def wrap_doc_comments(text: str, width: int) -> str:
    lines = text.split("\n")
    out = []
    in_fence = False
    for line in lines:
        m = DOC_RE.match(line)
        if not m:
            out.append(line)
            continue
        indent, marker, _, body = m.groups()
        if body.strip().startswith("```"):
            in_fence = not in_fence
            out.append(line)
            continue
        if in_fence or len(line) <= width or "|" in body or "http" in body:
            out.append(line)
            continue
        prefix = f"{indent}{marker} "
        lm = LIST_RE.match(body)
        cont_indent = " " * (len(lm.group(0)) if lm else (len(body) - len(body.lstrip())))
        words = body.split(" ")
        cur = prefix
        first = True
        wrapped = []
        for w in words:
            if w == "":
                continue
            candidate = (cur + (" " if not cur.endswith(" ") else "") + w) if cur.strip() else cur + w
            if len(candidate) > width and cur.strip() and cur != prefix:
                wrapped.append(cur.rstrip())
                cur = prefix + cont_indent + w
                first = False
            else:
                cur = candidate
        wrapped.append(cur.rstrip())
        if any(len(x) > width for x in wrapped) and len(wrapped) == 1:
            out.append(line)
        else:
            out.extend(wrapped)
    return "\n".join(out)


# ---------------------------------------------------------------------- driver


def decoded_strings_and_stream(text: str):
    """Returns (decoded string values in order, non-whitespace stream outside literals)."""
    lits = []
    stream = []
    for kind, s, e in Scanner(text).tokens():
        chunk = text[s:e]
        if kind == "lit":
            if is_plain_string(text, s):
                chunk = CONTINUATION_RE.sub("", chunk)
            lits.append(chunk)
        elif kind == "code":
            stream.append("".join(ch for ch in chunk if not ch.isspace()))
        else:  # comment: keep its text without whitespace (doc wrapping only moves whitespace)
            stream.append("".join(ch for ch in chunk if not ch.isspace()).rstrip("/"))
    return lits, "".join(stream)


def doc_text_only(text: str) -> str:
    """All comment text without whitespace and comment markers (used to verify doc wrapping)."""
    out = []
    for line in text.split("\n"):
        m = DOC_RE.match(line)
        if m:
            out.append("".join(ch for ch in m.group(4) if not ch.isspace()))
    return "".join(out)


def format_file(text: str, width: int) -> str:
    text = reflow_json_macros(text, width)
    text = wrap_long_strings(text, width)
    text = wrap_doc_comments(text, width)
    return text


def verify(original: str, formatted: str) -> str | None:
    o_lits, o_stream = decoded_strings_and_stream(original)
    f_lits, f_stream = decoded_strings_and_stream(formatted)
    if o_lits != f_lits:
        return "string literal values changed"
    # Comments may have been re-wrapped: compare code-only streams and doc text separately.
    o_code = "".join(
        "".join(ch for ch in original[s:e] if not ch.isspace())
        for k, s, e in Scanner(original).tokens()
        if k == "code"
    )
    f_code = "".join(
        "".join(ch for ch in formatted[s:e] if not ch.isspace())
        for k, s, e in Scanner(formatted).tokens()
        if k == "code"
    )
    if o_code != f_code:
        return "code token stream changed"
    if doc_text_only(original) != doc_text_only(formatted):
        return "doc comment text changed"
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="do not write; exit 1 if changes are needed")
    ap.add_argument("--width", type=int, default=100)
    ap.add_argument("paths", nargs="*")
    args = ap.parse_args()
    paths = args.paths or sorted(
        glob.glob(os.path.join(ROOT, "engines", "*", "src", "**", "*.rs"), recursive=True)
        + glob.glob(os.path.join(ROOT, "engines", "*", "tests", "*.rs"))
    )
    changed = 0
    for path in paths:
        with open(path, encoding="utf-8") as fh:
            original = fh.read()
        try:
            formatted = format_file(original, args.width)
        except ValueError as e:
            print(f"{path}: skipped ({e})", file=sys.stderr)
            continue
        if formatted == original:
            continue
        problem = verify(original, formatted)
        if problem:
            print(f"{path}: INTERNAL ERROR - {problem}, refusing to write", file=sys.stderr)
            return 2
        changed += 1
        rel = os.path.relpath(path, ROOT)
        if args.check:
            print(f"would reformat {rel}")
        else:
            with open(path, "w", encoding="utf-8") as fh:
                fh.write(formatted)
            print(f"reformatted {rel}")
    if args.check and changed:
        print(f"{changed} file(s) need `python3 scripts/format-extra.py`")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
