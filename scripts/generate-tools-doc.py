import json

def generate():
    with open("shared/data/tools.json") as f:
        raw_tools = json.load(f)

    with open("shared/data/engine-tools.json") as f:
        raw_engine_tools = json.load(f)["tools"]

    with open("shared/data/engines.json") as f:
        engines_data = json.load(f)["engines"]
        builtin_engines = {e["id"]: e for e in engines_data}

    by_slug = {}
    for m in raw_engine_tools:
        if m.get("registry_slug"):
            by_slug[m["registry_slug"]] = m
        for alias in m.get("registry_slugs") or []:
            by_slug[alias] = m

    dedicated = {
        "text-case-converter": "TextCaseConverter",
        "word-counter": "WordCounter",
        "json-formatter": "JsonFormatter",
        "base64-encoder-decoder": "Base64Tool",
        "url-encoder-decoder": "UrlEncoderTool",
        "image-cropper": "ImageCropperTool"
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

    all_tools = []
    for t in raw_tools:
        slug = t["slug"]
        is_ded = slug in dedicated
        mapping = by_slug.get(slug)
        eng_id = mapping.get("engine") if mapping and mapping.get("status") in ("implemented", "partial") else None
        has_eng = bool(eng_id and eng_id in builtin_engines)
        status = "Available" if (is_ded or has_eng) else "Coming Soon"
        engine_name = eng_id or (mapping.get("engine") if mapping else None) or "Pending"
        action = mapping.get("action") if mapping else "-"
        all_tools.append({
            "id": t["id"],
            "name": t["name"],
            "slug": slug,
            "category": t["category"],
            "status": status,
            "engine": engine_name,
            "action": action or "-",
            "description": t.get("description", "").strip()
        })

    next_id = 100000
    for m in raw_engine_tools:
        if not m.get("registry_slug") and not m.get("registry_slugs"):
            if m.get("engine") and m.get("status") in ("implemented", "partial") and m["engine"] in builtin_engines:
                tc_name = title_case(m["id"])
                eng_name = m["engine"]
                all_tools.append({
                    "id": next_id,
                    "name": tc_name,
                    "slug": m["id"],
                    "category": m.get("ui_category", "Developer Tools"),
                    "status": "Available",
                    "engine": eng_name,
                    "action": m.get("action") or "-",
                    "description": f"{tc_name} utility powered by {eng_name}."
                })
                next_id += 1

    total_count = len(all_tools)
    available_count = sum(1 for t in all_tools if t["status"] == "Available")
    coming_soon_count = total_count - available_count

    categories = {}
    for t in all_tools:
        categories.setdefault(t["category"], []).append(t)

    engine_counts = {}
    for t in all_tools:
        eng = t["engine"]
        engine_counts[eng] = engine_counts.get(eng, 0) + 1

    lines = []
    lines.append("# Dev Toolbox — Comprehensive Tools & Status Registry\n")
    lines.append("A complete catalog of all utilities, categories, WASM engines, and operational statuses across the Dev Toolbox platform.\n")
    lines.append("## Platform Overview\n")
    lines.append(f"- **Total Registered Utilities**: `{total_count}`")
    lines.append(f"- **Live & Interactive Tools (WASM / Client Execution)**: `{available_count}` ({available_count/total_count*100:.1f}%)")
    lines.append(f"- **Queued for Engine Implementation (Coming Soon)**: `{coming_soon_count}` ({coming_soon_count/total_count*100:.1f}%)")
    lines.append(f"- **Active WASM Processing Engines**: `{len(builtin_engines)}`")
    lines.append(f"- **Total Tool Categories**: `{len(categories)}`\n")

    lines.append("### Architectural Summary\n")
    lines.append("| Metric | Count | Operational Behavior |")
    lines.append("| :--- | :---: | :--- |")
    lines.append(f"| **Available (Live)** | **{available_count}** | Executes fully client-side in browser Web Workers with zero file uploads |")
    lines.append(f"| **Coming Soon** | **{coming_soon_count}** | Pre-registered in catalog with metadata and queued for WASM engine bindings |")
    lines.append(f"| **Total Tools** | **{total_count}** | Total unified platform inventory |")
    lines.append("\n---\n")

    lines.append("## Category Summary\n")
    lines.append("| Category | Total Tools | Available (Live) | Coming Soon | Primary Engine(s) |")
    lines.append("| :--- | :---: | :---: | :---: | :--- |")
    for cat, ts in sorted(categories.items()):
        avail = sum(1 for x in ts if x["status"] == "Available")
        cs = len(ts) - avail
        engs = sorted(list(set(x["engine"] for x in ts if x["engine"] != "Pending")))
        eng_str = ", ".join(f"`{e}`" for e in engs[:3]) if engs else "*Queued*"
        if len(engs) > 3:
            eng_str += f" +{len(engs)-3} more"
        lines.append(f"| **{cat}** | {len(ts)} | {avail} | {cs} | {eng_str} |")

    lines.append("\n---\n")

    lines.append("## Registered WASM Engines\n")
    lines.append("| Engine ID | Version | Tools Served | Primary Capabilities |")
    lines.append("| :--- | :---: | :---: | :--- |")
    for eng in sorted(engines_data, key=lambda x: x["id"]):
        eid = eng["id"]
        cnt = engine_counts.get(eid, 0)
        ver = eng.get("version", "1.0.0")
        caps_list = eng.get("capabilities", [])
        caps = ", ".join(caps_list[:4])
        if len(caps_list) > 4:
            caps += ", …"
        lines.append(f"| `{eid}` | `v{ver}` | {cnt} | {caps} |")

    lines.append("\n---\n")

    lines.append("## Detailed Tool Catalog by Category\n")

    for cat, ts in sorted(categories.items()):
        avail = sum(1 for x in ts if x["status"] == "Available")
        lines.append(f"### {cat} ({len(ts)} tools — {avail} Available)\n")
        lines.append("| ID | Tool Name | Slug | Status | Engine | Action | Description |")
        lines.append("| :---: | :--- | :--- | :---: | :--- | :--- | :--- |")
        for t in sorted(ts, key=lambda x: (0 if x["status"] == "Available" else 1, x["name"])):
            st_badge = "✅ Available" if t["status"] == "Available" else "⏳ Coming Soon"
            eng_str = f"`{t['engine']}`" if t["engine"] != "Pending" else "—"
            act_str = f"`{t['action']}`" if t["action"] != "-" else "—"
            desc = t["description"].replace("|", "\\|")
            lines.append(f"| {t['id']} | **{t['name']}** | `{t['slug']}` | {st_badge} | {eng_str} | {act_str} | {desc} |")
        lines.append("")

    with open("TOOLS.md", "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

    print(f"Generated TOOLS.md with {len(lines)} lines covering {total_count} tools.")

if __name__ == "__main__":
    generate()

