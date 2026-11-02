#!/usr/bin/env python3
"""
Tool Testing & Fix Tracker CLI for Dev Toolbox.
Maintains tracker.json and syncs TRACKER.md.

Usage:
  python3 scripts/track.py status
  python3 scripts/track.py mark <slug_or_id> <pass|fixed|issue|todo> [notes]
  python3 scripts/track.py issues
  python3 scripts/track.py next [category]
  python3 scripts/track.py sync
"""

import sys
import os
import json
from datetime import datetime

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TRACKER_JSON = os.path.join(ROOT, "tracker.json")
TRACKER_MD = os.path.join(ROOT, "TRACKER.md")
TOOLS_JSON = os.path.join(ROOT, "shared/data/tools.json")
ENGINE_TOOLS_JSON = os.path.join(ROOT, "shared/data/engine-tools.json")
ENGINES_JSON = os.path.join(ROOT, "shared/data/engines.json")

def load_data():
    with open(TOOLS_JSON) as f:
        raw_tools = json.load(f)
    with open(ENGINE_TOOLS_JSON) as f:
        raw_engine_tools = json.load(f)["tools"]
    with open(ENGINES_JSON) as f:
        builtin_engines = {e["id"]: e for e in json.load(f)["engines"]}

    by_slug = {}
    for m in raw_engine_tools:
        if m.get("registry_slug"):
            by_slug[m["registry_slug"]] = m
        for alias in m.get("registry_slugs") or []:
            by_slug[alias] = m

    dedicated = {
        "text-case-converter", "word-counter", "json-formatter",
        "base64-encoder-decoder", "url-encoder-decoder", "image-cropper"
    }

    def title_case(id_str):
        words = id_str.split("-")
        acronyms = {
            "pdf","csv","svg","png","jpg","gif","ocr","qr","sql","css","jwt",
            "url","utm","kml","wkt","tsv","xml","rtf","otf","ttf","stl","obj",
            "glb","ods","ini","env","rsa","aes","hmac","md5","sha","gst","tds",
            "ppf","sip","emi","roi","npv","irr","cagr","fd","rd","bpm","dpi",
            "ssa","ass","srt","vtt","epub","mobi","docx","xlsx","pptx"
        }
        out = []
        for w in words:
            if len(w) <= 3 and w.lower() in acronyms:
                out.append(w.upper())
            else:
                out.append(w.capitalize())
        return " ".join(out)

    tools = []
    for t in raw_tools:
        slug = t["slug"]
        is_ded = slug in dedicated
        mapping = by_slug.get(slug)
        eng_id = mapping.get("engine") if mapping and mapping.get("status") in ("implemented", "partial") else None
        has_eng = bool(eng_id and eng_id in builtin_engines)
        avail = is_ded or has_eng
        tools.append({
            "id": t["id"],
            "name": t["name"],
            "slug": slug,
            "category": t["category"],
            "available": avail,
            "engine": eng_id or (mapping.get("engine") if mapping else None) or "None",
            "action": mapping.get("action") if mapping else "-"
        })

    next_id = 100000
    for m in raw_engine_tools:
        if not m.get("registry_slug") and not m.get("registry_slugs"):
            if m.get("engine") and m.get("status") in ("implemented", "partial") and m["engine"] in builtin_engines:
                tools.append({
                    "id": next_id,
                    "name": title_case(m["id"]),
                    "slug": m["id"],
                    "category": m.get("ui_category", "Developer Tools"),
                    "available": True,
                    "engine": m["engine"],
                    "action": m.get("action") or "-"
                })
                next_id += 1

    return tools

def get_or_init_tracker():
    tools = load_data()
    tracker = None
    if os.path.exists(TRACKER_JSON):
        with open(TRACKER_JSON) as f:
            tracker = json.load(f)
    else:
        tracker = {
            "meta": {
                "created_at": datetime.now().isoformat(),
                "updated_at": datetime.now().isoformat()
            },
            "history": [],
            "tools": {}
        }

    # If TRACKER.md is newer than tracker.json, sync manual edits from TRACKER.md
    if os.path.exists(TRACKER_MD) and os.path.exists(TRACKER_JSON):
        try:
            if os.path.getmtime(TRACKER_MD) > os.path.getmtime(TRACKER_JSON):
                with open(TRACKER_MD, "r", encoding="utf-8") as f:
                    md_content = f.read()
                for line in md_content.splitlines():
                    if line.startswith("| [") and "|" in line[4:]:
                        parts = [p.strip() for p in line.split("|")[1:-1]]
                        if len(parts) >= 6:
                            chk, tid, name, slug_part, eng, st_text = parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]
                            notes = parts[6] if len(parts) > 6 else ""
                            slug = slug_part.strip("` ")
                            if slug in tracker.get("tools", {}):
                                current_tool = tracker["tools"][slug]
                                if chk == "[x]" and current_tool.get("test_status") in ("untested", "queued"):
                                    current_tool["test_status"] = "passed"
                                    if notes and notes != "—":
                                        current_tool["notes"] = notes
                                elif chk == "[!]" and current_tool.get("test_status") != "issue":
                                    current_tool["test_status"] = "issue"
                                    if notes and notes != "—":
                                        current_tool["notes"] = notes
        except Exception:
            pass

    # Ensure all current tools exist in tracker
    existing = tracker.get("tools", {})
    for t in tools:
        slug = t["slug"]
        if slug not in existing:
            existing[slug] = {
                "id": t["id"],
                "name": t["name"],
                "slug": slug,
                "category": t["category"],
                "available": t["available"],
                "engine": t["engine"],
                "action": t["action"],
                "test_status": "untested" if t["available"] else "queued",
                "tested_at": None,
                "notes": ""
            }
        else:
            # Sync metadata
            existing[slug]["id"] = t["id"]
            existing[slug]["name"] = t["name"]
            existing[slug]["slug"] = slug
            existing[slug]["category"] = t["category"]
            existing[slug]["available"] = t["available"]
            existing[slug]["engine"] = t["engine"]
            existing[slug]["action"] = t["action"]

    tracker["tools"] = existing
    return tracker

def save_tracker(tracker):
    tracker["meta"]["updated_at"] = datetime.now().isoformat()
    with open(TRACKER_JSON, "w") as f:
        json.dump(tracker, f, indent=2)
    sync_markdown(tracker)

def sync_markdown(tracker):
    tools = list(tracker["tools"].values())
    total = len(tools)
    available = sum(1 for t in tools if t["available"])
    queued = total - available

    passed = sum(1 for t in tools if t["test_status"] == "passed")
    fixed = sum(1 for t in tools if t["test_status"] == "fixed")
    issues = sum(1 for t in tools if t["test_status"] == "issue")
    untested = sum(1 for t in tools if t["test_status"] == "untested")

    pct_verified = ((passed + fixed) / available * 100) if available > 0 else 0

    lines = [
        "# Dev Toolbox — Tool Testing & Fix Tracker\n",
        f"> **Progress**: **{passed + fixed}** of **{available}** live utilities tested & verified (**{pct_verified:.1f}%**)\n",
        "## Overall Progress Dashboard\n",
        "| Status | Count | Percentage (of Live) | Meaning |",
        "| :--- | :---: | :---: | :--- |",
        f"| ✅ **Tested & Passed** | `{passed}` | {passed / available * 100:.1f}% | Tested thoroughly in browser; works as expected |",
        f"| 🔧 **Fixed & Verified** | `{fixed}` | {fixed / available * 100:.1f}% | Had an issue, fixed and re-verified |",
        f"| ⚠️ **Issue Found / Needs Fix** | `{issues}` | {issues / available * 100:.1f}% | Bug or unexpected behavior flagged for investigation |",
        f"| ⏳ **Untested** | `{untested}` | {untested / available * 100:.1f}% | Live tool waiting to be tested |",
        f"| 💤 **Queued (Coming Soon)** | `{queued}` | — | Defined in catalog, queued for WASM engine integration |",
        f"| **Total Tools** | **`{total}`** | **100%** | Full platform inventory |",
        "\n---\n",
        "## CLI Quick Reference\n",
        "You can update this tracker directly using the `scripts/track.py` script:\n",
        "```bash",
        "# Show current testing progress",
        "python3 scripts/track.py status\n",
        "# Mark a tool as tested & working",
        "python3 scripts/track.py mark <slug-or-id> pass \"worked with sample.pdf\"\n",
        "# Mark an issue found on a tool",
        "python3 scripts/track.py mark <slug-or-id> issue \"fails with large files\"\n",
        "# Mark an issue as fixed",
        "python3 scripts/track.py mark <slug-or-id> fixed \"adjusted memory buffer limit\"\n",
        "# View all tools with open issues",
        "python3 scripts/track.py issues\n",
        "# Get the next untested tool to work on",
        "python3 scripts/track.py next [category-name]",
        "```\n",
        "---\n"
    ]

    history = tracker.get("history", [])
    if history:
        lines.append("## Recent Testing Activity Log\n")
        lines.append("| Timestamp | Tool | Status | Notes |")
        lines.append("| :--- | :--- | :---: | :--- |")
        for h in reversed(history[-15:]):
            st_badge = {
                "passed": "✅ Passed",
                "fixed": "🔧 Fixed",
                "issue": "⚠️ Issue",
                "untested": "⏳ Untested"
            }.get(h.get("status", ""), h.get("status", ""))
            lines.append(f"| {h.get('timestamp', '')[:16]} | `{h.get('slug', '')}` | {st_badge} | {h.get('notes', '')} |")
        lines.append("\n---\n")

    # Category Breakdown
    categories = {}
    for t in tools:
        categories.setdefault(t["category"], []).append(t)

    lines.append("## Category Testing Progress\n")
    lines.append("| Category | Total | Live | Passed | Fixed | Issues | Untested | Progress |")
    lines.append("| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |")

    for cat, ts in sorted(categories.items()):
        c_tot = len(ts)
        c_live = sum(1 for x in ts if x["available"])
        c_pass = sum(1 for x in ts if x["test_status"] == "passed")
        c_fix = sum(1 for x in ts if x["test_status"] == "fixed")
        c_iss = sum(1 for x in ts if x["test_status"] == "issue")
        c_unt = sum(1 for x in ts if x["test_status"] == "untested")
        c_pct = ((c_pass + c_fix) / c_live * 100) if c_live > 0 else 0
        lines.append(f"| **{cat}** | {c_tot} | {c_live} | {c_pass} | {c_fix} | {c_iss} | {c_unt} | **{c_pct:.0f}%** |")

    lines.append("\n---\n")
    lines.append("## Detailed Tool Checklist\n")

    STATUS_ICONS = {
        "passed": "✅ Passed",
        "fixed": "🔧 Fixed",
        "issue": "⚠️ Needs Fix",
        "untested": "⏳ Untested",
        "queued": "💤 Queued"
    }

    CHECKBOX = {
        "passed": "[x]",
        "fixed": "[x]",
        "issue": "[!]",
        "untested": "[ ]",
        "queued": "[ ]"
    }

    for cat, ts in sorted(categories.items()):
        lines.append(f"### {cat} ({len(ts)} tools)\n")
        lines.append("| Check | ID | Tool Name | Slug | Engine | Status | Notes |")
        lines.append("| :---: | :---: | :--- | :--- | :--- | :---: | :--- |")
        for t in sorted(ts, key=lambda x: (0 if x["test_status"] in ("issue", "fixed", "passed") else 1, x["id"])):
            chk = CHECKBOX.get(t["test_status"], "[ ]")
            st = STATUS_ICONS.get(t["test_status"], t["test_status"])
            eng = f"`{t['engine']}`" if t["engine"] and t["engine"] != "None" else "—"
            notes = t.get("notes", "").replace("|", "\\|") or "—"
            lines.append(f"| {chk} | {t['id']} | **{t['name']}** | `{t['slug']}` | {eng} | {st} | {notes} |")
        lines.append("")

    with open(TRACKER_MD, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

def cmd_status():
    tracker = get_or_init_tracker()
    tools = list(tracker["tools"].values())
    total = len(tools)
    available = sum(1 for t in tools if t["available"])
    queued = total - available

    passed = sum(1 for t in tools if t["test_status"] == "passed")
    fixed = sum(1 for t in tools if t["test_status"] == "fixed")
    issues = sum(1 for t in tools if t["test_status"] == "issue")
    untested = sum(1 for t in tools if t["test_status"] == "untested")

    pct = ((passed + fixed) / available * 100) if available > 0 else 0

    print("==================================================")
    print("      DEV TOOLBOX — TESTING & FIX TRACKER")
    print("==================================================")
    print(f"Total Tools Registered : {total}")
    print(f"Live Tools to Test     : {available}")
    print(f"Queued Tools           : {queued}")
    print("--------------------------------------------------")
    print(f"✅ Tested & Passed     : {passed:4d}  ({passed / available * 100:5.1f}%)")
    print(f"🔧 Fixed & Verified    : {fixed:4d}  ({fixed / available * 100:5.1f}%)")
    print(f"⚠️ Needs Fix (Issues)  : {issues:4d}  ({issues / available * 100:5.1f}%)")
    print(f"⏳ Untested            : {untested:4d}  ({untested / available * 100:5.1f}%)")
    print("--------------------------------------------------")
    print(f"Overall Verified Progress: {pct:.1f}% ({passed + fixed}/{available})")
    print("==================================================")

def cmd_mark(identifier, status_code, notes=""):
    tracker = get_or_init_tracker()
    status_map = {
        "pass": "passed",
        "passed": "passed",
        "ok": "passed",
        "done": "passed",
        "tested": "passed",
        "verified": "passed",
        "fix": "fixed",
        "fixed": "fixed",
        "issue": "issue",
        "bug": "issue",
        "fail": "issue",
        "todo": "untested",
        "untested": "untested"
    }

    st = status_map.get(status_code.lower())
    if not st:
        print(f"Invalid status: {status_code}. Use pass, fixed, issue, or todo.")
        sys.exit(1)

    target_tool = None
    for slug, t in tracker["tools"].items():
        if slug == identifier or str(t["id"]) == identifier:
            target_tool = t
            break

    if not target_tool:
        print(f"Tool not found: '{identifier}'. Pass a slug (e.g. compress-pdf) or tool ID (e.g. 3).")
        sys.exit(1)

    old_st = target_tool["test_status"]
    target_tool["test_status"] = st
    target_tool["tested_at"] = datetime.now().isoformat()
    if notes:
        target_tool["notes"] = notes

    tracker["history"].append({
        "timestamp": datetime.now().isoformat(),
        "slug": target_tool["slug"],
        "id": target_tool["id"],
        "old_status": old_st,
        "status": st,
        "notes": notes
    })

    save_tracker(tracker)
    print(f"Updated {target_tool['name']} ({target_tool['slug']}): {old_st} -> {st}")
    if notes:
        print(f"Note: {notes}")
    print("TRACKER.md synced successfully.")

def cmd_issues():
    tracker = get_or_init_tracker()
    issues = [t for t in tracker["tools"].values() if t["test_status"] == "issue"]
    print("==================================================")
    print(f"  TOOLS FLAGGED WITH ISSUES ({len(issues)})")
    print("==================================================")
    if not issues:
        print("No open issues recorded! All tested tools are passing.")
    else:
        for t in issues:
            print(f"- [#{t['id']}] {t['name']} (`{t['slug']}`)")
            print(f"  Engine : {t['engine']} (action: {t['action']})")
            print(f"  Notes  : {t.get('notes') or 'No notes provided'}")
            print()

def cmd_next(category_filter=None):
    tracker = get_or_init_tracker()
    candidates = [
        t for t in tracker["tools"].values()
        if t["available"] and t["test_status"] == "untested"
    ]
    if category_filter:
        candidates = [t for t in candidates if category_filter.lower() in t["category"].lower()]

    if not candidates:
        print("No untested tools found" + (f" in category '{category_filter}'" if category_filter else "") + "!")
        return

    nxt = candidates[0]
    print(f"Next tool to test:")
    print(f"  Name     : {nxt['name']}")
    print(f"  ID       : {nxt['id']}")
    print(f"  Slug     : {nxt['slug']}")
    print(f"  Category : {nxt['category']}")
    print(f"  Engine   : {nxt['engine']} (action: {nxt['action']})")
    print(f"  URL      : http://localhost:5173/tool/{nxt['id']}/{nxt['slug']}")

def main():
    args = sys.argv[1:]
    if not args or args[0] in ("status", "summary"):
        cmd_status()
    elif args[0] == "sync":
        tracker = get_or_init_tracker()
        save_tracker(tracker)
        print("TRACKER.md synced.")
    elif args[0] in ("mark", "set"):
        if len(args) < 3:
            print("Usage: python3 scripts/track.py mark <slug_or_id> <pass|fixed|issue|todo> [notes]")
            sys.exit(1)
        ident = args[1]
        status = args[2]
        notes = " ".join(args[3:]) if len(args) > 3 else ""
        cmd_mark(ident, status, notes)
    elif args[0] in ("issues", "bugs"):
        cmd_issues()
    elif args[0] == "next":
        cat = args[1] if len(args) > 1 else None
        cmd_next(cat)
    else:
        print(f"Unknown command: {args[0]}")
        print("Commands: status, mark, issues, next, sync")

if __name__ == "__main__":
    main()
