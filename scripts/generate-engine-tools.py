#!/usr/bin/env python3
"""Generates shared/data/engine-tools.json – the single source of truth that
maps every catalog tool (catalog.json) to the engine + action that serves it.

Run after adding/renaming engine actions:

    python3 scripts/generate-engine-tools.py

Status values
  implemented  – engine action exists and is covered by tests
  partial      – engine covers the core of the tool; documented limits apply
  unsupported  – engine action exists but returns E_UNSUPPORTED with guidance
  browser      – catalog runtime is JavaScript (browser APIs); no WASM engine
  backend      – needs a server-side service
  not-started  – WASM engine planned but not built (OCR / AI / speech models)
"""
import json
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CATALOG = json.load(open(os.path.join(ROOT, "catalog.json")))
TOOLS_JSON = json.load(open(os.path.join(ROOT, "shared", "data", "tools.json")))

# catalog engine id -> (registry engine id, cargo crate, wasm file, UI category)
ENGINES = {
    "pdf": ("pdf-engine", "pdf-engine", "pdf_engine.wasm", "PDF & Documents"),
    "image": ("image-engine", "image-engine", "image_engine.wasm", "Image Tools"),
    "video": ("video-engine", "video-engine", "video_engine.wasm", "Video Tools"),
    "audio": ("audio-engine", "audio-engine", "audio_engine.wasm", "Audio Tools"),
    "archive": ("archive-engine", "archive-engine", "archive_engine.wasm", "File Tools"),
    "text": ("text-engine", "text-engine", "text_engine.wasm", "Text Tools"),
    "json": ("json-engine", "json-engine", "json_engine.wasm", "Developer Tools"),
    "xml_yaml_toml": ("json-engine", "json-engine", "json_engine.wasm", "Developer Tools"),
    "csv": ("csv-engine", "csv-engine", "csv_engine.wasm", "File Tools"),
    "spreadsheet": ("spreadsheet-engine", "spreadsheet-engine", "spreadsheet_engine.wasm", "File Tools"),
    "crypto": ("crypto-engine", "crypto-engine", "crypto_engine.wasm", "Security & Network"),
    "encoding": ("encoding-engine", "encoding-engine", "encoding_engine.wasm", "Developer Tools"),
    "developer": ("developer-engine", "developer-engine", "developer_engine.wasm", "Developer Tools"),
    "math": ("math-engine", "math-engine", "math_engine.wasm", "Math Tools"),
    "unit_conversion": ("units-engine", "units-engine", "units_engine.wasm", "Calculators & Converters"),
    "finance": ("finance-engine", "finance-engine", "finance_engine.wasm", "Calculators & Converters"),
    "india_tax_gst": ("indiatax-engine", "indiatax-engine", "indiatax_engine.wasm", "Calculators & Converters"),
    "construction": ("construction-engine", "construction-engine", "construction_engine.wasm", "Calculators & Converters"),
    "solar_energy": ("solar-engine", "solar-engine", "solar_engine.wasm", "Calculators & Converters"),
    "date_time": ("datetime-engine", "datetime-engine", "datetime_engine.wasm", "Everyday Tools"),
    "business": ("business-engine", "business-engine", "business_engine.wasm", "Productivity"),
    "qr_barcode": ("qrbarcode-engine", "qrbarcode-engine", "qrbarcode_engine.wasm", "Everyday Tools"),
    "barcode_labels": ("qrbarcode-engine", "qrbarcode-engine", "qrbarcode_engine.wasm", "Everyday Tools"),
    "gis_geo": ("geo-engine", "geo-engine", "geo_engine.wasm", "Calculators & Converters"),
    "color_design": ("colordesign-engine", "colordesign-engine", "colordesign_engine.wasm", "Design Tools"),
    "data_visualization": ("dataviz-engine", "dataviz-engine", "dataviz_engine.wasm", "Developer Tools"),
    "code_analysis": ("developer-engine", "developer-engine", "developer_engine.wasm", "Developer Tools"),
    "ebook": ("document-engine", "document-engine", "document_engine.wasm", "PDF & Documents"),
    "document": ("document-engine", "document-engine", "document_engine.wasm", "PDF & Documents"),
    "font": ("font-engine", "font-engine", "font_engine.wasm", "Design Tools"),
    "3d": ("mesh3d-engine", "mesh3d-engine", "mesh3d_engine.wasm", "Design Tools"),
}

# Catalog engines with no WASM implementation (yet).
PLANNED = {
    "ocr": "not-started", "ai_local": "not-started", "speech": "not-started",
}

# Tool -> (action, status, note). Default status "implemented".
I = "implemented"
P = "partial"
U = "unsupported"

MAP = {
    "pdf": {
        "pdf-merge": ("merge", I, ""), "pdf-split": ("split", I, ""), "pdf-compress": ("compress", P, "Stream re-compression and object cleanup; no image re-encoding"),
        "pdf-rotate": ("rotate", I, ""), "pdf-crop": ("crop", I, ""), "pdf-reorder-pages": ("reorder", I, ""), "pdf-delete-pages": ("delete_pages", I, ""),
        "pdf-extract-pages": ("extract_pages", I, ""), "pdf-duplicate-pages": ("duplicate_pages", I, ""), "pdf-add-page-numbers": ("add_page_numbers", I, ""),
        "pdf-add-watermark": ("add_watermark", I, ""), "pdf-add-header-footer": ("add_header_footer", I, ""), "pdf-protect": ("protect", I, "AES-256/AES-128/RC4"),
        "pdf-unlock": ("unlock", I, "Requires the password; owner-only restrictions removed without it"), "pdf-permission-inspector": ("permissions", I, ""),
        "pdf-metadata-viewer": ("metadata", I, ""), "pdf-edit-metadata": ("set_metadata", I, ""), "pdf-repair": ("repair", I, ""),
        "pdf-to-images": ("to_images", U, "Rasterising pages needs a renderer; use pdf.js in the browser"), "pdf-to-text": ("extract_text", I, ""),
        "pdf-to-html": ("to_html", I, "Text-flow HTML, not pixel layout"), "pdf-to-svg": ("to_svg", U, "Needs a page renderer"), "pdf-to-docx": ("to_docx", P, "Paragraph-level DOCX"),
        "images-to-pdf": ("from_images", I, ""), "text-to-pdf": ("from_text", I, ""), "html-to-pdf": ("from_html", P, "Simplified HTML subset (headings, paragraphs, lists, tables, bold/italic)"),
        "markdown-to-pdf": ("from_markdown", I, ""), "pdf-compare": ("compare", I, "Text diff per page"), "pdf-overlay": ("overlay", I, ""), "pdf-stamp": ("stamp", I, ""),
        "pdf-forms-fill": ("fill_form", I, ""), "pdf-form-field-inspector": ("form_fields", I, ""), "pdf-bookmark-editor": ("set_bookmarks", I, ""),
        "pdf-outline-extractor": ("outlines", I, ""), "pdf-page-size-checker": ("page_sizes", I, ""), "pdf-grayscale": ("grayscale", P, "Converts page content colour operators and images"),
        "pdf-linearize": ("linearize", P, "Rewrites objects in page order; no hint tables"), "pdf-redact": ("redact", I, "Removes matching text and draws boxes"), "pdf-signature-inspector": ("signatures", I, ""),
    },
    "image": {
        "image-resize": ("resize", I, ""), "image-crop": ("crop", I, ""), "image-compress": ("compress", I, ""), "image-convert": ("convert", I, ""), "image-rotate": ("rotate", I, ""), "image-flip": ("flip", I, ""),
        "image-watermark": ("watermark_text", I, "Also watermark_image"), "image-round-corners": ("round_corners", I, ""), "image-grayscale": ("adjust", I, "op=grayscale"), "image-blur": ("adjust", I, "op=blur"),
        "image-sharpen": ("adjust", I, "op=sharpen"), "image-brightness": ("adjust", I, "op=brightness"), "image-contrast": ("adjust", I, "op=contrast"), "image-saturation": ("adjust", I, "op=saturation"),
        "image-hue": ("adjust", I, "op=hue"), "image-invert": ("adjust", I, "op=invert"), "image-pixelate": ("pixel_art", I, ""), "image-add-border": ("add_border", I, ""), "image-remove-border": ("trim_border", I, ""),
        "image-metadata-viewer": ("info", I, "EXIF via kamadak-exif"), "image-strip-metadata": ("strip_metadata", I, ""), "image-dpi-changer": ("set_dpi", I, ""), "image-color-extractor": ("color_at", I, ""),
        "image-palette-generator": ("palette", I, ""), "image-dominant-color": ("dominant_color", I, ""), "image-average-color": ("dominant_color", I, "mode=average"), "image-dimensions": ("info", I, ""),
        "image-base64": ("to_data_url", I, ""), "base64-to-image": ("from_data_url", I, ""), "svg-optimizer": ("svg_optimize", I, ""), "svg-to-png": ("svg_to_raster", I, "resvg"), "svg-to-jpeg": ("svg_to_raster", I, "format=jpeg"),
        "png-to-webp": ("convert", I, "format=webp (lossless)"), "jpeg-to-webp": ("convert", I, ""), "webp-to-png": ("convert", I, ""), "heic-converter": ("convert", U, "No pure-Rust HEIC decoder; use the browser or backend"),
        "avif-converter": ("convert", P, "AVIF decode only"), "gif-frame-extractor": ("gif_frames", I, ""), "gif-resizer": ("gif_resize", I, ""), "image-sprite-generator": ("sprite", I, ""),
        "image-difference": ("difference", I, ""), "image-overlay": ("watermark_image", I, ""), "image-mosaic": ("pixel_art", I, ""), "image-contact-sheet": ("contact_sheet", I, ""), "favicon-generator": ("favicon", I, ""), "og-image-generator": ("og_image", I, ""),
    },
    "video": {
        "video-trim": ("trim", P, "Lossless MP4 only, keyframe aligned"), "video-cut": ("cut", P, "Lossless MP4 only"), "video-merge": ("concat", P, "Identical codec settings required"), "video-concat": ("concat", P, ""),
        "video-compress": ("transcode", U, "Needs an encoder → backend ffmpeg; calc_bitrate helps pick a target"), "video-convert": ("transcode", U, ""), "video-resize": ("resize_calculator", U, "Parameters only; encoding on backend"),
        "video-crop": ("crop_calculator", U, "Crop rectangle only"), "video-rotate": ("transcode", U, ""), "video-flip": ("transcode", U, ""), "video-mute": ("mute", P, "MP4 only"), "video-extract-audio": ("extract_audio_track", P, "AAC copy; decode via audio-engine extract_from_video"),
        "video-extract-frames": ("frame_plan", P, "Browser grabs frames from <video>; engine plans timestamps"), "video-thumbnail": ("frame_plan", P, ""), "video-gif": ("gif_from_frames", P, "Frames captured in the browser"), "video-to-webp": ("gif_from_frames", U, "Animated WebP encoder unavailable; GIF produced instead"),
        "video-speed": ("speed_calculator", U, ""), "video-reverse": ("transcode", U, ""), "video-volume": ("transcode", U, "Use audio-engine on the extracted track"), "video-watermark": ("transcode", U, ""),
        "video-subtitle-burn": ("subtitle_convert", U, "Subtitle prep only; burning needs an encoder"), "video-subtitle-extract": ("extract_subtitles", P, "MP4 tx3g tracks"), "video-metadata": ("inspect", I, "MP4/MOV/MKV/WebM"),
        "video-resolution-check": ("inspect", I, ""), "video-frame-rate-check": ("inspect", I, ""), "video-aspect-ratio": ("aspect_ratio", I, ""), "video-remove-audio": ("remove_audio", P, "MP4 only"), "video-loop": ("loop", P, "MP4 only"),
        "video-split": ("split", P, "MP4 only"), "video-contact-sheet": ("contact_sheet", P, "Frames captured in the browser"),
    },
    "audio": {
        "audio-trim": ("trim", I, ""), "audio-cut": ("cut", I, ""), "audio-merge": ("mix", I, ""), "audio-concat": ("join", I, ""), "audio-compress": ("convert", P, "Downsample/FLAC; no MP3/AAC encoder"), "audio-convert": ("convert", P, "Decodes MP3/FLAC/OGG/AAC/WAV; encodes WAV/FLAC"),
        "audio-normalize": ("normalize", I, ""), "audio-volume": ("volume", I, ""), "audio-fade-in": ("fade", I, ""), "audio-fade-out": ("fade", I, ""), "audio-reverse": ("reverse", I, ""), "audio-speed": ("speed", I, "WSOLA time-stretch"),
        "audio-pitch": ("pitch", I, ""), "audio-noise-reduction": ("noise_reduction", I, "Spectral gating"), "audio-silence-removal": ("silence_removal", I, ""), "audio-extract-from-video": ("extract_from_video", I, "MP4/MKV/WebM audio tracks"),
        "audio-to-video": ("to_video", U, "Needs a video encoder"), "audio-waveform": ("waveform", I, ""), "audio-spectrum": ("spectrum", I, ""), "audio-metadata": ("inspect", I, ""), "audio-duration": ("inspect", I, ""),
        "audio-sample-rate-converter": ("convert", I, ""), "audio-channel-converter": ("convert", I, ""), "audio-mono-stereo": ("convert", I, ""), "audio-loop": ("loop", I, ""), "audio-split": ("split", I, ""), "audio-join": ("join", I, ""),
        "audio-visualizer": ("spectrum", I, "spectrogram frames"), "audio-equalizer": ("equalizer", I, ""), "audio-bass-boost": ("bass_boost", I, ""), "audio-treble-boost": ("treble_boost", I, ""), "audio-tone-generator": ("tone", I, "Also dtmf"),
    },
    "archive": {
        "zip-create": ("create", I, "AES-256 password optional"), "zip-extract": ("extract", I, ""), "zip-view": ("list", I, ""), "zip-repair": ("zip_repair", P, "Central-directory rebuild"), "zip-password-check": ("test", I, ""),
        "tar-create": ("create", I, "format=tar|tar.gz"), "tar-extract": ("extract", I, ""), "gzip-compress": ("create", I, "format=gz"), "gzip-decompress": ("extract", I, ""), "7z-extract": ("seven_zip_extract", U, "No wasm-compatible 7z crate"),
        "rar-extract": ("rar_extract", U, "Proprietary format"), "file-archive-converter": ("convert", I, ""), "archive-list-files": ("list", I, ""), "archive-test-integrity": ("test", I, ""),
    },
    "text": {
        "word-counter": ("stats", I, ""), "character-counter": ("stats", I, ""), "line-counter": ("stats", I, ""), "sentence-counter": ("stats", I, ""), "reading-time": ("stats", I, ""), "text-case-converter": ("case", I, ""),
        "uppercase-converter": ("case", I, "mode=upper"), "lowercase-converter": ("case", I, "mode=lower"), "title-case-converter": ("case", I, "mode=title"), "slug-generator": ("slug", I, ""), "text-reverser": ("reverse_text", I, ""),
        "text-sort-lines": ("lines", I, "op=sort"), "text-deduplicator": ("lines", I, "op=dedupe"), "remove-empty-lines": ("lines", I, "op=remove_empty"), "remove-extra-spaces": ("cleanup", I, ""), "find-and-replace": ("find_replace", I, "regex supported"),
        "text-diff": ("diff", I, ""), "text-compare": ("diff", I, ""), "text-to-speech-prep": ("tts_prep", I, ""), "lorem-ipsum-generator": ("lorem", I, ""), "random-text-generator": ("random_text", I, ""), "text-formatter": ("wrap", I, ""),
        "text-cleaner": ("cleanup", I, ""), "unicode-inspector": ("unicode_inspect", I, ""), "unicode-normalizer": ("unicode_normalize", I, ""), "ascii-converter": ("unicode_escape", I, ""), "rot13": ("rot", I, ""), "morse-code-converter": ("morse_encode", I, "Also morse_decode"),
        "text-to-binary": ("encode_codes", I, "base=2"), "binary-to-text": ("decode_codes", I, ""), "text-to-hex": ("encode_codes", I, "base=16"), "hex-to-text": ("decode_codes", I, ""), "text-to-decimal": ("encode_codes", I, "base=10"),
        "text-joiner": ("join", I, ""), "text-splitter": ("split", I, ""), "text-truncator": ("truncate", I, ""), "text-extractor": ("extract", I, "emails/urls/numbers/…"), "markdown-to-html": ("markdown_to_html", I, ""), "html-to-markdown": ("html_to_markdown", I, ""), "markdown-preview-parser": ("markdown_parse", I, ""),
    },
    "json": {
        "json-formatter": ("format", I, ""), "json-minifier": ("minify", I, ""), "json-validator": ("validate", I, ""), "json-to-csv": ("to_csv", I, ""), "csv-to-json": ("from_csv", I, ""), "json-to-xml": ("to_xml", I, ""), "xml-to-json": ("from_xml", I, ""),
        "json-to-yaml": ("to_yaml", I, ""), "yaml-to-json": ("from_yaml", I, ""), "json-to-toml": ("to_toml", I, ""), "json-to-typescript": ("to_code", I, "language=typescript"), "json-to-rust-struct": ("to_code", I, "language=rust"), "json-to-go-struct": ("to_code", I, "language=go"),
        "json-diff": ("diff", I, ""), "json-compare": ("diff", I, ""), "json-path-query": ("path_query", I, ""), "json-schema-validator": ("schema_validate", I, ""), "json-schema-generator": ("schema_generate", I, ""), "json-escape": ("escape", I, ""), "json-unescape": ("unescape", I, ""),
        "json-array-sorter": ("sort_array", I, ""), "json-key-sorter": ("sort_keys", I, ""), "json-flattener": ("flatten", I, ""), "json-unflattener": ("unflatten", I, ""), "json-to-table": ("to_table", I, ""),
    },
    "xml_yaml_toml": {
        "xml-formatter": ("xml_format", I, ""), "xml-minifier": ("xml_minify", I, ""), "xml-validator": ("xml_validate", I, ""), "xml-to-json": ("from_xml", I, ""), "json-to-xml": ("to_xml", I, ""), "yaml-formatter": ("yaml_format", I, ""),
        "yaml-validator": ("yaml_validate", I, ""), "yaml-to-json": ("from_yaml", I, ""), "json-to-yaml": ("to_yaml", I, ""), "toml-formatter": ("toml_format", I, ""), "toml-validator": ("toml_validate", I, ""), "toml-to-json": ("from_toml", I, ""),
        "ini-formatter": ("ini_format", I, ""), "ini-to-json": ("from_ini", I, ""), "plist-viewer": ("from_plist", I, "XML plists"),
    },
    "csv": {
        "csv-viewer": ("parse", I, ""), "csv-editor": ("write", I, "cell/row/column edits"), "csv-cleaner": ("clean", I, ""), "csv-validator": ("validate", I, "schema rules"), "csv-to-excel": ("from_csv", I, "served by spreadsheet-engine"), "excel-to-csv": ("to_csv", I, "served by spreadsheet-engine"),
        "csv-to-tsv": ("convert", I, ""), "tsv-to-csv": ("convert", I, ""), "csv-merge": ("merge", I, "append or key join"), "csv-split": ("split", I, ""), "csv-deduplicator": ("dedupe", I, ""), "csv-column-extractor": ("select_columns", I, ""),
        "csv-column-reorder": ("reorder_columns", I, ""), "csv-filter": ("filter", I, ""), "csv-sort": ("sort", I, ""), "csv-to-sql": ("to_sql", I, "postgres/mysql/sqlite/mssql"), "csv-to-jsonl": ("to_jsonl", I, ""), "jsonl-to-csv": ("from_jsonl", I, ""),
        "csv-statistics": ("statistics", I, ""), "csv-transpose": ("transpose", I, ""), "csv-compare": ("diff", I, ""), "csv-diff": ("diff", I, ""), "csv-encoding-converter": ("encoding_convert", I, "UTF-8/16, Windows-1252, …"),
    },
    "spreadsheet": {
        "xlsx-viewer": ("read", I, "XLSX/XLSM/XLSB/XLS/ODS"), "xlsx-to-csv": ("to_csv", I, ""), "csv-to-xlsx": ("from_csv", I, ""), "xlsx-to-pdf": ("to_html", P, "Render to HTML, then pdf-engine from_html"), "xlsx-to-json": ("to_json", I, ""), "json-to-xlsx": ("from_json", I, ""),
        "xlsx-merge": ("merge", I, ""), "xlsx-split-sheets": ("split_sheets", I, ""), "xlsx-remove-duplicates": ("dedupe", I, ""), "xlsx-cleaner": ("clean", I, ""), "xlsx-formula-inspector": ("formulas", I, "lists + recalculates ~100 functions"),
        "xlsx-diff": ("diff", I, "cell level"), "ods-to-xlsx": ("convert", I, ""), "xlsx-to-ods": ("convert", I, ""), "spreadsheet-column-letter": ("column_letter", I, ""), "spreadsheet-formula-evaluator": ("evaluate", I, ""),
    },
    "crypto": {
        "md5-generator": ("hash", I, "algorithm=md5"), "sha1-generator": ("hash", I, ""), "sha256-generator": ("hash", I, ""), "sha512-generator": ("hash", I, ""), "sha3-generator": ("hash", I, ""), "blake3-generator": ("hash", I, ""),
        "hmac-generator": ("hmac", I, ""), "hmac-verifier": ("hmac_verify", I, ""), "bcrypt-hash-generator": ("bcrypt_hash", I, ""), "argon2-hash-generator": ("argon2_hash", I, ""), "password-generator": ("password", I, "needs caller entropy"),
        "password-strength-checker": ("password_strength", I, ""), "uuid-generator": ("uuid", I, "v4 needs entropy; v3/v5 deterministic"), "ulid-generator": ("ulid", I, "timestamp from caller"), "jwt-decoder": ("jwt_decode", I, ""), "jwt-inspector": ("jwt_verify", I, ""),
        "base64-encoder": ("base64_encode", I, "served by encoding-engine"), "base64-decoder": ("base64_decode", I, "served by encoding-engine"), "url-encoder": ("url_encode", I, "served by encoding-engine"), "url-decoder": ("url_decode", I, "served by encoding-engine"),
        "html-encoder": ("html_encode", I, "served by encoding-engine"), "html-decoder": ("html_decode", I, "served by encoding-engine"), "hex-encoder": ("hex_encode", I, "served by encoding-engine"), "hex-decoder": ("hex_decode", I, "served by encoding-engine"),
        "aes-encrypt-decrypt": ("encrypt", I, "AES-256-GCM / ChaCha20-Poly1305 container"), "rsa-key-generator": ("rsa_generate", I, "2048-bit ≈ 5 s in WASM"), "rsa-sign-verify": ("rsa_sign", I, "Also rsa_verify"), "pem-parser": ("pem_parse", I, ""), "certificate-decoder": ("certificate_decode", I, ""),
    },
    "encoding": {
        "url-encode-decode": ("url_encode", I, "Also url_decode"), "base64-encode-decode": ("base64_encode", I, "Also base64_decode"), "html-entity-encode-decode": ("html_encode", I, ""), "unicode-encode-decode": ("unicode_escape", I, "served by text-engine"),
        "punycode-converter": ("punycode_encode", I, ""), "quoted-printable-converter": ("quoted_printable_encode", I, ""), "mime-word-decoder": ("mime_word_decode", I, ""), "protobuf-inspector": ("protobuf_inspect", I, "schema-less wire decode"), "messagepack-converter": ("msgpack_encode", I, ""), "cbor-viewer": ("cbor_decode", I, ""),
    },
    "developer": {
        "regex-tester": ("regex_test", I, "lookaround + backreferences"), "regex-explainer": ("regex_explain", I, ""), "cron-expression-parser": ("cron_parse", I, ""), "cron-expression-generator": ("cron_generate", I, ""), "semver-checker": ("semver_check", I, ""), "color-code-converter": ("color_convert", I, ""),
        "sql-formatter": ("sql_format", I, ""), "sql-minifier": ("sql_minify", I, ""), "sql-validator": ("sql_validate", I, "12 dialects"), "sql-to-entity-diagram": ("sql_erd", I, "Mermaid + DBML"), "graphql-formatter": ("graphql_format", I, ""), "graphql-validator": ("graphql_validate", I, ""), "graphql-schema-viewer": ("graphql_schema", I, ""),
        "html-formatter": ("html_format", I, ""), "html-minifier": ("html_minify", I, ""), "css-formatter": ("css_format", I, ""), "css-minifier": ("css_minify", I, ""), "javascript-formatter": ("js_format", P, "Token-level formatter"), "javascript-minifier": ("js_minify", P, "Whitespace/comment minifier, no renaming"),
        "typescript-formatter": ("js_format", P, ""), "typescript-to-javascript": ("ts_to_js", U, "Needs the TypeScript compiler (browser runtime)"), "markdown-linter": ("lint_markdown", I, ""), "dockerfile-linter": ("lint_dockerfile", I, ""), "yaml-linter": ("lint_yaml", I, ""),
        "gitignore-generator": ("gitignore", I, "50+ templates"), "docker-compose-validator": ("validate_compose", I, ""), "env-file-parser": ("env_parse", I, ""), "diff-viewer": ("diff", I, ""), "patch-parser": ("patch_parse", I, "Also patch_apply"), "chmod-calculator": ("chmod", I, ""),
        "mime-type-lookup": ("mime", I, "extension/MIME/magic bytes"), "http-status-lookup": ("http_status", I, ""), "user-agent-parser": ("user_agent", I, ""), "url-parser": ("url_parse", I, ""), "query-string-parser": ("query_parse", I, "Also query_build"), "html-to-jsx": ("html_to_jsx", I, ""), "json-to-code": ("to_code", I, "served by json-engine"),
    },
    "math": {
        "scientific-calculator": ("evaluate", I, ""), "fraction-calculator": ("fraction", I, ""), "percentage-calculator": ("percentage", I, ""), "ratio-calculator": ("ratio", I, ""), "average-calculator": ("statistics", I, ""), "standard-deviation-calculator": ("statistics", I, ""), "variance-calculator": ("statistics", I, ""),
        "prime-number-checker": ("prime_check", I, ""), "prime-factorization": ("prime_factorization", I, ""), "gcd-calculator": ("gcd_lcm", I, ""), "lcm-calculator": ("gcd_lcm", I, ""), "quadratic-equation-solver": ("quadratic", I, ""), "linear-equation-solver": ("linear_equation", I, "Also linear_system"),
        "matrix-calculator": ("matrix", I, ""), "matrix-determinant": ("matrix", I, "op=determinant"), "matrix-inverse": ("matrix", I, "op=inverse"), "unit-circle-calculator": ("trigonometry", I, ""), "trigonometry-calculator": ("trigonometry", I, "Also triangle"), "logarithm-calculator": ("evaluate", I, ""),
        "exponent-calculator": ("evaluate", I, ""), "permutation-calculator": ("combinatorics", I, ""), "combination-calculator": ("combinatorics", I, ""), "probability-calculator": ("probability", I, ""), "statistics-calculator": ("statistics", I, ""), "random-number-generator": ("random_numbers", I, ""),
        "sequence-generator": ("sequence", I, ""), "base-converter": ("base_convert", I, ""), "binary-calculator": ("binary_calc", I, ""), "roman-numeral-converter": ("roman", I, ""), "number-to-words": ("number_to_words", I, ""), "equation-graph-data": ("plot", I, "Also derivative/integral/limit"),
    },
    "unit_conversion": {k: ("convert", I, "") for k in ["length-converter", "weight-converter", "area-converter", "volume-converter", "speed-converter", "time-converter", "pressure-converter", "energy-converter", "power-converter", "data-size-converter", "fuel-economy-converter", "angle-converter", "force-converter", "density-converter", "frequency-converter", "torque-converter"]}
    | {"temperature-converter": ("temperature", I, ""), "cooking-unit-converter": ("cooking", I, ""), "shoe-size-converter": ("shoe_size", I, ""), "paper-size-converter": ("paper_size", I, "")},
    "finance": {
        "loan-emi-calculator": ("loan", I, ""), "mortgage-calculator": ("mortgage", I, ""), "interest-calculator": ("interest", I, ""), "compound-interest-calculator": ("interest", I, "mode=compound"), "simple-interest-calculator": ("interest", I, "mode=simple"), "sip-calculator": ("investment", I, "mode=sip"),
        "lumpsum-calculator": ("investment", I, "mode=lumpsum"), "mutual-fund-return-calculator": ("investment", I, ""), "fd-calculator": ("fixed_deposit", I, ""), "rd-calculator": ("recurring_deposit", I, ""), "ppf-calculator": ("ppf", I, ""), "inflation-calculator": ("inflation", I, ""),
        "cagr-calculator": ("cagr", I, ""), "roi-calculator": ("roi", I, ""), "irr-calculator": ("irr", I, ""), "npv-calculator": ("npv", I, ""), "discount-calculator": ("discount", I, ""), "markup-calculator": ("markup", I, ""), "profit-margin-calculator": ("margin", I, ""),
        "break-even-calculator": ("break_even", I, ""), "salary-calculator": ("salary", I, ""), "hourly-to-salary-calculator": ("hourly", I, ""), "currency-converter": ("currency", P, "Rates supplied by the caller (no network)"), "tip-calculator": ("tip", I, ""), "tax-bracket-calculator": ("tax_brackets", I, "Brackets supplied by the caller"),
    },
    "india_tax_gst": {
        "gst-calculator": ("gst", I, ""), "gst-inclusive-exclusive-calculator": ("gst", I, ""), "gst-invoice-validator": ("validate_invoice", I, ""), "gstin-format-validator": ("validate_gstin", I, "checksum"), "hsn-sac-lookup": ("hsn_lookup", P, "Chapter-level dataset"), "gst-rate-calculator": ("gst", I, ""),
        "input-tax-credit-calculator": ("itc", I, ""), "gstr-data-validator": ("validate_gstr", I, ""), "invoice-number-validator": ("validate_invoice_number", I, ""), "tds-calculator": ("tds", I, ""), "income-tax-estimator": ("income_tax", I, "FY2024-25 presets, overridable"),
        "salary-in-hand-calculator": ("salary_in_hand", I, ""), "professional-tax-calculator": ("professional_tax", I, ""), "capital-gains-estimator": ("capital_gains", I, ""), "bank-statement-cleaner": ("bank_statement", I, ""), "bank-statement-to-csv": ("bank_statement", I, ""), "bank-statement-to-tally": ("bank_statement", I, "format=tally"),
        "purchase-register-reconciliation": ("reconcile", I, ""), "sales-register-reconciliation": ("reconcile", I, ""),
    },
    "construction": {
        "cement-calculator": ("concrete", I, ""), "concrete-calculator": ("concrete", I, ""), "sand-calculator": ("concrete", I, ""), "aggregate-calculator": ("concrete", I, ""), "brick-calculator": ("masonry", I, ""), "block-calculator": ("masonry", I, ""), "mortar-calculator": ("masonry", I, ""),
        "plaster-calculator": ("plaster", I, ""), "paint-calculator": ("paint", I, ""), "tile-calculator": ("tiles", I, ""), "flooring-calculator": ("tiles", I, ""), "roofing-calculator": ("roofing", I, ""), "steel-weight-calculator": ("steel", I, ""), "rebar-calculator": ("rebar", I, ""),
        "excavation-calculator": ("excavation", I, ""), "wall-area-calculator": ("wall_area", I, ""), "staircase-calculator": ("staircase", I, ""), "paver-calculator": ("tiles", I, "mode=paver"), "water-tank-capacity-calculator": ("tank", I, ""), "construction-cost-estimator": ("cost_estimate", I, ""),
    },
    "solar_energy": {
        "solar-panel-size-calculator": ("panel_size", I, ""), "solar-roi-calculator": ("roi", I, ""), "solar-savings-calculator": ("roi", I, ""), "solar-roof-area-calculator": ("panel_size", I, ""), "solar-battery-sizing": ("battery", I, ""), "inverter-sizing-calculator": ("inverter", I, ""),
        "backup-time-calculator": ("battery", I, "mode=backup_time"), "electricity-bill-calculator": ("bill", I, ""), "appliance-energy-calculator": ("appliances", I, ""), "electricity-unit-cost-calculator": ("bill", I, ""), "carbon-footprint-estimator": ("carbon", I, ""),
    },
    "date_time": {
        "date-difference-calculator": ("difference", I, ""), "date-add-subtract": ("add", I, ""), "age-calculator": ("age", I, ""), "time-zone-converter": ("convert_zone", I, "IANA zones"), "unix-timestamp-converter": ("timestamp", I, ""), "timestamp-to-date": ("timestamp", I, ""), "date-to-timestamp": ("timestamp", I, ""),
        "business-days-calculator": ("business_days", I, ""), "working-days-calculator": ("business_days", I, ""), "week-number-calculator": ("week_number", I, ""), "countdown-calculator": ("countdown", I, "now supplied by caller"), "stopwatch": ("format", P, "UI timer; engine formats durations"),
        "pomodoro-timer": ("pomodoro", I, "schedule generation"), "calendar-generator": ("calendar", I, ""), "meeting-time-planner": ("meeting_planner", I, ""), "cron-next-run": ("cron_next", I, ""),
    },
    "business": {
        "invoice-generator": ("document", I, "kind=invoice, HTML output"), "quotation-generator": ("document", I, "kind=quotation"), "purchase-order-generator": ("document", I, "kind=purchase_order"), "profit-loss-calculator": ("profit_loss", I, ""), "markup-margin-calculator": ("markup_margin", I, ""),
        "inventory-turnover-calculator": ("inventory_turnover", I, ""), "stock-reorder-calculator": ("reorder", I, ""), "discount-pricing-calculator": ("discount_pricing", I, ""), "sales-tax-calculator": ("sales_tax", I, ""), "commission-calculator": ("commission", I, ""), "payroll-estimator": ("payroll", I, ""),
        "cash-flow-forecast": ("cash_flow", I, ""), "break-even-analysis": ("break_even", I, ""), "business-valuation-estimator": ("valuation", I, ""), "unit-economics-calculator": ("unit_economics", I, ""),
    },
    "qr_barcode": {
        "qr-code-generator": ("qr_generate", I, ""), "qr-code-reader": ("qr_decode", I, "rqrr"), "barcode-generator": ("barcode_generate", I, "EAN/UPC/Code128/Code39/ITF/Codabar"), "barcode-reader": ("barcode_read", P, "EAN-13/UPC-A/EAN-8 only"), "wifi-qr-generator": ("qr_wifi", I, ""),
        "vcard-qr-generator": ("qr_vcard", I, ""), "url-qr-generator": ("qr_url", I, ""), "bulk-qr-generator": ("qr_bulk", I, ""), "qr-code-styler": ("qr_generate", I, "style options"), "qr-code-to-svg": ("qr_generate", I, "formats=[svg]"), "qr-code-to-png": ("qr_generate", I, "formats=[png]"),
    },
    "barcode_labels": {
        "barcode-label-maker": ("barcode_generate", P, "Single label SVG; sheet layout in the browser"), "shipping-label-maker": ("qr_generate", P, ""), "address-label-maker": ("qr_generate", P, ""), "qr-label-sheet-generator": ("qr_bulk", P, "Bulk codes; grid layout in the browser/PDF engine"), "printable-sticker-sheet": ("qr_bulk", P, ""),
    },
    "gis_geo": {
        "geojson-formatter": ("format", I, ""), "geojson-validator": ("validate", I, "RFC 7946 checks"), "geojson-to-csv": ("to_csv", I, ""), "csv-to-geojson": ("from_csv", I, ""), "geojson-to-kml": ("to_kml", I, ""), "kml-to-geojson": ("from_kml", I, ""), "wkt-viewer": ("wkt_parse", I, ""),
        "coordinate-converter": ("convert_coordinate", I, "DD/DMS/UTM/geohash"), "distance-between-coordinates": ("distance", I, "haversine + Vincenty"), "area-from-coordinates": ("area", I, ""), "geojson-simplifier": ("simplify", I, "Douglas–Peucker"), "geojson-bbox": ("bbox", I, ""), "geohash-encoder-decoder": ("geohash", I, ""), "utm-latlon-converter": ("utm", I, ""),
    },
    "color_design": {
        "hex-to-rgb": ("convert", I, ""), "rgb-to-hex": ("convert", I, ""), "rgb-to-hsl": ("convert", I, ""), "hsl-to-rgb": ("convert", I, ""), "color-picker": ("convert", I, ""), "color-palette-generator": ("palette", I, ""), "gradient-generator": ("gradient", I, ""), "gradient-css-generator": ("gradient", I, ""),
        "contrast-checker": ("contrast", I, "WCAG 2"), "accessible-color-pairing": ("accessible_pairs", I, ""), "color-blindness-simulator": ("simulate_color_blindness", I, "colours and images"), "image-palette-extractor": ("image_palette", I, ""), "shadow-generator": ("shadow", I, ""), "border-radius-generator": ("border_radius", I, ""),
        "css-unit-converter": ("css_units", I, ""), "spacing-scale-generator": ("spacing_scale", I, ""), "typography-scale-generator": ("type_scale", I, ""), "favicon-preview": ("favicon_preview", I, ""), "social-image-dimension-checker": ("social_dimensions", I, ""),
    },
    "code_analysis": {
        "code-complexity-analyzer": ("code_complexity", I, "Cyclomatic + nesting heuristics for 15 languages"), "code-line-counter": ("code_line_count", I, "cloc-style"), "code-minifier": ("code_minify", P, "JS/TS/CSS/HTML/SQL/JSON"), "code-beautifier": ("code_format", P, "JS/TS/CSS/HTML/SQL/JSON/GraphQL"),
        "code-diff": ("diff", I, ""), "code-duplicate-detector": ("code_duplicates", I, "Normalised line-window hashing"), "import-sorter": ("import_sort", I, "JS/TS/Python/Rust/Go"), "unused-import-checker": ("unused_imports", I, "Heuristic, JS/TS/Python/Rust/Go"),
        "dependency-license-scanner": ("dependency_scan", P, "Manifests parsed offline; licences from lockfiles or a provided map"), "sql-query-analyzer": ("sql_analyze", I, ""),
        "rust-code-playground": ("sandbox", U, "Code execution belongs to the browser/backend sandbox"), "javascript-sandbox": ("sandbox", U, ""), "html-css-js-playground": ("sandbox", U, ""),
    },
    "ebook": {
        "epub-reader": ("epub_inspect", I, "Also epub_chapter"), "epub-to-pdf": ("epub_to_html", P, "HTML → pdf-engine from_html"), "epub-to-html": ("epub_to_html", I, ""), "mobi-metadata-viewer": ("mobi_metadata", I, "PalmDOC/MOBI/EXTH headers"),
        "ebook-metadata-editor": ("epub_set_metadata", I, "EPUB dc:* fields"), "ebook-cover-extractor": ("epub_cover", I, ""), "ebook-format-converter": ("epub_convert", P, "EPUB → HTML/Markdown/Text/DOCX; Kindle formats unsupported"), "epub-validator": ("epub_validate", I, "Structural OCF/OPF/NAV checks"),
    },
    "3d": {
        "stl-viewer": ("thumbnail", P, "Static software-rendered view; interactive orbit needs the browser WebGL viewer"), "stl-to-obj": ("convert", I, ""), "obj-to-stl": ("convert", I, ""),
        "gltf-viewer": ("gltf_inspect", P, "Structure/statistics only; rendering needs WebGL"), "glb-to-gltf": ("glb_to_gltf", I, "Binary chunk embedded as data: URI"), "gltf-optimizer": ("gltf_optimize", P, "Structural repack only – no Draco/meshopt"),
        "mesh-inspector": ("inspect", I, ""), "mesh-volume-calculator": ("inspect", I, "Signed volume; flags non-watertight meshes"), "mesh-scale": ("transform", I, ""), "mesh-rotate": ("transform", I, ""),
        "mesh-repair-basic": ("repair", I, "Degenerate/duplicate removal, winding flip; no hole filling"), "3d-model-thumbnail": ("thumbnail", I, ""), "point-cloud-viewer": ("inspect", U, "PLY/PCD point clouds need the browser WebGL viewer"),
    },
    "font": {
        "font-metadata-viewer": ("inspect", I, ""), "font-preview": ("preview", I, ""), "font-pairing-tool": ("pairing", I, "Heuristic pairing from classification (serif/sans/mono/display)"),
        "font-converter": ("convert", P, "TTF/OTF ↔ WOFF 1.0; WOFF2 (Brotli) decoding not supported"), "woff-to-ttf": ("convert", I, ""), "ttf-to-woff": ("convert", I, ""), "otf-inspector": ("inspect", I, ""),
        "font-subsetter": ("subset", I, "Rebuilds cmap; GSUB/GPOS layout tables are dropped"), "font-character-map": ("char_map", I, ""), "font-size-checker": ("size_check", I, ""),
    },
    "document": {
        "docx-to-pdf": ("to_html", P, "Structural HTML → pdf-engine from_html; no page-faithful layout"), "docx-to-html": ("to_html", I, ""), "docx-to-text": ("to_text", I, ""), "docx-metadata-viewer": ("inspect", I, ""),
        "docx-template-fill": ("fill_template", I, "{{placeholder}} replacement incl. headers/footers"), "odt-to-docx": ("convert", I, ""), "rtf-to-text": ("to_text", I, ""), "rtf-to-html": ("to_html", I, ""),
        "pptx-to-pdf": ("pptx_slides", P, "Text outline HTML → pdf-engine; slide rendering needs backend"), "pptx-to-images": ("render", U, "Slide rasterisation needs LibreOffice (backend)"), "pptx-metadata-viewer": ("pptx_slides", I, ""),
        "docx-to-markdown": ("to_markdown", I, ""), "markdown-to-docx": ("from_markdown", I, ""),
    },
    "data_visualization": {
        "csv-chart-generator": ("chart", I, "SVG bar/line/area/pie/donut/scatter"), "json-chart-generator": ("chart", I, ""), "histogram-generator": ("histogram", I, ""), "frequency-distribution": ("frequency", I, ""), "pivot-table-lite": ("pivot", I, ""), "data-profiler": ("profile", I, ""),
        "descriptive-statistics": ("describe", I, ""), "outlier-detector": ("outliers", I, "IQR / z-score / MAD"), "data-sampler": ("sample", I, ""),
    },
}

# Tools whose catalog engine differs from the engine that actually serves them.
REDIRECT = {
    "csv-to-excel": "spreadsheet", "excel-to-csv": "spreadsheet",
    "base64-encoder": "encoding", "base64-decoder": "encoding", "url-encoder": "encoding", "url-decoder": "encoding", "html-encoder": "encoding", "html-decoder": "encoding", "hex-encoder": "encoding", "hex-decoder": "encoding",
    "unicode-encode-decode": "text", "json-to-code": "json",
}

FILE_ENGINES = {"pdf", "image", "video", "audio", "archive", "spreadsheet", "qr_barcode", "barcode_labels", "document", "ebook", "font", "3d"}
MAYBE_FILE = {"csv", "encoding", "crypto", "color_design", "data_visualization", "gis_geo", "india_tax_gst"}


# Payload values implied by the tool name (pre-filled in the generic UI).
# Keys must be top-level fields of the action (validated against
# shared/data/engine-schemas.json).
PRESETS = {
    # units
    "length-converter": {"category": "length"}, "weight-converter": {"category": "mass"},
    "area-converter": {"category": "area"}, "volume-converter": {"category": "volume"},
    "speed-converter": {"category": "speed"}, "time-converter": {"category": "time"},
    "pressure-converter": {"category": "pressure"}, "energy-converter": {"category": "energy"},
    "power-converter": {"category": "power"}, "data-size-converter": {"category": "data"},
    "fuel-economy-converter": {"category": "fuel"}, "angle-converter": {"category": "angle"},
    "force-converter": {"category": "force"}, "density-converter": {"category": "density"},
    "frequency-converter": {"category": "frequency"}, "torque-converter": {"category": "torque"},
    # image adjustments
    "image-grayscale": {"adjustments": [{"op": "grayscale"}]},
    "image-blur": {"adjustments": [{"op": "blur", "sigma": 3}]},
    "image-sharpen": {"adjustments": [{"op": "sharpen", "amount": 1.0, "radius": 1.0}]},
    "image-brightness": {"adjustments": [{"op": "brightness", "value": 20}]},
    "image-contrast": {"adjustments": [{"op": "contrast", "value": 20}]},
    "image-saturation": {"adjustments": [{"op": "saturation", "value": 20}]},
    "image-hue": {"adjustments": [{"op": "hue", "degrees": 90}]},
    "image-invert": {"adjustments": [{"op": "invert"}]},
    "image-pixelate": {"pixel_size": 8}, "image-mosaic": {"pixel_size": 16, "colors": 32},
    # hashes
    "md5-generator": {"algorithms": ["md5"]}, "sha1-generator": {"algorithms": ["sha1"]},
    "sha256-generator": {"algorithms": ["sha256"]}, "sha512-generator": {"algorithms": ["sha512"]},
    "sha3-generator": {"algorithms": ["sha3-256"]}, "blake3-generator": {"algorithms": ["blake3"]},
    # qr
    "qr-code-to-svg": {"formats": ["svg"]}, "qr-code-to-png": {"formats": ["png"]},
    # text
    "uppercase-converter": {"target": "upper"}, "lowercase-converter": {"target": "lower"},
    "title-case-converter": {"target": "title"},
    "text-sort-lines": {"operations": [{"op": "sort"}]},
    "text-deduplicator": {"operations": [{"op": "dedupe"}]},
    "remove-empty-lines": {"operations": [{"op": "remove_empty"}]},
    "text-to-binary": {"base": 2}, "text-to-hex": {"base": 16}, "text-to-decimal": {"base": 10},
    "binary-to-text": {"base": 2}, "hex-to-text": {"base": 16},
    "remove-extra-spaces": {"collapse_spaces": True, "trim_lines": True},
    # json code generation
    "json-to-typescript": {"language": "typescript"}, "json-to-rust-struct": {"language": "rust"},
    "json-to-go-struct": {"language": "go"},
    # archives
    "zip-create": {"format": "zip"}, "tar-create": {"format": "tar.gz"}, "gzip-compress": {"format": "gz"},
    # finance
    "simple-interest-calculator": {"compounding": "simple"},
    "compound-interest-calculator": {"compounding": "monthly"},
    # business
    "quotation-generator": {"kind": "quotation"},
    "purchase-order-generator": {"kind": "purchase_order"},
    "invoice-generator": {"kind": "invoice"},
    # audio
    "audio-fade-in": {"fade_in": 2.0}, "audio-fade-out": {"fade_out": 2.0},
    "audio-mono-stereo": {"channels": 2},
    # csv / spreadsheet
    "csv-to-tsv": {"to_delimiter": "tab"}, "tsv-to-csv": {"to_delimiter": ","},
    # developer
    "typescript-formatter": {"indent": 2},
    # math
    "standard-deviation-calculator": {"population": False},
    "matrix-determinant": {"op": "determinant"}, "matrix-inverse": {"op": "inverse"},
    "logarithm-calculator": {"expression": "log10(1000)"},
    "exponent-calculator": {"expression": "2^10"},
}

# Registry tools (shared/data/tools.json) that are served by an existing catalog
# tool: registry slug -> catalog tool id, or (catalog tool id, preset override).
# The preset override replaces the catalog tool's preset for that slug.
REGISTRY_ALIASES = {
    # PDF
    "pdf-to-word": "pdf-to-docx", "word-to-pdf": "docx-to-pdf", "pdf-to-jpg": "pdf-to-images", "pdf-to-png": "pdf-to-images",
    "pdf-to-webp": "pdf-to-images", "jpg-to-pdf": "images-to-pdf", "png-to-pdf": "images-to-pdf",
    "rearrange-pdf-pages": "pdf-reorder-pages", "add-pdf-password": "pdf-protect", "remove-pdf-password": "pdf-unlock",
    "highlight-pdf": "pdf-annotate", "annotate-pdf": "pdf-annotate", "draw-on-pdf": "pdf-annotate",
    "resize-pdf-pages": "pdf-resize-pages", "change-pdf-page-size": "pdf-resize-pages", "pdf-text-extractor": "pdf-to-text",
    "pdf-image-extractor": "pdf-extract-images", "pdf-comparison": "pdf-compare", "flatten-pdf-forms": ("pdf-forms-fill", {"flatten": True}),
    "pdf-to-grayscale": "pdf-grayscale", "pdf-to-markdown": "pdf-to-markdown-catalog",
    # Image
    "jpg-to-png": ("image-convert", {"format": "png"}), "png-to-jpg": ("image-convert", {"format": "jpeg"}),
    "webp-to-jpg": ("image-convert", {"format": "jpeg"}), "gif-to-png": ("image-convert", {"format": "png"}),
    "svg-to-jpg": "svg-to-jpeg", "heic-to-jpg": "heic-converter",
    "image-background-remover": "image-remove-background", "image-blur-tool": "image-blur", "image-sharpen-tool": "image-sharpen",
    "image-denoiser": ("image-adjust", {"adjustments": [{"op": "denoise", "radius": 1}]}),
    "image-brightness-editor": "image-brightness", "image-contrast-editor": "image-contrast", "image-saturation-editor": "image-saturation",
    "image-exposure-editor": ("image-adjust", {"adjustments": [{"op": "exposure", "stops": 0.5}]}),
    "image-hue-editor": "image-hue", "image-temperature-editor": ("image-adjust", {"adjustments": [{"op": "temperature", "value": 20}]}),
    "image-filters": "image-adjust", "grayscale-converter": "image-grayscale",
    "black-and-white-converter": ("image-adjust", {"adjustments": [{"op": "threshold", "level": 128}]}),
    "sepia-filter": ("image-adjust", {"adjustments": [{"op": "sepia"}]}), "image-pixelator": "image-pixelate",
    "image-border-generator": "image-add-border", "rounded-corner-generator": "image-round-corners",
    "photo-grid-maker": "image-contact-sheet", "photo-collage-maker": "image-contact-sheet",
    "thumbnail-maker": ("image-resize", {"width": 320, "fit": "inside"}), "image-comparison-tool": "image-difference",
    "exif-remover": "image-strip-metadata", "exif-editor": "image-metadata-viewer", "image-color-picker": "image-color-extractor",
    "dominant-color-detector": "image-dominant-color", "image-to-base64": "image-base64", "image-to-ascii": "image-ascii-art",
    "image-to-pixel-art": "image-pixelate", "profile-picture-maker": ("image-crop-ratio", {"ratio": "1:1"}),
    "passport-photo-maker": ("image-fit-print", {"width": 35, "height": 45, "unit": "mm", "dpi": 300}),
    "id-photo-maker": ("image-fit-print", {"width": 35, "height": 45, "unit": "mm", "dpi": 300}),
    "meme-generator": "image-watermark", "image-caption-generator": "image-watermark",
    # Video / audio
    "video-cutter": "video-cut", "video-trimmer": "video-trim", "mp4-to-gif": "video-gif", "video-to-images": "video-extract-frames",
    "video-thumbnail-extractor": "video-thumbnail", "video-speed-changer": "video-speed", "slow-motion-creator": "video-speed",
    "time-lapse-creator": "video-speed", "video-rotation": "video-rotate", "video-aspect-ratio-converter": "video-aspect-ratio",
    "subtitle-extractor": "video-subtitle-extract", "srt-to-vtt": ("video-subtitle-convert", {"to": "vtt"}), "vtt-to-srt": ("video-subtitle-convert", {"to": "srt"}),
    "video-mute-tool": "video-mute", "video-metadata-viewer": "video-metadata", "video-thumbnail-maker": "video-contact-sheet",
    "gif-maker": "video-gif", "gif-compressor": "gif-resizer", "mp4-to-webm": "video-convert", "webm-to-mp4": "video-convert",
    "mov-to-mp4": "video-convert", "avi-to-mp4": "video-convert", "mkv-to-mp4": "video-convert",
    "audio-cutter": "audio-cut", "audio-trimmer": "audio-trim", "mp3-to-wav": ("audio-convert", {"output": {"format": "wav"}}),
    "flac-to-mp3": "audio-convert", "m4a-to-mp3": "audio-convert", "ogg-to-mp3": "audio-convert", "wav-to-mp3": "audio-convert", "mp3-to-aac": "audio-convert",
    "audio-volume-booster": "audio-volume", "audio-normalizer": "audio-normalize", "audio-speed-changer": "audio-speed",
    "audio-pitch-changer": "audio-pitch", "audio-fade-in-out": ("audio-fade-in", {"fade_in": 2.0, "fade_out": 2.0}),
    "audio-noise-remover": "audio-noise-reduction", "audio-silence-remover": "audio-silence-removal",
    "audio-waveform-visualizer": ("audio-waveform", {"svg": True}), "audio-spectrum-visualizer": "audio-spectrum",
    "audio-bass-booster": "audio-bass-boost", "audio-treble-booster": "audio-treble-boost",
    "mono-to-stereo": ("audio-mono-stereo", {"channels": 2}), "stereo-to-mono": ("audio-mono-stereo", {"channels": 1}),
    "audio-metadata-viewer": "audio-metadata", "audio-to-spectrogram": ("audio-spectrum", {"spectrogram_frames": 200}),
    "audio-frequency-generator": "audio-tone-generator", "white-noise-generator": ("audio-tone-generator", {"wave": "noise"}),
    "audio-loop-maker": "audio-loop",
    # Text
    "paragraph-counter": "word-counter", "reading-time-calculator": "reading-time", "speaking-time-calculator": "reading-time",
    "sentence-case-converter": ("text-case-converter", {"target": "sentence"}), "remove-duplicate-lines": "text-deduplicator",
    "sort-lines-alphabetically": "text-sort-lines", "reverse-text": "text-reverser", "reverse-words": ("text-reverser", {"mode": "words"}),
    "text-to-morse-code": "morse-code-converter", "morse-code-to-text": "morse-code-to-text-catalog", "text-to-ascii": "text-to-decimal",
    "ascii-to-text": ("binary-to-text", {"base": 10}), "text-to-unicode": ("unicode-encode-decode", {"style": "unicode"}),
    "text-diff-checker": "text-diff", "markdown-previewer": "markdown-to-html", "text-to-qr-code": "qr-code-generator",
    "text-encryptor": "aes-encrypt-decrypt", "text-decryptor": "text-decryptor-catalog", "readability-checker": "word-counter",
    # Developer
    "json-beautifier": "json-formatter", "regex-visualizer": "regex-explainer", "jwt-generator": "jwt-generator-catalog",
    "jwt-validator": "jwt-inspector", "html-entity-encoder": "html-encoder", "html-entity-decoder": "html-decoder",
    "unicode-encoder": "unicode-encode-decode", "unicode-decoder": "unicode-decoder-catalog", "hash-generator": "sha256-generator",
    "sha-1-generator": "sha1-generator", "sha-256-generator": "sha256-generator", "sha-512-generator": "sha512-generator",
    "nanoid-generator": "nanoid-generator-catalog", "epoch-converter": "unix-timestamp-converter", "hsl-converter": "hsl-to-rgb",
    "css-box-shadow-generator": "shadow-generator", "css-border-radius-generator": "border-radius-generator",
    "dockerfile-formatter": "dockerfile-linter", "sql-schema-visualizer": "sql-to-entity-diagram", "er-diagram-generator": "sql-to-entity-diagram",
    "graphql-query-formatter": ("graphql-formatter", {"kind": "query"}), "protobuf-decoder": "protobuf-inspector",
    "jwt-expiration-calculator": "jwt-decoder", "semantic-version-comparator": "semver-checker", "semantic-version-generator": "semver-checker",
    "environment-variable-formatter": "env-file-parser", "env-editor": "env-file-parser", "csv-to-sql-converter": "csv-to-sql",
    "regex-match-extractor": "regex-tester", "html-to-jsx-converter": "html-to-jsx", "sql-query-builder": "sql-formatter",
    "svg-viewer": "svg-info-catalog", "markdown-table-generator": "csv-to-markdown-catalog",
    # Calculators & converters
    "basic-calculator": "scientific-calculator", "income-tax-calculator": "income-tax-estimator", "take-home-salary-calculator": "salary-in-hand-calculator",
    "emi-calculator": "loan-emi-calculator", "loan-calculator": "loan-emi-calculator", "currency-calculator": "currency-converter",
    "days-between-dates": "date-difference-calculator", "time-calculator": "date-add-subtract", "salary-hike-calculator": "salary-hike-catalog",
    "overtime-calculator": "overtime-catalog", "freelance-hourly-rate-calculator": "freelance-rate-catalog", "split-bill-calculator": "tip-calculator",
    "speed-calculator": ("speed-converter", {"category": "speed"}), "distance-calculator": "distance-between-coordinates",
    "data-storage-converter": "data-size-converter", "binary-to-decimal": ("base-converter", {"from_base": 2, "to_base": 10}),
    "decimal-to-binary": ("base-converter", {"from_base": 10, "to_base": 2}), "binary-to-hexadecimal": ("base-converter", {"from_base": 2, "to_base": 16}),
    "hexadecimal-to-decimal": ("base-converter", {"from_base": 16, "to_base": 10}), "octal-converter": ("base-converter", {"from_base": 10, "to_base": 8}),
    "feet-to-meters": ("length-converter", {"category": "length", "from": "ft", "to": "m"}),
    "inches-to-centimeters": ("length-converter", {"category": "length", "from": "in", "to": "cm"}),
    "miles-to-kilometers": ("length-converter", {"category": "length", "from": "mi", "to": "km"}),
    "kg-to-pounds": ("weight-converter", {"category": "mass", "from": "kg", "to": "lb"}),
    "celsius-to-fahrenheit": ("temperature-converter", {"from": "c", "to": "f"}),
    "liters-to-gallons": ("volume-converter", {"category": "volume", "from": "l", "to": "gal_us"}),
    "mb-to-gb": ("data-size-converter", {"category": "data", "from": "mb", "to": "gb"}),
    "mbps-to-mb-s": ("data-size-converter", {"category": "data_rate", "from": "mbps", "to": "MB/s"}),
    "pixels-to-inches": "pixel-converter-catalog", "dpi-calculator": "pixel-converter-catalog", "aspect-ratio-calculator": "video-aspect-ratio",
    "screen-resolution-calculator": "pixel-converter-catalog", "fuel-cost-calculator": "fuel-economy-converter", "mileage-calculator": "fuel-economy-converter",
    # Everyday
    "recipe-measurement-converter": "cooking-unit-converter", "cooking-temperature-converter": "temperature-converter",
    "time-zone-meeting-planner": "meeting-time-planner", "age-in-days-calculator": "age-calculator", "birthday-countdown": ("countdown-calculator", {"yearly": True}),
    "loan-payoff-calculator": "loan-emi-calculator", "savings-goal-calculator": "sip-calculator", "rent-split-calculator": "tip-calculator",
    "daily-expense-splitter": "tip-calculator", "trip-fuel-calculator": "fuel-economy-converter", "vehicle-mileage-calculator": "fuel-economy-converter",
    # File tools
    "qr-code-scanner": "qr-code-reader", "barcode-scanner": "barcode-reader", "file-hash-checker": "sha256-generator",
    "file-checksum-calculator": "sha256-generator", "file-checksum-verifier": "sha256-generator", "zip-compressor": "zip-create",
    "gzip-decompressor": "gzip-decompress", "7z-viewer-extractor": "7z-extract", "file-extension-detector": "mime-type-lookup",
    "mime-type-detector": "mime-type-lookup", "file-signature-detector": "mime-type-lookup", "file-size-converter": "data-size-converter",
    "file-comparison": "text-diff", "excel-viewer": "xlsx-viewer", "excel-formula-helper": "spreadsheet-formula-evaluator",
    "xml-viewer": "xml-formatter", "mobi-reader": "mobi-metadata-viewer", "file-metadata-viewer": "image-metadata-viewer",
    # Security
    "password-strength-tester": "password-strength-checker", "hash-calculator": "sha256-generator", "sha-checksum-verifier": "sha256-generator",
    "text-encryption-tool": "aes-encrypt-decrypt", "text-decryption-tool": "text-decryptor-catalog", "aes-encryption-tool": "aes-encrypt-decrypt",
    "aes-decryption-tool": "text-decryptor-catalog", "rsa-key-pair-generator": "rsa-key-generator", "certificate-viewer": "certificate-decoder",
    "ssl-certificate-decoder": "certificate-decoder", "totp-authenticator": "totp-catalog", "qr-based-2fa-generator": "totp-uri-catalog",
    "exif-data-remover": "image-strip-metadata", "metadata-stripper": "image-strip-metadata",
    # Design
    "hex-color-converter": "hex-to-rgb", "rgb-color-converter": "rgb-to-hex", "hsl-color-converter": "hsl-to-rgb", "cmyk-converter": "color-picker",
    "gradient-palette-generator": "gradient-generator", "complementary-color-finder": ("color-palette-generator", {"scheme": "complementary"}),
    "analogous-color-finder": ("color-palette-generator", {"scheme": "analogous"}), "wcag-accessibility-checker": "contrast-checker",
    "font-pairing-generator": "font-pairing-tool", "font-previewer": "font-preview", "app-icon-generator": "favicon-generator",
    "box-shadow-generator": "shadow-generator", "neumorphism-generator": ("shadow-generator", {"preset": "neumorphic"}),
    "glassmorphism-generator": ("shadow-generator", {"preset": "soft"}), "open-graph-image-generator": "og-image-generator",
    "social-media-post-resizer": "social-image-dimension-checker", "instagram-story-resizer": "social-image-dimension-checker",
    # SEO
    "word-count-analyzer": "word-counter", "url-slug-generator": "slug-generator", "qr-code-generator-for-urls": "url-qr-generator",
    "heading-hierarchy-checker": "markdown-linter", "html-source-viewer": "html-formatter",
    # Math
    "algebra-calculator": "linear-equation-solver", "equation-solver": "quadratic-equation-solver",
    "matrix-multiplication": ("matrix-calculator", {"op": "multiply"}), "determinant-calculator": "matrix-determinant",
    "derivative-calculator": "derivative-catalog", "integral-calculator": "integral-catalog", "limit-calculator": "limit-catalog",
    "scientific-notation-converter": "scientific-notation-catalog", "percentage-change-calculator": ("percentage-calculator", {"kind": "change"}),
    "mean-median-mode-calculator": "statistics-calculator", "fibonacci-generator": ("sequence-generator", {"kind": "fibonacci"}),
    "graph-plotter": "equation-graph-data", "triangle-calculator": "triangle-catalog", "pythagorean-theorem-calculator": "triangle-catalog",
    "geometry-calculator": "triangle-catalog", "coordinate-distance-calculator": "distance-between-coordinates",
    # Social
    "x-twitter-character-counter": "character-counter", "instagram-post-resizer": "social-image-dimension-checker",
    "social-media-image-resizer": "social-image-dimension-checker", "profile-picture-cropper": ("image-crop-ratio", {"ratio": "1:1"}),
    "meme-maker": "image-watermark", "social-media-preview-generator": "og-image-generator", "youtube-thumbnail-maker": ("og-image-generator", {"width": 1280, "height": 720}),
    "video-caption-formatter": "video-subtitle-convert",
}

# Catalog-free targets: actions that exist in an engine but have no catalog tool id
# yet. They are registered as synthetic catalog entries so aliases can point at them.
EXTRA_CATALOG_TOOLS = {
    "pdf-to-markdown-catalog": ("pdf", "to_markdown", "PDF & Documents", {}),
    "morse-code-to-text-catalog": ("text", "morse_decode", "Text Tools", {}),
    "text-decryptor-catalog": ("crypto", "decrypt", "Security & Network", {}),
    "jwt-generator-catalog": ("crypto", "jwt_sign", "Developer Tools", {}),
    "unicode-decoder-catalog": ("text", "unicode_unescape", "Developer Tools", {}),
    "nanoid-generator-catalog": ("crypto", "nanoid", "Developer Tools", {}),
    "svg-info-catalog": ("image", "svg_info", "Developer Tools", {}),
    "csv-to-markdown-catalog": ("csv", "to_markdown", "Developer Tools", {}),
    "salary-hike-catalog": ("finance", "salary_hike", "Calculators & Converters", {}),
    "overtime-catalog": ("finance", "overtime", "Calculators & Converters", {}),
    "freelance-rate-catalog": ("finance", "freelance_rate", "Calculators & Converters", {}),
    "pixel-converter-catalog": ("unit_conversion", "pixels", "Calculators & Converters", {}),
    "totp-catalog": ("crypto", "totp", "Security & Network", {}),
    "totp-uri-catalog": ("crypto", "totp_uri", "Security & Network", {}),
    "derivative-catalog": ("math", "derivative", "Math Tools", {}),
    "integral-catalog": ("math", "integral", "Math Tools", {}),
    "limit-catalog": ("math", "limit", "Math Tools", {}),
    "scientific-notation-catalog": ("math", "scientific_notation", "Math Tools", {}),
    "triangle-catalog": ("math", "triangle", "Math Tools", {}),
    "image-ascii-art": ("image", "to_ascii", "Image Tools", {}),
    "image-remove-background": ("image", "remove_background", "Image Tools", {}),
    "image-adjust": ("image", "adjust", "Image Tools", {}),
    "image-crop-ratio": ("image", "crop_ratio", "Image Tools", {}),
    "image-fit-print": ("image", "fit_print_size", "Image Tools", {}),
    "pdf-annotate": ("pdf", "annotate", "PDF & Documents", {}),
    "pdf-resize-pages": ("pdf", "resize_pages", "PDF & Documents", {}),
    "pdf-extract-images": ("pdf", "extract_images", "PDF & Documents", {}),
    "video-subtitle-convert": ("video", "subtitle_convert", "Video Tools", {}),
}

FORMAT_ALIASES = {"excel": "xlsx", "jpg": "jpeg", "word": "docx", "tiff": "tiff", "markdown": "markdown", "md": "markdown"}


def implied_preset(tool_id: str, action_fields: set) -> dict:
    """`<x>-to-<fmt>` tools pre-fill the target format field when the action has one."""
    preset = dict(PRESETS.get(tool_id, {}))
    m = re.match(r"^.+-to-([a-z0-9]+)$", tool_id)
    if m and not preset:
        fmt = FORMAT_ALIASES.get(m.group(1), m.group(1))
        for key in ("to", "format", "to_format", "target"):
            if key in action_fields and fmt not in ("text", "code", "image", "json"):
                preset[key] = fmt
                break
    return preset


def norm(s: str):
    syn = {"converter": "convert", "generator": "generate", "calculator": "calc", "checker": "check", "validator": "validate", "formatter": "format", "encoder": "encode", "decoder": "decode", "viewer": "view", "editor": "edit", "extractor": "extract", "analyzer": "analyze", "detector": "detect", "remover": "remove", "compressor": "compress", "resizer": "resize", "cropper": "crop", "and": "", "minifier": "minify", "beautifier": "beautify", "parser": "parse", "counter": "count", "tester": "test", "explainer": "explain", "maker": "make", "splitter": "split", "merger": "merge", "rotator": "rotate", "sorter": "sort", "creator": "create", "picker": "pick", "simulator": "simulate", "builder": "build", "inspector": "inspect", "estimator": "estimate", "planner": "plan", "reader": "read", "scanner": "scan", "comparer": "compare"}
    out = []
    for k in s.lower().replace("_", "-").split("-"):
        k = syn.get(k, k)
        if k:
            out.append(k.rstrip("s") if len(k) > 3 else k)
    return tuple(sorted(out))


def load_schema_fields():
    path = os.path.join(ROOT, "shared", "data", "engine-schemas.json")
    if not os.path.exists(path):
        return {}
    data = json.load(open(path))
    out = {}
    for eid, eng in data["engines"].items():
        for action, spec in eng["actions"].items():
            out[(eid, action)] = {f["name"] for f in spec["fields"]}
    return out


def main():
    schema_fields = load_schema_fields()
    registry = {t["slug"]: t for t in TOOLS_JSON}
    by_norm = {}
    for slug in registry:
        by_norm.setdefault(norm(slug), []).append(slug)
    tools = []
    engines_out = {}
    seen = set()
    for eng in CATALOG["engines"]:
        cid = eng["engine_id"]
        for tool_id in eng["tools"]:
            if tool_id in seen:
                continue
            seen.add(tool_id)
            serving = REDIRECT.get(tool_id, cid)
            entry = {"id": tool_id, "catalog_engine": cid, "runtime": eng["runtime"], "priority": eng["priority"]}
            # Registry alias.
            if tool_id in registry:
                entry["registry_slug"] = tool_id
            else:
                cands = by_norm.get(norm(tool_id), [])
                if len(cands) == 1:
                    entry["registry_slug"] = cands[0]
            if serving in ENGINES:
                reg_id, crate, wasm, ui_cat = ENGINES[serving]
                mapping = MAP.get(serving, {}).get(tool_id) or MAP.get(cid, {}).get(tool_id)
                if mapping is None:
                    raise SystemExit(f"no mapping for {tool_id} ({cid})")
                action, status, note = mapping
                entry.update({"engine": reg_id, "action": action, "status": status, "ui_category": ui_cat})
                if note:
                    entry["note"] = note
                fields = schema_fields.get((reg_id, action))
                if fields is not None:
                    preset = implied_preset(tool_id, fields)
                    unknown = [k for k in preset if k not in fields]
                    if unknown:
                        raise SystemExit(f"preset for {tool_id} uses unknown fields {unknown} (action {action})")
                    if preset:
                        entry["preset"] = preset
                elif tool_id in PRESETS:
                    raise SystemExit(f"preset for {tool_id} but action {action} has no schema")
                requires = []
                if serving in FILE_ENGINES or (serving in MAYBE_FILE and any(w in tool_id for w in ["image", "excel", "xlsx", "encoding", "certificate", "favicon", "qr", "barcode", "bank", "kml"])):
                    requires.append("files")
                if serving not in FILE_ENGINES:
                    requires.append("text")
                entry["requires"] = requires
                entry["network"] = False
                engines_out.setdefault(reg_id, {"id": reg_id, "crate": crate, "wasm": wasm, "tools": []})["tools"].append(tool_id)
            elif eng["runtime"] == "wasm":
                entry.update({"engine": None, "action": None, "status": PLANNED.get(cid, "not-started"), "requires": ["files"] if cid in {"ocr", "font", "ebook", "3d", "document", "speech", "ai_local"} else ["text"], "network": False, "note": eng.get("notes", "")})
            else:
                entry.update({"engine": None, "action": None, "status": "browser" if eng["runtime"] == "javascript" else "backend", "requires": ["text"], "network": eng["runtime"] == "backend", "note": eng.get("notes", "")})
            tools.append(entry)
    # Synthetic catalog entries for engine actions without a catalog tool id.
    for tool_id, (cid, action, ui_cat, preset) in EXTRA_CATALOG_TOOLS.items():
        if tool_id in seen:
            raise SystemExit(f"extra catalog tool {tool_id} clashes with catalog.json")
        reg_id, crate, wasm, _ = ENGINES[cid]
        if (reg_id, action) not in schema_fields:
            raise SystemExit(f"extra catalog tool {tool_id}: action {action} not found in engine {reg_id}")
        entry = {"id": tool_id, "catalog_engine": cid, "runtime": "wasm", "priority": "P2", "engine": reg_id, "action": action,
                 "status": "implemented", "ui_category": ui_cat, "requires": ["files"] if cid in FILE_ENGINES else ["text"], "network": False,
                 "note": "Registered for registry aliases (no catalog.json id)"}
        if preset:
            entry["preset"] = preset
        tools.append(entry)
        engines_out.setdefault(reg_id, {"id": reg_id, "crate": crate, "wasm": wasm, "tools": []})["tools"].append(tool_id)
        seen.add(tool_id)
    # Registry aliases: several registry slugs can share one catalog tool.
    by_id = {t["id"]: t for t in tools}
    taken = {t.get("registry_slug") for t in tools}
    for slug, target in REGISTRY_ALIASES.items():
        if slug not in registry:
            raise SystemExit(f"alias {slug} is not a registry slug")
        if slug in taken:
            raise SystemExit(f"alias {slug} is already mapped")
        tool_id, override = (target, None) if isinstance(target, str) else target
        entry = by_id.get(tool_id)
        if entry is None or not entry.get("engine"):
            raise SystemExit(f"alias {slug} -> unknown catalog tool {tool_id}")
        entry.setdefault("registry_slugs", []).append(slug)
        if override is not None:
            fields = schema_fields.get((entry["engine"], entry["action"]), set())
            unknown = [k for k in override if k not in fields]
            if unknown:
                raise SystemExit(f"alias {slug}: preset uses unknown fields {unknown} (action {entry['action']})")
            entry.setdefault("slug_presets", {})[slug] = override
        taken.add(slug)
    counts = {}
    for t in tools:
        counts[t["status"]] = counts.get(t["status"], 0) + 1
    out = {
        "schema_version": "1.0.0",
        "generated_by": "scripts/generate-engine-tools.py",
        "summary": {"catalog_tools": len(tools), "by_status": counts, "registry_matches": sum(1 for t in tools if "registry_slug" in t) + sum(len(t.get("registry_slugs", [])) for t in tools)},
        "engines": list(engines_out.values()),
        "tools": tools,
    }
    path = os.path.join(ROOT, "shared", "data", "engine-tools.json")
    json.dump(out, open(path, "w"), indent=2)
    print(json.dumps(out["summary"], indent=2))


if __name__ == "__main__":
    main()
