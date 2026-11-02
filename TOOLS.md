# Dev Toolbox — Comprehensive Tools & Status Registry

A complete catalog of all utilities, categories, WASM engines, and operational statuses across the Dev Toolbox platform.

## Platform Overview

- **Total Registered Utilities**: `1022`
- **Live & Interactive Tools (WASM / Client Execution)**: `766` (75.0%)
- **Queued for Engine Implementation (Coming Soon)**: `256` (25.0%)
- **Active WASM Processing Engines**: `28`
- **Total Tool Categories**: `17`

### Architectural Summary

| Metric | Count | Operational Behavior |
| :--- | :---: | :--- |
| **Available (Live)** | **766** | Executes fully client-side in browser Web Workers with zero file uploads |
| **Coming Soon** | **256** | Pre-registered in catalog with metadata and queued for WASM engine bindings |
| **Total Tools** | **1022** | Total unified platform inventory |

---

## Category Summary

| Category | Total Tools | Available (Live) | Coming Soon | Primary Engine(s) |
| :--- | :---: | :---: | :---: | :--- |
| **AI Tools** | 25 | 0 | 25 | *Queued* |
| **Audio Tools** | 47 | 40 | 7 | `audio-engine` |
| **Calculators & Converters** | 151 | 141 | 10 | `construction-engine`, `datetime-engine`, `finance-engine` +6 more |
| **Creative Tools** | 25 | 1 | 24 | `qrbarcode-engine` |
| **Design Tools** | 57 | 50 | 7 | `colordesign-engine`, `font-engine`, `image-engine` +1 more |
| **Developer Tools** | 159 | 141 | 18 | `colordesign-engine`, `crypto-engine`, `csv-engine` +8 more |
| **Everyday Tools** | 41 | 28 | 13 | `datetime-engine`, `finance-engine`, `qrbarcode-engine` +2 more |
| **File Tools** | 74 | 66 | 8 | `archive-engine`, `crypto-engine`, `csv-engine` +8 more |
| **Image Tools** | 71 | 63 | 8 | `colordesign-engine`, `image-engine` |
| **Math Tools** | 39 | 38 | 1 | `geo-engine`, `math-engine` |
| **PDF & Documents** | 75 | 63 | 12 | `document-engine`, `pdf-engine` |
| **Productivity** | 39 | 19 | 20 | `business-engine`, `datetime-engine`, `math-engine` |
| **SEO & Website** | 25 | 5 | 20 | `developer-engine`, `qrbarcode-engine`, `text-engine` |
| **Security & Network** | 58 | 25 | 33 | `crypto-engine`, `developer-engine`, `image-engine` |
| **Social Media** | 20 | 8 | 12 | `colordesign-engine`, `image-engine`, `text-engine` +1 more |
| **Text Tools** | 60 | 54 | 6 | `crypto-engine`, `document-engine`, `pdf-engine` +2 more |
| **Video Tools** | 56 | 24 | 32 | `audio-engine`, `image-engine`, `video-engine` |

---

## Registered WASM Engines

| Engine ID | Version | Tools Served | Primary Capabilities |
| :--- | :---: | :---: | :--- |
| `archive-engine` | `v1.0.0` | 15 | zip, tar, gzip, list, … |
| `audio-engine` | `v2.0.0` | 41 | file-size-calculation, bpm-to-milliseconds, decode, wav-flac-encode, … |
| `business-engine` | `v1.0.0` | 15 | documents, profit-loss, margin, inventory, … |
| `colordesign-engine` | `v1.0.0` | 34 | color-convert, palette, gradient, contrast, … |
| `construction-engine` | `v1.0.0` | 20 | concrete, masonry, plaster, paint, … |
| `crypto-engine` | `v1.0.0` | 41 | hash, hmac, bcrypt, argon2, … |
| `csv-engine` | `v1.0.0` | 22 | parse, clean, validate, convert, … |
| `dataviz-engine` | `v1.0.0` | 9 | svg-charts, histogram, frequency, pivot, … |
| `datetime-engine` | `v1.0.0` | 19 | parse, difference, add, age, … |
| `developer-engine` | `v1.0.0` | 53 | regex, cron, semver, color, … |
| `document-engine` | `v1.0.0` | 19 | docx, pptx, odt, rtf, … |
| `encoding-engine` | `v1.0.0` | 17 | base64, base32, hex, url, … |
| `finance-engine` | `v1.0.0` | 34 | loan, mortgage, interest, investment, … |
| `font-engine` | `v1.0.0` | 10 | ttf, otf, woff, inspect, … |
| `geo-engine` | `v1.0.0` | 15 | geojson, kml, wkt, coordinates, … |
| `image-engine` | `v2.0.0` | 77 | aspect-ratio, grayscale-filter, dimension-calc, crop-calculation, … |
| `indiatax-engine` | `v1.0.0` | 19 | gst, itc, gstin, hsn, … |
| `json-engine` | `v1.0.0` | 39 | json, yaml, xml, toml, … |
| `math-engine` | `v1.0.0` | 47 | evaluate, fractions, statistics, primes, … |
| `mesh3d-engine` | `v1.0.0` | 12 | stl, obj, gltf, glb, … |
| `pdf-engine` | `v2.0.0` | 51 | pdf-inspection, page-count, encryption-check, merge, … |
| `qrbarcode-engine` | `v1.0.0` | 19 | qr-generate, qr-decode, qr-payloads, bulk, … |
| `search-engine` | `v0.1.0` | 0 | fuzzy-search, levenshtein-scoring, category-filtering |
| `solar-engine` | `v1.0.0` | 11 | panel-size, roi, battery, inverter, … |
| `spreadsheet-engine` | `v1.0.0` | 18 | xlsx, xls, xlsb, ods, … |
| `text-engine` | `v2.0.0` | 55 | case-conversion, text-analysis, word-counting, reading-time, … |
| `units-engine` | `v1.0.0` | 37 | convert, temperature, cooking, shoe-size, … |
| `video-engine` | `v2.0.0` | 39 | bitrate-calculation, frame-calculation, inspect, mp4-lossless-trim, … |

---

## Detailed Tool Catalog by Category

### AI Tools (25 tools — 0 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 605 | **AI Code Explainer** | `ai-code-explainer` | ⏳ Coming Soon | — | — | Free online ai code explainer utility. This tool is planned and coming soon. |
| 609 | **AI Commit Message Generator** | `ai-commit-message-generator` | ⏳ Coming Soon | — | — | Free online ai commit message generator utility. This tool is planned and coming soon. |
| 590 | **AI Cover Letter Generator** | `ai-cover-letter-generator` | ⏳ Coming Soon | — | — | Free online ai cover letter generator utility. This tool is planned and coming soon. |
| 594 | **AI Document Summarizer** | `ai-document-summarizer` | ⏳ Coming Soon | — | — | Free online ai document summarizer utility. This tool is planned and coming soon. |
| 589 | **AI Email Writer** | `ai-email-writer` | ⏳ Coming Soon | — | — | Free online ai email writer utility. This tool is planned and coming soon. |
| 588 | **AI Grammar Fixer** | `ai-grammar-fixer` | ⏳ Coming Soon | — | — | Free online ai grammar fixer utility. This tool is planned and coming soon. |
| 596 | **AI Image Background Remover** | `ai-image-background-remover` | ⏳ Coming Soon | — | — | Free online ai image background remover utility. This tool is planned and coming soon. |
| 595 | **AI Image Caption Generator** | `ai-image-caption-generator` | ⏳ Coming Soon | — | — | Free online ai image caption generator utility. This tool is planned and coming soon. |
| 599 | **AI Image Upscaler** | `ai-image-upscaler` | ⏳ Coming Soon | — | — | Free online ai image upscaler utility. This tool is planned and coming soon. |
| 604 | **AI Meeting Summarizer** | `ai-meeting-summarizer` | ⏳ Coming Soon | — | — | Free online ai meeting summarizer utility. This tool is planned and coming soon. |
| 597 | **AI Object Remover** | `ai-object-remover` | ⏳ Coming Soon | — | — | Free online ai object remover utility. This tool is planned and coming soon. |
| 593 | **AI PDF Question Answering** | `ai-pdf-question-answering` | ⏳ Coming Soon | — | — | Free online ai pdf question answering utility. This tool is planned and coming soon. |
| 587 | **AI Paraphraser** | `ai-paraphraser` | ⏳ Coming Soon | — | — | Free online ai paraphraser utility. This tool is planned and coming soon. |
| 598 | **AI Photo Enhancer** | `ai-photo-enhancer` | ⏳ Coming Soon | — | — | Free online ai photo enhancer utility. This tool is planned and coming soon. |
| 610 | **AI Presentation Generator** | `ai-presentation-generator` | ⏳ Coming Soon | — | — | Free online ai presentation generator utility. This tool is planned and coming soon. |
| 607 | **AI Regex Generator** | `ai-regex-generator` | ⏳ Coming Soon | — | — | Free online ai regex generator utility. This tool is planned and coming soon. |
| 592 | **AI Resume Builder** | `ai-resume-builder` | ⏳ Coming Soon | — | — | Free online ai resume builder utility. This tool is planned and coming soon. |
| 591 | **AI Resume Reviewer** | `ai-resume-reviewer` | ⏳ Coming Soon | — | — | Free online ai resume reviewer utility. This tool is planned and coming soon. |
| 606 | **AI SQL Generator** | `ai-sql-generator` | ⏳ Coming Soon | — | — | Free online ai sql generator utility. This tool is planned and coming soon. |
| 600 | **AI Speech-to-Text** | `ai-speech-to-text` | ⏳ Coming Soon | — | — | Free online ai speech-to-text utility. This tool is planned and coming soon. |
| 602 | **AI Subtitle Generator** | `ai-subtitle-generator` | ⏳ Coming Soon | — | — | Free online ai subtitle generator utility. This tool is planned and coming soon. |
| 608 | **AI Test Case Generator** | `ai-test-case-generator` | ⏳ Coming Soon | — | — | Free online ai test case generator utility. This tool is planned and coming soon. |
| 586 | **AI Text Summarizer** | `ai-text-summarizer` | ⏳ Coming Soon | — | — | Free online ai text summarizer utility. This tool is planned and coming soon. |
| 601 | **AI Text-to-Speech** | `ai-text-to-speech` | ⏳ Coming Soon | — | — | Free online ai text-to-speech utility. This tool is planned and coming soon. |
| 603 | **AI Translation Tool** | `ai-translation-tool` | ⏳ Coming Soon | — | — | Free online ai translation tool utility. This tool is planned and coming soon. |

### Audio Tools (47 tools — 40 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 184 | **Audio Bass Booster** | `audio-bass-booster` | ✅ Available | `audio-engine` | `bass_boost` | Free online audio bass booster utility. This tool is planned and coming soon. |
| 186 | **Audio Channel Converter** | `audio-channel-converter` | ✅ Available | `audio-engine` | `convert` | Free online audio channel converter utility. This tool is planned and coming soon. |
| 165 | **Audio Compressor** | `audio-compressor` | ✅ Available | `audio-engine` | `convert` | Free online audio compressor utility. This tool is planned and coming soon. |
| 100028 | **Audio Concat** | `audio-concat` | ✅ Available | `audio-engine` | `join` | Audio Concat utility powered by audio-engine. |
| 166 | **Audio Converter** | `audio-converter` | ✅ Available | `audio-engine` | `convert` | Free online audio converter utility. This tool is planned and coming soon. |
| 161 | **Audio Cutter** | `audio-cutter` | ✅ Available | `audio-engine` | `cut` | Free online audio cutter utility. This tool is planned and coming soon. |
| 100031 | **Audio Duration** | `audio-duration` | ✅ Available | `audio-engine` | `inspect` | Audio Duration utility powered by audio-engine. |
| 183 | **Audio Equalizer** | `audio-equalizer` | ✅ Available | `audio-engine` | `equalizer` | Free online audio equalizer utility. This tool is planned and coming soon. |
| 100030 | **Audio Extract From Video** | `audio-extract-from-video` | ✅ Available | `audio-engine` | `extract_from_video` | Audio Extract From Video utility powered by audio-engine. |
| 178 | **Audio Fade In/Out** | `audio-fade-in-out` | ✅ Available | `audio-engine` | `fade` | Free online audio fade in/out utility. This tool is planned and coming soon. |
| 100029 | **Audio Fade Out** | `audio-fade-out` | ✅ Available | `audio-engine` | `fade` | Audio Fade Out utility powered by audio-engine. |
| 198 | **Audio Frequency Generator** | `audio-frequency-generator` | ✅ Available | `audio-engine` | `tone` | Free online audio frequency generator utility. This tool is planned and coming soon. |
| 100033 | **Audio Join** | `audio-join` | ✅ Available | `audio-engine` | `join` | Audio Join utility powered by audio-engine. |
| 200 | **Audio Loop Maker** | `audio-loop-maker` | ✅ Available | `audio-engine` | `loop` | Free online audio loop maker utility. This tool is planned and coming soon. |
| 163 | **Audio Merger** | `audio-merger` | ✅ Available | `audio-engine` | `mix` | Free online audio merger utility. This tool is planned and coming soon. |
| 190 | **Audio Metadata Viewer** | `audio-metadata-viewer` | ✅ Available | `audio-engine` | `inspect` | Free online audio metadata viewer utility. This tool is planned and coming soon. |
| 179 | **Audio Noise Remover** | `audio-noise-remover` | ✅ Available | `audio-engine` | `noise_reduction` | Free online audio noise remover utility. This tool is planned and coming soon. |
| 174 | **Audio Normalizer** | `audio-normalizer` | ✅ Available | `audio-engine` | `normalize` | Free online audio normalizer utility. This tool is planned and coming soon. |
| 176 | **Audio Pitch Changer** | `audio-pitch-changer` | ✅ Available | `audio-engine` | `pitch` | Free online audio pitch changer utility. This tool is planned and coming soon. |
| 177 | **Audio Reverse** | `audio-reverse` | ✅ Available | `audio-engine` | `reverse` | Free online audio reverse utility. This tool is planned and coming soon. |
| 100032 | **Audio Sample Rate Converter** | `audio-sample-rate-converter` | ✅ Available | `audio-engine` | `convert` | Audio Sample Rate Converter utility powered by audio-engine. |
| 180 | **Audio Silence Remover** | `audio-silence-remover` | ✅ Available | `audio-engine` | `silence_removal` | Free online audio silence remover utility. This tool is planned and coming soon. |
| 182 | **Audio Spectrum Visualizer** | `audio-spectrum-visualizer` | ✅ Available | `audio-engine` | `spectrum` | Free online audio spectrum visualizer utility. This tool is planned and coming soon. |
| 175 | **Audio Speed Changer** | `audio-speed-changer` | ✅ Available | `audio-engine` | `speed` | Free online audio speed changer utility. This tool is planned and coming soon. |
| 164 | **Audio Splitter** | `audio-splitter` | ✅ Available | `audio-engine` | `split` | Free online audio splitter utility. This tool is planned and coming soon. |
| 185 | **Audio Treble Booster** | `audio-treble-booster` | ✅ Available | `audio-engine` | `treble_boost` | Free online audio treble booster utility. This tool is planned and coming soon. |
| 162 | **Audio Trimmer** | `audio-trimmer` | ✅ Available | `audio-engine` | `trim` | Free online audio trimmer utility. This tool is planned and coming soon. |
| 100034 | **Audio Visualizer** | `audio-visualizer` | ✅ Available | `audio-engine` | `spectrum` | Audio Visualizer utility powered by audio-engine. |
| 173 | **Audio Volume Booster** | `audio-volume-booster` | ✅ Available | `audio-engine` | `volume` | Free online audio volume booster utility. This tool is planned and coming soon. |
| 181 | **Audio Waveform Visualizer** | `audio-waveform-visualizer` | ✅ Available | `audio-engine` | `waveform` | Free online audio waveform visualizer utility. This tool is planned and coming soon. |
| 193 | **Audio to Spectrogram** | `audio-to-spectrogram` | ✅ Available | `audio-engine` | `spectrum` | Free online audio to spectrogram utility. This tool is planned and coming soon. |
| 170 | **FLAC to MP3** | `flac-to-mp3` | ✅ Available | `audio-engine` | `convert` | Free online flac to mp3 utility. This tool is planned and coming soon. |
| 171 | **M4A to MP3** | `m4a-to-mp3` | ✅ Available | `audio-engine` | `convert` | Free online m4a to mp3 utility. This tool is planned and coming soon. |
| 169 | **MP3 to AAC** | `mp3-to-aac` | ✅ Available | `audio-engine` | `convert` | Free online mp3 to aac utility. This tool is planned and coming soon. |
| 167 | **MP3 to WAV** | `mp3-to-wav` | ✅ Available | `audio-engine` | `convert` | Free online mp3 to wav utility. This tool is planned and coming soon. |
| 187 | **Mono to Stereo** | `mono-to-stereo` | ✅ Available | `audio-engine` | `convert` | Free online mono to stereo utility. This tool is planned and coming soon. |
| 172 | **OGG to MP3** | `ogg-to-mp3` | ✅ Available | `audio-engine` | `convert` | Free online ogg to mp3 utility. This tool is planned and coming soon. |
| 188 | **Stereo to Mono** | `stereo-to-mono` | ✅ Available | `audio-engine` | `convert` | Free online stereo to mono utility. This tool is planned and coming soon. |
| 168 | **WAV to MP3** | `wav-to-mp3` | ✅ Available | `audio-engine` | `convert` | Free online wav to mp3 utility. This tool is planned and coming soon. |
| 199 | **White Noise Generator** | `white-noise-generator` | ✅ Available | `audio-engine` | `tone` | Free online white noise generator utility. This tool is planned and coming soon. |
| 192 | **Album Art Editor** | `album-art-editor` | ⏳ Coming Soon | — | — | Free online album art editor utility. This tool is planned and coming soon. |
| 189 | **Audio Metadata Editor** | `audio-metadata-editor` | ⏳ Coming Soon | — | — | Free online audio metadata editor utility. This tool is planned and coming soon. |
| 194 | **Audio Recorder** | `audio-recorder` | ⏳ Coming Soon | — | — | Free online audio recorder utility. This tool is planned and coming soon. |
| 191 | **ID3 Tag Editor** | `id3-tag-editor` | ⏳ Coming Soon | — | — | Free online id3 tag editor utility. This tool is planned and coming soon. |
| 196 | **Microphone Tester** | `microphone-tester` | ⏳ Coming Soon | — | — | Free online microphone tester utility. This tool is planned and coming soon. |
| 197 | **Speaker Tester** | `speaker-tester` | ⏳ Coming Soon | — | — | Free online speaker tester utility. This tool is planned and coming soon. |
| 195 | **Voice Recorder** | `voice-recorder` | ⏳ Coming Soon | — | — | Free online voice recorder utility. This tool is planned and coming soon. |

### Calculators & Converters (151 tools — 141 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 337 | **Age Calculator** | `age-calculator` | ✅ Available | `datetime-engine` | `age` | Free online age calculator utility. This tool is planned and coming soon. |
| 100191 | **Aggregate Calculator** | `aggregate-calculator` | ✅ Available | `construction-engine` | `concrete` | Aggregate Calculator utility powered by construction-engine. |
| 100160 | **Angle Converter** | `angle-converter` | ✅ Available | `units-engine` | `convert` | Angle Converter utility powered by units-engine. |
| 100215 | **Appliance Energy Calculator** | `appliance-energy-calculator` | ✅ Available | `solar-engine` | `appliances` | Appliance Energy Calculator utility powered by solar-engine. |
| 364 | **Area Converter** | `area-converter` | ✅ Available | `units-engine` | `convert` | Free online area converter utility. This tool is planned and coming soon. |
| 100269 | **Area From Coordinates** | `area-from-coordinates` | ✅ Available | `geo-engine` | `area` | Area From Coordinates utility powered by geo-engine. |
| 388 | **Aspect Ratio Calculator** | `aspect-ratio-calculator` | ✅ Available | `video-engine` | `aspect_ratio` | Free online aspect ratio calculator utility. This tool is planned and coming soon. |
| 100214 | **Backup Time Calculator** | `backup-time-calculator` | ✅ Available | `solar-engine` | `battery` | Backup Time Calculator utility powered by solar-engine. |
| 100183 | **Bank Statement Cleaner** | `bank-statement-cleaner` | ✅ Available | `indiatax-engine` | `bank_statement` | Bank Statement Cleaner utility powered by indiatax-engine. |
| 100184 | **Bank Statement To CSV** | `bank-statement-to-csv` | ✅ Available | `indiatax-engine` | `bank_statement` | Bank Statement To CSV utility powered by indiatax-engine. |
| 100185 | **Bank Statement To Tally** | `bank-statement-to-tally` | ✅ Available | `indiatax-engine` | `bank_statement` | Bank Statement To Tally utility powered by indiatax-engine. |
| 311 | **Basic Calculator** | `basic-calculator` | ✅ Available | `math-engine` | `evaluate` | Free online basic calculator utility. This tool is planned and coming soon. |
| 372 | **Binary to Decimal** | `binary-to-decimal` | ✅ Available | `math-engine` | `base_convert` | Free online binary to decimal utility. This tool is planned and coming soon. |
| 374 | **Binary to Hexadecimal** | `binary-to-hexadecimal` | ✅ Available | `math-engine` | `base_convert` | Free online binary to hexadecimal utility. This tool is planned and coming soon. |
| 100193 | **Block Calculator** | `block-calculator` | ✅ Available | `construction-engine` | `masonry` | Block Calculator utility powered by construction-engine. |
| 334 | **Break-even Calculator** | `break-even-calculator` | ✅ Available | `finance-engine` | `break_even` | Free online break-even calculator utility. This tool is planned and coming soon. |
| 100192 | **Brick Calculator** | `brick-calculator` | ✅ Available | `construction-engine` | `masonry` | Brick Calculator utility powered by construction-engine. |
| 336 | **CAGR Calculator** | `cagr-calculator` | ✅ Available | `finance-engine` | `cagr` | Free online cagr calculator utility. This tool is planned and coming soon. |
| 100264 | **CSV To Geojson** | `csv-to-geojson` | ✅ Available | `geo-engine` | `from_csv` | CSV To Geojson utility powered by geo-engine. |
| 100182 | **Capital Gains Estimator** | `capital-gains-estimator` | ✅ Available | `indiatax-engine` | `capital_gains` | Capital Gains Estimator utility powered by indiatax-engine. |
| 100217 | **Carbon Footprint Estimator** | `carbon-footprint-estimator` | ✅ Available | `solar-engine` | `carbon` | Carbon Footprint Estimator utility powered by solar-engine. |
| 382 | **Celsius to Fahrenheit** | `celsius-to-fahrenheit` | ✅ Available | `units-engine` | `temperature` | Free online celsius to fahrenheit utility. This tool is planned and coming soon. |
| 100188 | **Cement Calculator** | `cement-calculator` | ✅ Available | `construction-engine` | `concrete` | Cement Calculator utility powered by construction-engine. |
| 323 | **Compound Interest Calculator** | `compound-interest-calculator` | ✅ Available | `finance-engine` | `interest` | Free online compound interest calculator utility. This tool is planned and coming soon. |
| 100189 | **Concrete Calculator** | `concrete-calculator` | ✅ Available | `construction-engine` | `concrete` | Concrete Calculator utility powered by construction-engine. |
| 100207 | **Construction Cost Estimator** | `construction-cost-estimator` | ✅ Available | `construction-engine` | `cost_estimate` | Construction Cost Estimator utility powered by construction-engine. |
| 100268 | **Coordinate Converter** | `coordinate-converter` | ✅ Available | `geo-engine` | `convert_coordinate` | Coordinate Converter utility powered by geo-engine. |
| 331 | **Currency Calculator** | `currency-calculator` | ✅ Available | `finance-engine` | `currency` | Free online currency calculator utility. This tool is planned and coming soon. |
| 387 | **DPI Calculator** | `dpi-calculator` | ✅ Available | `units-engine` | `pixels` | Free online dpi calculator utility. This tool is planned and coming soon. |
| 371 | **Data Storage Converter** | `data-storage-converter` | ✅ Available | `units-engine` | `convert` | Free online data storage converter utility. This tool is planned and coming soon. |
| 338 | **Date Difference Calculator** | `date-difference-calculator` | ✅ Available | `datetime-engine` | `difference` | Free online date difference calculator utility. This tool is planned and coming soon. |
| 339 | **Days Between Dates** | `days-between-dates` | ✅ Available | `datetime-engine` | `difference` | Free online days between dates utility. This tool is planned and coming soon. |
| 373 | **Decimal to Binary** | `decimal-to-binary` | ✅ Available | `math-engine` | `base_convert` | Free online decimal to binary utility. This tool is planned and coming soon. |
| 100162 | **Density Converter** | `density-converter` | ✅ Available | `units-engine` | `convert` | Density Converter utility powered by units-engine. |
| 314 | **Discount Calculator** | `discount-calculator` | ✅ Available | `finance-engine` | `discount` | Free online discount calculator utility. This tool is planned and coming soon. |
| 350 | **Distance Calculator** | `distance-calculator` | ✅ Available | `geo-engine` | `distance` | Free online distance calculator utility. This tool is planned and coming soon. |
| 319 | **EMI Calculator** | `emi-calculator` | ✅ Available | `finance-engine` | `loan` | Free online emi calculator utility. This tool is planned and coming soon. |
| 100216 | **Electricity Unit Cost Calculator** | `electricity-unit-cost-calculator` | ✅ Available | `solar-engine` | `bill` | Electricity Unit Cost Calculator utility powered by solar-engine. |
| 369 | **Energy Converter** | `energy-converter` | ✅ Available | `units-engine` | `convert` | Free online energy converter utility. This tool is planned and coming soon. |
| 100202 | **Excavation Calculator** | `excavation-calculator` | ✅ Available | `construction-engine` | `excavation` | Excavation Calculator utility powered by construction-engine. |
| 327 | **FD Calculator** | `fd-calculator` | ✅ Available | `finance-engine` | `fixed_deposit` | Free online fd calculator utility. This tool is planned and coming soon. |
| 378 | **Feet to Meters** | `feet-to-meters` | ✅ Available | `units-engine` | `convert` | Free online feet to meters utility. This tool is planned and coming soon. |
| 100198 | **Flooring Calculator** | `flooring-calculator` | ✅ Available | `construction-engine` | `tiles` | Flooring Calculator utility powered by construction-engine. |
| 100161 | **Force Converter** | `force-converter` | ✅ Available | `units-engine` | `convert` | Force Converter utility powered by units-engine. |
| 344 | **Freelance Hourly Rate Calculator** | `freelance-hourly-rate-calculator` | ✅ Available | `finance-engine` | `freelance_rate` | Free online freelance hourly rate calculator utility. This tool is planned and coming soon. |
| 100163 | **Frequency Converter** | `frequency-converter` | ✅ Available | `units-engine` | `convert` | Frequency Converter utility powered by units-engine. |
| 347 | **Fuel Cost Calculator** | `fuel-cost-calculator` | ✅ Available | `units-engine` | `convert` | Free online fuel cost calculator utility. This tool is planned and coming soon. |
| 315 | **GST Calculator** | `gst-calculator` | ✅ Available | `indiatax-engine` | `gst` | Free online gst calculator utility. This tool is planned and coming soon. |
| 100172 | **GST Inclusive Exclusive Calculator** | `gst-inclusive-exclusive-calculator` | ✅ Available | `indiatax-engine` | `gst` | GST Inclusive Exclusive Calculator utility powered by indiatax-engine. |
| 100173 | **GST Invoice Validator** | `gst-invoice-validator` | ✅ Available | `indiatax-engine` | `validate_invoice` | GST Invoice Validator utility powered by indiatax-engine. |
| 100176 | **GST Rate Calculator** | `gst-rate-calculator` | ✅ Available | `indiatax-engine` | `gst` | GST Rate Calculator utility powered by indiatax-engine. |
| 100272 | **Geohash Encoder Decoder** | `geohash-encoder-decoder` | ✅ Available | `geo-engine` | `geohash` | Geohash Encoder Decoder utility powered by geo-engine. |
| 100271 | **Geojson Bbox** | `geojson-bbox` | ✅ Available | `geo-engine` | `bbox` | Geojson Bbox utility powered by geo-engine. |
| 100261 | **Geojson Formatter** | `geojson-formatter` | ✅ Available | `geo-engine` | `format` | Geojson Formatter utility powered by geo-engine. |
| 100270 | **Geojson Simplifier** | `geojson-simplifier` | ✅ Available | `geo-engine` | `simplify` | Geojson Simplifier utility powered by geo-engine. |
| 100263 | **Geojson To CSV** | `geojson-to-csv` | ✅ Available | `geo-engine` | `to_csv` | Geojson To CSV utility powered by geo-engine. |
| 100265 | **Geojson To KML** | `geojson-to-kml` | ✅ Available | `geo-engine` | `to_kml` | Geojson To KML utility powered by geo-engine. |
| 100262 | **Geojson Validator** | `geojson-validator` | ✅ Available | `geo-engine` | `validate` | Geojson Validator utility powered by geo-engine. |
| 100174 | **Gstin Format Validator** | `gstin-format-validator` | ✅ Available | `indiatax-engine` | `validate_gstin` | Gstin Format Validator utility powered by indiatax-engine. |
| 100178 | **Gstr Data Validator** | `gstr-data-validator` | ✅ Available | `indiatax-engine` | `validate_gstr` | Gstr Data Validator utility powered by indiatax-engine. |
| 375 | **Hexadecimal to Decimal** | `hexadecimal-to-decimal` | ✅ Available | `math-engine` | `base_convert` | Free online hexadecimal to decimal utility. This tool is planned and coming soon. |
| 100170 | **Hourly To Salary Calculator** | `hourly-to-salary-calculator` | ✅ Available | `finance-engine` | `hourly` | Hourly To Salary Calculator utility powered by finance-engine. |
| 100175 | **Hsn Sac Lookup** | `hsn-sac-lookup` | ✅ Available | `indiatax-engine` | `hsn_lookup` | Hsn Sac Lookup utility powered by indiatax-engine. |
| 100168 | **IRR Calculator** | `irr-calculator` | ✅ Available | `finance-engine` | `irr` | IRR Calculator utility powered by finance-engine. |
| 379 | **Inches to Centimeters** | `inches-to-centimeters` | ✅ Available | `units-engine` | `convert` | Free online inches to centimeters utility. This tool is planned and coming soon. |
| 316 | **Income Tax Calculator** | `income-tax-calculator` | ✅ Available | `indiatax-engine` | `income_tax` | Free online income tax calculator utility. This tool is planned and coming soon. |
| 330 | **Inflation Calculator** | `inflation-calculator` | ✅ Available | `finance-engine` | `inflation` | Free online inflation calculator utility. This tool is planned and coming soon. |
| 100177 | **Input Tax Credit Calculator** | `input-tax-credit-calculator` | ✅ Available | `indiatax-engine` | `itc` | Input Tax Credit Calculator utility powered by indiatax-engine. |
| 322 | **Interest Calculator** | `interest-calculator` | ✅ Available | `finance-engine` | `interest` | Free online interest calculator utility. This tool is planned and coming soon. |
| 100213 | **Inverter Sizing Calculator** | `inverter-sizing-calculator` | ✅ Available | `solar-engine` | `inverter` | Inverter Sizing Calculator utility powered by solar-engine. |
| 100179 | **Invoice Number Validator** | `invoice-number-validator` | ✅ Available | `indiatax-engine` | `validate_invoice_number` | Invoice Number Validator utility powered by indiatax-engine. |
| 381 | **KG to Pounds** | `kg-to-pounds` | ✅ Available | `units-engine` | `convert` | Free online kg to pounds utility. This tool is planned and coming soon. |
| 100266 | **KML To Geojson** | `kml-to-geojson` | ✅ Available | `geo-engine` | `from_kml` | KML To Geojson utility powered by geo-engine. |
| 361 | **Length Converter** | `length-converter` | ✅ Available | `units-engine` | `convert` | Free online length converter utility. This tool is planned and coming soon. |
| 383 | **Liters to Gallons** | `liters-to-gallons` | ✅ Available | `units-engine` | `convert` | Free online liters to gallons utility. This tool is planned and coming soon. |
| 320 | **Loan Calculator** | `loan-calculator` | ✅ Available | `finance-engine` | `loan` | Free online loan calculator utility. This tool is planned and coming soon. |
| 100167 | **Lumpsum Calculator** | `lumpsum-calculator` | ✅ Available | `finance-engine` | `investment` | Lumpsum Calculator utility powered by finance-engine. |
| 384 | **MB to GB** | `mb-to-gb` | ✅ Available | `units-engine` | `convert` | Free online mb to gb utility. This tool is planned and coming soon. |
| 333 | **Markup Calculator** | `markup-calculator` | ✅ Available | `finance-engine` | `markup` | Free online markup calculator utility. This tool is planned and coming soon. |
| 385 | **Mbps to MB/s** | `mbps-to-mb-s` | ✅ Available | `units-engine` | `convert` | Free online mbps to mb/s utility. This tool is planned and coming soon. |
| 348 | **Mileage Calculator** | `mileage-calculator` | ✅ Available | `units-engine` | `convert` | Free online mileage calculator utility. This tool is planned and coming soon. |
| 380 | **Miles to Kilometers** | `miles-to-kilometers` | ✅ Available | `units-engine` | `convert` | Free online miles to kilometers utility. This tool is planned and coming soon. |
| 100194 | **Mortar Calculator** | `mortar-calculator` | ✅ Available | `construction-engine` | `masonry` | Mortar Calculator utility powered by construction-engine. |
| 321 | **Mortgage Calculator** | `mortgage-calculator` | ✅ Available | `finance-engine` | `mortgage` | Free online mortgage calculator utility. This tool is planned and coming soon. |
| 326 | **Mutual Fund Return Calculator** | `mutual-fund-return-calculator` | ✅ Available | `finance-engine` | `investment` | Free online mutual fund return calculator utility. This tool is planned and coming soon. |
| 100169 | **NPV Calculator** | `npv-calculator` | ✅ Available | `finance-engine` | `npv` | NPV Calculator utility powered by finance-engine. |
| 376 | **Octal Converter** | `octal-converter` | ✅ Available | `math-engine` | `base_convert` | Free online octal converter utility. This tool is planned and coming soon. |
| 343 | **Overtime Calculator** | `overtime-calculator` | ✅ Available | `finance-engine` | `overtime` | Free online overtime calculator utility. This tool is planned and coming soon. |
| 329 | **PPF Calculator** | `ppf-calculator` | ✅ Available | `finance-engine` | `ppf` | Free online ppf calculator utility. This tool is planned and coming soon. |
| 100196 | **Paint Calculator** | `paint-calculator` | ✅ Available | `construction-engine` | `paint` | Paint Calculator utility powered by construction-engine. |
| 100166 | **Paper Size Converter** | `paper-size-converter` | ✅ Available | `units-engine` | `paper_size` | Paper Size Converter utility powered by units-engine. |
| 100205 | **Paver Calculator** | `paver-calculator` | ✅ Available | `construction-engine` | `tiles` | Paver Calculator utility powered by construction-engine. |
| 313 | **Percentage Calculator** | `percentage-calculator` | ✅ Available | `math-engine` | `percentage` | Free online percentage calculator utility. This tool is planned and coming soon. |
| 386 | **Pixels to Inches** | `pixels-to-inches` | ✅ Available | `units-engine` | `pixels` | Free online pixels to inches utility. This tool is planned and coming soon. |
| 100195 | **Plaster Calculator** | `plaster-calculator` | ✅ Available | `construction-engine` | `plaster` | Plaster Calculator utility powered by construction-engine. |
| 370 | **Power Converter** | `power-converter` | ✅ Available | `units-engine` | `convert` | Free online power converter utility. This tool is planned and coming soon. |
| 368 | **Pressure Converter** | `pressure-converter` | ✅ Available | `units-engine` | `convert` | Free online pressure converter utility. This tool is planned and coming soon. |
| 100181 | **Professional Tax Calculator** | `professional-tax-calculator` | ✅ Available | `indiatax-engine` | `professional_tax` | Professional Tax Calculator utility powered by indiatax-engine. |
| 332 | **Profit Margin Calculator** | `profit-margin-calculator` | ✅ Available | `finance-engine` | `margin` | Free online profit margin calculator utility. This tool is planned and coming soon. |
| 100186 | **Purchase Register Reconciliation** | `purchase-register-reconciliation` | ✅ Available | `indiatax-engine` | `reconcile` | Purchase Register Reconciliation utility powered by indiatax-engine. |
| 328 | **RD Calculator** | `rd-calculator` | ✅ Available | `finance-engine` | `recurring_deposit` | Free online rd calculator utility. This tool is planned and coming soon. |
| 335 | **ROI Calculator** | `roi-calculator` | ✅ Available | `finance-engine` | `roi` | Free online roi calculator utility. This tool is planned and coming soon. |
| 100201 | **Rebar Calculator** | `rebar-calculator` | ✅ Available | `construction-engine` | `rebar` | Rebar Calculator utility powered by construction-engine. |
| 377 | **Roman Numeral Converter** | `roman-numeral-converter` | ✅ Available | `math-engine` | `roman` | Free online roman numeral converter utility. This tool is planned and coming soon. |
| 100199 | **Roofing Calculator** | `roofing-calculator` | ✅ Available | `construction-engine` | `roofing` | Roofing Calculator utility powered by construction-engine. |
| 325 | **SIP Calculator** | `sip-calculator` | ✅ Available | `finance-engine` | `investment` | Free online sip calculator utility. This tool is planned and coming soon. |
| 317 | **Salary Calculator** | `salary-calculator` | ✅ Available | `finance-engine` | `salary` | Free online salary calculator utility. This tool is planned and coming soon. |
| 342 | **Salary Hike Calculator** | `salary-hike-calculator` | ✅ Available | `finance-engine` | `salary_hike` | Free online salary hike calculator utility. This tool is planned and coming soon. |
| 100187 | **Sales Register Reconciliation** | `sales-register-reconciliation` | ✅ Available | `indiatax-engine` | `reconcile` | Sales Register Reconciliation utility powered by indiatax-engine. |
| 100190 | **Sand Calculator** | `sand-calculator` | ✅ Available | `construction-engine` | `concrete` | Sand Calculator utility powered by construction-engine. |
| 312 | **Scientific Calculator** | `scientific-calculator` | ✅ Available | `math-engine` | `evaluate` | Free online scientific calculator utility. This tool is planned and coming soon. |
| 389 | **Screen Resolution Calculator** | `screen-resolution-calculator` | ✅ Available | `units-engine` | `pixels` | Free online screen resolution calculator utility. This tool is planned and coming soon. |
| 100165 | **Shoe Size Converter** | `shoe-size-converter` | ✅ Available | `units-engine` | `shoe_size` | Shoe Size Converter utility powered by units-engine. |
| 324 | **Simple Interest Calculator** | `simple-interest-calculator` | ✅ Available | `finance-engine` | `interest` | Free online simple interest calculator utility. This tool is planned and coming soon. |
| 100212 | **Solar Battery Sizing** | `solar-battery-sizing` | ✅ Available | `solar-engine` | `battery` | Solar Battery Sizing utility powered by solar-engine. |
| 100208 | **Solar Panel Size Calculator** | `solar-panel-size-calculator` | ✅ Available | `solar-engine` | `panel_size` | Solar Panel Size Calculator utility powered by solar-engine. |
| 100209 | **Solar ROI Calculator** | `solar-roi-calculator` | ✅ Available | `solar-engine` | `roi` | Solar ROI Calculator utility powered by solar-engine. |
| 100211 | **Solar Roof Area Calculator** | `solar-roof-area-calculator` | ✅ Available | `solar-engine` | `panel_size` | Solar Roof Area Calculator utility powered by solar-engine. |
| 100210 | **Solar Savings Calculator** | `solar-savings-calculator` | ✅ Available | `solar-engine` | `roi` | Solar Savings Calculator utility powered by solar-engine. |
| 349 | **Speed Calculator** | `speed-calculator` | ✅ Available | `units-engine` | `convert` | Free online speed calculator utility. This tool is planned and coming soon. |
| 366 | **Speed Converter** | `speed-converter` | ✅ Available | `units-engine` | `convert` | Free online speed converter utility. This tool is planned and coming soon. |
| 346 | **Split Bill Calculator** | `split-bill-calculator` | ✅ Available | `finance-engine` | `tip` | Free online split bill calculator utility. This tool is planned and coming soon. |
| 100204 | **Staircase Calculator** | `staircase-calculator` | ✅ Available | `construction-engine` | `staircase` | Staircase Calculator utility powered by construction-engine. |
| 100200 | **Steel Weight Calculator** | `steel-weight-calculator` | ✅ Available | `construction-engine` | `steel` | Steel Weight Calculator utility powered by construction-engine. |
| 100180 | **TDS Calculator** | `tds-calculator` | ✅ Available | `indiatax-engine` | `tds` | TDS Calculator utility powered by indiatax-engine. |
| 318 | **Take-home Salary Calculator** | `take-home-salary-calculator` | ✅ Available | `indiatax-engine` | `salary_in_hand` | Free online take-home salary calculator utility. This tool is planned and coming soon. |
| 100171 | **Tax Bracket Calculator** | `tax-bracket-calculator` | ✅ Available | `finance-engine` | `tax_brackets` | Tax Bracket Calculator utility powered by finance-engine. |
| 363 | **Temperature Converter** | `temperature-converter` | ✅ Available | `units-engine` | `temperature` | Free online temperature converter utility. This tool is planned and coming soon. |
| 100197 | **Tile Calculator** | `tile-calculator` | ✅ Available | `construction-engine` | `tiles` | Tile Calculator utility powered by construction-engine. |
| 340 | **Time Calculator** | `time-calculator` | ✅ Available | `datetime-engine` | `add` | Free online time calculator utility. This tool is planned and coming soon. |
| 367 | **Time Converter** | `time-converter` | ✅ Available | `units-engine` | `convert` | Free online time converter utility. This tool is planned and coming soon. |
| 390 | **Time Zone Converter** | `time-zone-converter` | ✅ Available | `datetime-engine` | `convert_zone` | Free online time zone converter utility. This tool is planned and coming soon. |
| 345 | **Tip Calculator** | `tip-calculator` | ✅ Available | `finance-engine` | `tip` | Free online tip calculator utility. This tool is planned and coming soon. |
| 100164 | **Torque Converter** | `torque-converter` | ✅ Available | `units-engine` | `convert` | Torque Converter utility powered by units-engine. |
| 100273 | **UTM Latlon Converter** | `utm-latlon-converter` | ✅ Available | `geo-engine` | `utm` | UTM Latlon Converter utility powered by geo-engine. |
| 365 | **Volume Converter** | `volume-converter` | ✅ Available | `units-engine` | `convert` | Free online volume converter utility. This tool is planned and coming soon. |
| 100267 | **WKT Viewer** | `wkt-viewer` | ✅ Available | `geo-engine` | `wkt_parse` | WKT Viewer utility powered by geo-engine. |
| 100203 | **Wall Area Calculator** | `wall-area-calculator` | ✅ Available | `construction-engine` | `wall_area` | Wall Area Calculator utility powered by construction-engine. |
| 100206 | **Water Tank Capacity Calculator** | `water-tank-capacity-calculator` | ✅ Available | `construction-engine` | `tank` | Water Tank Capacity Calculator utility powered by construction-engine. |
| 362 | **Weight Converter** | `weight-converter` | ✅ Available | `units-engine` | `convert` | Free online weight converter utility. This tool is planned and coming soon. |
| 341 | **Working Days Calculator** | `working-days-calculator` | ✅ Available | `datetime-engine` | `business_days` | Free online working days calculator utility. This tool is planned and coming soon. |
| 351 | **BMI Calculator** | `bmi-calculator` | ⏳ Coming Soon | — | — | Free online bmi calculator utility. This tool is planned and coming soon. |
| 352 | **BMR Calculator** | `bmr-calculator` | ⏳ Coming Soon | — | — | Free online bmr calculator utility. This tool is planned and coming soon. |
| 356 | **Body Fat Calculator** | `body-fat-calculator` | ⏳ Coming Soon | — | — | Free online body fat calculator utility. This tool is planned and coming soon. |
| 353 | **Calorie Calculator** | `calorie-calculator` | ⏳ Coming Soon | — | — | Free online calorie calculator utility. This tool is planned and coming soon. |
| 357 | **Ideal Weight Calculator** | `ideal-weight-calculator` | ⏳ Coming Soon | — | — | Free online ideal weight calculator utility. This tool is planned and coming soon. |
| 354 | **Macro Calculator** | `macro-calculator` | ⏳ Coming Soon | — | — | Free online macro calculator utility. This tool is planned and coming soon. |
| 360 | **Ovulation Calculator** | `ovulation-calculator` | ⏳ Coming Soon | — | — | Free online ovulation calculator utility. This tool is planned and coming soon. |
| 359 | **Pregnancy Due Date Calculator** | `pregnancy-due-date-calculator` | ⏳ Coming Soon | — | — | Free online pregnancy due date calculator utility. This tool is planned and coming soon. |
| 355 | **Protein Intake Calculator** | `protein-intake-calculator` | ⏳ Coming Soon | — | — | Free online protein intake calculator utility. This tool is planned and coming soon. |
| 358 | **Water Intake Calculator** | `water-intake-calculator` | ⏳ Coming Soon | — | — | Free online water intake calculator utility. This tool is planned and coming soon. |

### Creative Tools (25 tools — 1 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 705 | **Barcode Reader** | `barcode-reader` | ✅ Available | `qrbarcode-engine` | `barcode_read` | Free online barcode reader utility. This tool is planned and coming soon. |
| 704 | **Browser-based OCR** | `browser-based-ocr` | ⏳ Coming Soon | — | — | Free online browser-based ocr utility. This tool is planned and coming soon. |
| 696 | **CSS Playground** | `css-playground` | ⏳ Coming Soon | — | — | Free online css playground utility. This tool is planned and coming soon. |
| 694 | **Code Editor** | `code-editor` | ⏳ Coming Soon | — | — | Free online code editor utility. This tool is planned and coming soon. |
| 689 | **Diagram Editor** | `diagram-editor` | ⏳ Coming Soon | — | — | Free online diagram editor utility. This tool is planned and coming soon. |
| 692 | **Document Editor** | `document-editor` | ⏳ Coming Soon | — | — | Free online document editor utility. This tool is planned and coming soon. |
| 710 | **File Previewer** | `file-previewer` | ⏳ Coming Soon | — | — | Free online file previewer utility. This tool is planned and coming soon. |
| 688 | **Flowchart Maker** | `flowchart-maker` | ⏳ Coming Soon | — | — | Free online flowchart maker utility. This tool is planned and coming soon. |
| 695 | **HTML Playground** | `html-playground` | ⏳ Coming Soon | — | — | Free online html playground utility. This tool is planned and coming soon. |
| 701 | **Image Annotation Tool** | `image-annotation-tool` | ⏳ Coming Soon | — | — | Free online image annotation tool utility. This tool is planned and coming soon. |
| 697 | **JavaScript Playground** | `javascript-playground` | ⏳ Coming Soon | — | — | Free online javascript playground utility. This tool is planned and coming soon. |
| 687 | **Mind Map Maker** | `mind-map-maker` | ⏳ Coming Soon | — | — | Free online mind map maker utility. This tool is planned and coming soon. |
| 690 | **Presentation Editor** | `presentation-editor` | ⏳ Coming Soon | — | — | Free online presentation editor utility. This tool is planned and coming soon. |
| 699 | **Python Playground** | `python-playground` | ⏳ Coming Soon | — | — | Free online python playground utility. This tool is planned and coming soon. |
| 693 | **Rich Text Editor** | `rich-text-editor` | ⏳ Coming Soon | — | — | Free online rich text editor utility. This tool is planned and coming soon. |
| 700 | **Rust Playground** | `rust-playground` | ⏳ Coming Soon | — | — | Free online rust playground utility. This tool is planned and coming soon. |
| 698 | **SQL Playground** | `sql-playground` | ⏳ Coming Soon | — | — | Free online sql playground utility. This tool is planned and coming soon. |
| 707 | **Screen Color Picker** | `screen-color-picker` | ⏳ Coming Soon | — | — | Free online screen color picker utility. This tool is planned and coming soon. |
| 708 | **Screen Ruler** | `screen-ruler` | ⏳ Coming Soon | — | — | Free online screen ruler utility. This tool is planned and coming soon. |
| 703 | **Screenshot Capture Tool** | `screenshot-capture-tool` | ⏳ Coming Soon | — | — | Free online screenshot capture tool utility. This tool is planned and coming soon. |
| 702 | **Screenshot Editor** | `screenshot-editor` | ⏳ Coming Soon | — | — | Free online screenshot editor utility. This tool is planned and coming soon. |
| 691 | **Spreadsheet Editor** | `spreadsheet-editor` | ⏳ Coming Soon | — | — | Free online spreadsheet editor utility. This tool is planned and coming soon. |
| 709 | **Virtual Keyboard** | `virtual-keyboard` | ⏳ Coming Soon | — | — | Free online virtual keyboard utility. This tool is planned and coming soon. |
| 706 | **Webcam Image Capture** | `webcam-image-capture` | ⏳ Coming Soon | — | — | Free online webcam image capture utility. This tool is planned and coming soon. |
| 686 | **Whiteboard** | `whiteboard` | ⏳ Coming Soon | — | — | Free online whiteboard utility. This tool is planned and coming soon. |

### Design Tools (57 tools — 50 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 100260 | **3d Model Thumbnail** | `3d-model-thumbnail` | ✅ Available | `mesh3d-engine` | `thumbnail` | 3d Model Thumbnail utility powered by mesh3d-engine. |
| 100224 | **Accessible Color Pairing** | `accessible-color-pairing` | ✅ Available | `colordesign-engine` | `accessible_pairs` | Accessible Color Pairing utility powered by colordesign-engine. |
| 510 | **Analogous Color Finder** | `analogous-color-finder` | ✅ Available | `colordesign-engine` | `palette` | Free online analogous color finder utility. This tool is planned and coming soon. |
| 517 | **App Icon Generator** | `app-icon-generator` | ✅ Available | `image-engine` | `favicon` | Free online app icon generator utility. This tool is planned and coming soon. |
| 524 | **Border Radius Generator** | `border-radius-generator` | ✅ Available | `colordesign-engine` | `border_radius` | Free online border radius generator utility. This tool is planned and coming soon. |
| 523 | **Box Shadow Generator** | `box-shadow-generator` | ✅ Available | `colordesign-engine` | `shadow` | Free online box shadow generator utility. This tool is planned and coming soon. |
| 505 | **CMYK Converter** | `cmyk-converter` | ✅ Available | `colordesign-engine` | `convert` | Free online cmyk converter utility. This tool is planned and coming soon. |
| 100226 | **CSS Unit Converter** | `css-unit-converter` | ✅ Available | `colordesign-engine` | `css_units` | CSS Unit Converter utility powered by colordesign-engine. |
| 100225 | **Color Blindness Simulator** | `color-blindness-simulator` | ✅ Available | `colordesign-engine` | `simulate_color_blindness` | Color Blindness Simulator utility powered by colordesign-engine. |
| 508 | **Color Palette Generator** | `color-palette-generator` | ✅ Available | `colordesign-engine` | `palette` | Free online color palette generator utility. This tool is planned and coming soon. |
| 501 | **Color Picker** | `color-picker` | ✅ Available | `colordesign-engine` | `convert` | Free online color picker utility. This tool is planned and coming soon. |
| 509 | **Complementary Color Finder** | `complementary-color-finder` | ✅ Available | `colordesign-engine` | `palette` | Free online complementary color finder utility. This tool is planned and coming soon. |
| 511 | **Contrast Checker** | `contrast-checker` | ✅ Available | `colordesign-engine` | `contrast` | Free online contrast checker utility. This tool is planned and coming soon. |
| 516 | **Favicon Generator** | `favicon-generator` | ✅ Available | `image-engine` | `favicon` | Free online favicon generator utility. This tool is planned and coming soon. |
| 100229 | **Favicon Preview** | `favicon-preview` | ✅ Available | `colordesign-engine` | `favicon_preview` | Favicon Preview utility powered by colordesign-engine. |
| 100242 | **Font Character Map** | `font-character-map` | ✅ Available | `font-engine` | `char_map` | Font Character Map utility powered by font-engine. |
| 100237 | **Font Converter** | `font-converter` | ✅ Available | `font-engine` | `convert` | Font Converter utility powered by font-engine. |
| 100236 | **Font Metadata Viewer** | `font-metadata-viewer` | ✅ Available | `font-engine` | `inspect` | Font Metadata Viewer utility powered by font-engine. |
| 513 | **Font Pairing Generator** | `font-pairing-generator` | ✅ Available | `font-engine` | `pairing` | Free online font pairing generator utility. This tool is planned and coming soon. |
| 514 | **Font Previewer** | `font-previewer` | ✅ Available | `font-engine` | `preview` | Free online font previewer utility. This tool is planned and coming soon. |
| 100243 | **Font Size Checker** | `font-size-checker` | ✅ Available | `font-engine` | `size_check` | Font Size Checker utility powered by font-engine. |
| 100241 | **Font Subsetter** | `font-subsetter` | ✅ Available | `font-engine` | `subset` | Font Subsetter utility powered by font-engine. |
| 100253 | **GLB To Gltf** | `glb-to-gltf` | ✅ Available | `mesh3d-engine` | `glb_to_gltf` | GLB To Gltf utility powered by mesh3d-engine. |
| 521 | **Glassmorphism Generator** | `glassmorphism-generator` | ✅ Available | `colordesign-engine` | `shadow` | Free online glassmorphism generator utility. This tool is planned and coming soon. |
| 100254 | **Gltf Optimizer** | `gltf-optimizer` | ✅ Available | `mesh3d-engine` | `gltf_optimize` | Gltf Optimizer utility powered by mesh3d-engine. |
| 100252 | **Gltf Viewer** | `gltf-viewer` | ✅ Available | `mesh3d-engine` | `gltf_inspect` | Gltf Viewer utility powered by mesh3d-engine. |
| 506 | **Gradient Generator** | `gradient-generator` | ✅ Available | `colordesign-engine` | `gradient` | Free online gradient generator utility. This tool is planned and coming soon. |
| 507 | **Gradient Palette Generator** | `gradient-palette-generator` | ✅ Available | `colordesign-engine` | `gradient` | Free online gradient palette generator utility. This tool is planned and coming soon. |
| 502 | **HEX Color Converter** | `hex-color-converter` | ✅ Available | `colordesign-engine` | `convert` | Free online hex color converter utility. This tool is planned and coming soon. |
| 504 | **HSL Color Converter** | `hsl-color-converter` | ✅ Available | `colordesign-engine` | `convert` | Free online hsl color converter utility. This tool is planned and coming soon. |
| 526 | **Instagram Story Resizer** | `instagram-story-resizer` | ✅ Available | `colordesign-engine` | `social_dimensions` | Free online instagram story resizer utility. This tool is planned and coming soon. |
| 100255 | **Mesh Inspector** | `mesh-inspector` | ✅ Available | `mesh3d-engine` | `inspect` | Mesh Inspector utility powered by mesh3d-engine. |
| 100259 | **Mesh Repair Basic** | `mesh-repair-basic` | ✅ Available | `mesh3d-engine` | `repair` | Mesh Repair Basic utility powered by mesh3d-engine. |
| 100258 | **Mesh Rotate** | `mesh-rotate` | ✅ Available | `mesh3d-engine` | `transform` | Mesh Rotate utility powered by mesh3d-engine. |
| 100257 | **Mesh Scale** | `mesh-scale` | ✅ Available | `mesh3d-engine` | `transform` | Mesh Scale utility powered by mesh3d-engine. |
| 100256 | **Mesh Volume Calculator** | `mesh-volume-calculator` | ✅ Available | `mesh3d-engine` | `inspect` | Mesh Volume Calculator utility powered by mesh3d-engine. |
| 522 | **Neumorphism Generator** | `neumorphism-generator` | ✅ Available | `colordesign-engine` | `shadow` | Free online neumorphism generator utility. This tool is planned and coming soon. |
| 100251 | **OBJ To STL** | `obj-to-stl` | ✅ Available | `mesh3d-engine` | `convert` | OBJ To STL utility powered by mesh3d-engine. |
| 100240 | **OTF Inspector** | `otf-inspector` | ✅ Available | `font-engine` | `inspect` | OTF Inspector utility powered by font-engine. |
| 528 | **Open Graph Image Generator** | `open-graph-image-generator` | ✅ Available | `image-engine` | `og_image` | Free online open graph image generator utility. This tool is planned and coming soon. |
| 503 | **RGB Color Converter** | `rgb-color-converter` | ✅ Available | `colordesign-engine` | `convert` | Free online rgb color converter utility. This tool is planned and coming soon. |
| 100223 | **Rgb To Hsl** | `rgb-to-hsl` | ✅ Available | `colordesign-engine` | `convert` | Rgb To Hsl utility powered by colordesign-engine. |
| 100250 | **STL To OBJ** | `stl-to-obj` | ✅ Available | `mesh3d-engine` | `convert` | STL To OBJ utility powered by mesh3d-engine. |
| 100249 | **STL Viewer** | `stl-viewer` | ✅ Available | `mesh3d-engine` | `thumbnail` | STL Viewer utility powered by mesh3d-engine. |
| 525 | **Social Media Post Resizer** | `social-media-post-resizer` | ✅ Available | `colordesign-engine` | `social_dimensions` | Free online social media post resizer utility. This tool is planned and coming soon. |
| 100227 | **Spacing Scale Generator** | `spacing-scale-generator` | ✅ Available | `colordesign-engine` | `spacing_scale` | Spacing Scale Generator utility powered by colordesign-engine. |
| 100239 | **TTF To Woff** | `ttf-to-woff` | ✅ Available | `font-engine` | `convert` | TTF To Woff utility powered by font-engine. |
| 100228 | **Typography Scale Generator** | `typography-scale-generator` | ✅ Available | `colordesign-engine` | `type_scale` | Typography Scale Generator utility powered by colordesign-engine. |
| 512 | **WCAG Accessibility Checker** | `wcag-accessibility-checker` | ✅ Available | `colordesign-engine` | `contrast` | Free online wcag accessibility checker utility. This tool is planned and coming soon. |
| 100238 | **Woff To TTF** | `woff-to-ttf` | ✅ Available | `font-engine` | `convert` | Woff To TTF utility powered by font-engine. |
| 530 | **CSS Flexbox Playground** | `css-flexbox-playground` | ⏳ Coming Soon | — | — | Free online css flexbox playground utility. This tool is planned and coming soon. |
| 529 | **CSS Grid Generator** | `css-grid-generator` | ⏳ Coming Soon | — | — | Free online css grid generator utility. This tool is planned and coming soon. |
| 520 | **CSS Pattern Generator** | `css-pattern-generator` | ⏳ Coming Soon | — | — | Free online css pattern generator utility. This tool is planned and coming soon. |
| 515 | **Google Fonts Previewer** | `google-fonts-previewer` | ⏳ Coming Soon | — | — | Free online google fonts previewer utility. This tool is planned and coming soon. |
| 518 | **Logo Maker** | `logo-maker` | ⏳ Coming Soon | — | — | Free online logo maker utility. This tool is planned and coming soon. |
| 519 | **SVG Icon Library** | `svg-icon-library` | ⏳ Coming Soon | — | — | Free online svg icon library utility. This tool is planned and coming soon. |
| 527 | **YouTube Thumbnail Resizer** | `youtube-thumbnail-resizer` | ⏳ Coming Soon | — | — | Free online youtube thumbnail resizer utility. This tool is planned and coming soon. |

### Developer Tools (159 tools — 141 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 650 | **.env Editor** | `env-editor` | ✅ Available | `developer-engine` | `env_parse` | Free online .env editor utility. This tool is planned and coming soon. |
| 306 | **Barcode Generator** | `barcode-generator` | ✅ Available | `qrbarcode-engine` | `barcode_generate` | Free online barcode generator utility. This tool is planned and coming soon. |
| 273 | **Base64 Decoder** | `base64-decoder` | ✅ Available | `encoding-engine` | `base64_decode` | Free online base64 decoder utility. This tool is planned and coming soon. |
| 100122 | **Base64 Encode Decode** | `base64-encode-decode` | ✅ Available | `encoding-engine` | `base64_encode` | Base64 Encode Decode utility powered by encoding-engine. |
| 272 | **Base64 Encoder** | `base64-encoder` | ✅ Available | `encoding-engine` | `base64_encode` | Free online base64 encoder utility. This tool is planned and coming soon. |
| 299 | **CSS Border Radius Generator** | `css-border-radius-generator` | ✅ Available | `colordesign-engine` | `border_radius` | Free online css border radius generator utility. This tool is planned and coming soon. |
| 298 | **CSS Box Shadow Generator** | `css-box-shadow-generator` | ✅ Available | `colordesign-engine` | `shadow` | Free online css box shadow generator utility. This tool is planned and coming soon. |
| 260 | **CSS Formatter** | `css-formatter` | ✅ Available | `developer-engine` | `css_format` | Free online css formatter utility. This tool is planned and coming soon. |
| 297 | **CSS Gradient Generator** | `css-gradient-generator` | ✅ Available | `colordesign-engine` | `gradient` | Free online css gradient generator utility. This tool is planned and coming soon. |
| 261 | **CSS Minifier** | `css-minifier` | ✅ Available | `developer-engine` | `css_minify` | Free online css minifier utility. This tool is planned and coming soon. |
| 100298 | **CSV Chart Generator** | `csv-chart-generator` | ✅ Available | `dataviz-engine` | `chart` | CSV Chart Generator utility powered by dataviz-engine. |
| 253 | **CSV to JSON** | `csv-to-json` | ✅ Available | `json-engine` | `from_csv` | Free online csv to json utility. This tool is planned and coming soon. |
| 653 | **CSV to SQL Converter** | `csv-to-sql-converter` | ✅ Available | `csv-engine` | `to_sql` | Free online csv to sql converter utility. This tool is planned and coming soon. |
| 100128 | **Cbor Viewer** | `cbor-viewer` | ✅ Available | `encoding-engine` | `cbor_decode` | Cbor Viewer utility powered by encoding-engine. |
| 100136 | **Chmod Calculator** | `chmod-calculator` | ✅ Available | `developer-engine` | `chmod` | Chmod Calculator utility powered by developer-engine. |
| 100144 | **Code Beautifier** | `code-beautifier` | ✅ Available | `developer-engine` | `code_format` | Code Beautifier utility powered by developer-engine. |
| 100141 | **Code Complexity Analyzer** | `code-complexity-analyzer` | ✅ Available | `developer-engine` | `code_complexity` | Code Complexity Analyzer utility powered by developer-engine. |
| 100145 | **Code Diff** | `code-diff` | ✅ Available | `developer-engine` | `diff` | Code Diff utility powered by developer-engine. |
| 100146 | **Code Duplicate Detector** | `code-duplicate-detector` | ✅ Available | `developer-engine` | `code_duplicates` | Code Duplicate Detector utility powered by developer-engine. |
| 100142 | **Code Line Counter** | `code-line-counter` | ✅ Available | `developer-engine` | `code_line_count` | Code Line Counter utility powered by developer-engine. |
| 100143 | **Code Minifier** | `code-minifier` | ✅ Available | `developer-engine` | `code_minify` | Code Minifier utility powered by developer-engine. |
| 293 | **Color Code Converter** | `color-code-converter` | ✅ Available | `developer-engine` | `color_convert` | Free online color code converter utility. This tool is planned and coming soon. |
| 290 | **Cron Expression Generator** | `cron-expression-generator` | ✅ Available | `developer-engine` | `cron_generate` | Free online cron expression generator utility. This tool is planned and coming soon. |
| 289 | **Cron Expression Parser** | `cron-expression-parser` | ✅ Available | `developer-engine` | `cron_parse` | Free online cron expression parser utility. This tool is planned and coming soon. |
| 100303 | **Data Profiler** | `data-profiler` | ✅ Available | `dataviz-engine` | `profile` | Data Profiler utility powered by dataviz-engine. |
| 100306 | **Data Sampler** | `data-sampler` | ✅ Available | `dataviz-engine` | `sample` | Data Sampler utility powered by dataviz-engine. |
| 100149 | **Dependency License Scanner** | `dependency-license-scanner` | ✅ Available | `developer-engine` | `dependency_scan` | Dependency License Scanner utility powered by developer-engine. |
| 100304 | **Descriptive Statistics** | `descriptive-statistics` | ✅ Available | `dataviz-engine` | `describe` | Descriptive Statistics utility powered by dataviz-engine. |
| 307 | **Diff Viewer** | `diff-viewer` | ✅ Available | `developer-engine` | `diff` | Free online diff viewer utility. This tool is planned and coming soon. |
| 100134 | **Docker Compose Validator** | `docker-compose-validator` | ✅ Available | `developer-engine` | `validate_compose` | Docker Compose Validator utility powered by developer-engine. |
| 309 | **Dockerfile Formatter** | `dockerfile-formatter` | ✅ Available | `developer-engine` | `lint_dockerfile` | Free online dockerfile formatter utility. This tool is planned and coming soon. |
| 633 | **ER Diagram Generator** | `er-diagram-generator` | ✅ Available | `developer-engine` | `sql_erd` | Free online er diagram generator utility. This tool is planned and coming soon. |
| 649 | **Environment Variable Formatter** | `environment-variable-formatter` | ✅ Available | `developer-engine` | `env_parse` | Free online environment variable formatter utility. This tool is planned and coming soon. |
| 292 | **Epoch Converter** | `epoch-converter` | ✅ Available | `datetime-engine` | `timestamp` | Free online epoch converter utility. This tool is planned and coming soon. |
| 100301 | **Frequency Distribution** | `frequency-distribution` | ✅ Available | `dataviz-engine` | `frequency` | Frequency Distribution utility powered by dataviz-engine. |
| 308 | **Gitignore Generator** | `gitignore-generator` | ✅ Available | `developer-engine` | `gitignore` | Free online gitignore generator utility. This tool is planned and coming soon. |
| 639 | **GraphQL Query Formatter** | `graphql-query-formatter` | ✅ Available | `developer-engine` | `graphql_format` | Free online graphql query formatter utility. This tool is planned and coming soon. |
| 100131 | **Graphql Schema Viewer** | `graphql-schema-viewer` | ✅ Available | `developer-engine` | `graphql_schema` | Graphql Schema Viewer utility powered by developer-engine. |
| 100130 | **Graphql Validator** | `graphql-validator` | ✅ Available | `developer-engine` | `graphql_validate` | Graphql Validator utility powered by developer-engine. |
| 295 | **HEX to RGB** | `hex-to-rgb` | ✅ Available | `colordesign-engine` | `convert` | Free online hex to rgb utility. This tool is planned and coming soon. |
| 296 | **HSL Converter** | `hsl-converter` | ✅ Available | `colordesign-engine` | `convert` | Free online hsl converter utility. This tool is planned and coming soon. |
| 277 | **HTML Entity Decoder** | `html-entity-decoder` | ✅ Available | `encoding-engine` | `html_decode` | Free online html entity decoder utility. This tool is planned and coming soon. |
| 276 | **HTML Entity Encoder** | `html-entity-encoder` | ✅ Available | `encoding-engine` | `html_encode` | Free online html entity encoder utility. This tool is planned and coming soon. |
| 258 | **HTML Formatter** | `html-formatter` | ✅ Available | `developer-engine` | `html_format` | Free online html formatter utility. This tool is planned and coming soon. |
| 259 | **HTML Minifier** | `html-minifier` | ✅ Available | `developer-engine` | `html_minify` | Free online html minifier utility. This tool is planned and coming soon. |
| 659 | **HTML to JSX Converter** | `html-to-jsx-converter` | ✅ Available | `developer-engine` | `html_to_jsx` | Free online html to jsx converter utility. This tool is planned and coming soon. |
| 280 | **Hash Generator** | `hash-generator` | ✅ Available | `crypto-engine` | `hash` | Free online hash generator utility. This tool is planned and coming soon. |
| 100118 | **Hex Decoder** | `hex-decoder` | ✅ Available | `encoding-engine` | `hex_decode` | Hex Decoder utility powered by encoding-engine. |
| 100117 | **Hex Encoder** | `hex-encoder` | ✅ Available | `encoding-engine` | `hex_encode` | Hex Encoder utility powered by encoding-engine. |
| 100300 | **Histogram Generator** | `histogram-generator` | ✅ Available | `dataviz-engine` | `histogram` | Histogram Generator utility powered by dataviz-engine. |
| 100123 | **Html Entity Encode Decode** | `html-entity-encode-decode` | ✅ Available | `encoding-engine` | `html_encode` | Html Entity Encode Decode utility powered by encoding-engine. |
| 100137 | **Http Status Lookup** | `http-status-lookup` | ✅ Available | `developer-engine` | `http_status` | Http Status Lookup utility powered by developer-engine. |
| 652 | **INI Formatter** | `ini-formatter` | ✅ Available | `json-engine` | `ini_format` | Free online ini formatter utility. This tool is planned and coming soon. |
| 100109 | **INI To Json** | `ini-to-json` | ✅ Available | `json-engine` | `from_ini` | INI To Json utility powered by json-engine. |
| 100147 | **Import Sorter** | `import-sorter` | ✅ Available | `developer-engine` | `import_sort` | Import Sorter utility powered by developer-engine. |
| 249 | **JSON Beautifier** | `json-beautifier` | ✅ Available | `json-engine` | `format` | Free online json beautifier utility. This tool is planned and coming soon. |
| 246 | **JSON Formatter** | `json-formatter` | ✅ Available | `json-engine` | `format` | Free online json formatter utility. This tool is planned and coming soon. |
| 248 | **JSON Minifier** | `json-minifier` | ✅ Available | `json-engine` | `minify` | Free online json minifier utility. This tool is planned and coming soon. |
| 247 | **JSON Validator** | `json-validator` | ✅ Available | `json-engine` | `validate` | Free online json validator utility. This tool is planned and coming soon. |
| 252 | **JSON to CSV** | `json-to-csv` | ✅ Available | `json-engine` | `to_csv` | Free online json to csv utility. This tool is planned and coming soon. |
| 254 | **JSON to XML** | `json-to-xml` | ✅ Available | `json-engine` | `to_xml` | Free online json to xml utility. This tool is planned and coming soon. |
| 250 | **JSON to YAML** | `json-to-yaml` | ✅ Available | `json-engine` | `to_yaml` | Free online json to yaml utility. This tool is planned and coming soon. |
| 269 | **JWT Decoder** | `jwt-decoder` | ✅ Available | `crypto-engine` | `jwt_decode` | Free online jwt decoder utility. This tool is planned and coming soon. |
| 642 | **JWT Expiration Calculator** | `jwt-expiration-calculator` | ✅ Available | `crypto-engine` | `jwt_decode` | Free online jwt expiration calculator utility. This tool is planned and coming soon. |
| 270 | **JWT Generator** | `jwt-generator` | ✅ Available | `crypto-engine` | `jwt_sign` | Free online jwt generator utility. This tool is planned and coming soon. |
| 271 | **JWT Validator** | `jwt-validator` | ✅ Available | `crypto-engine` | `jwt_verify` | Free online jwt validator utility. This tool is planned and coming soon. |
| 262 | **JavaScript Formatter** | `javascript-formatter` | ✅ Available | `developer-engine` | `js_format` | Free online javascript formatter utility. This tool is planned and coming soon. |
| 263 | **JavaScript Minifier** | `javascript-minifier` | ✅ Available | `developer-engine` | `js_minify` | Free online javascript minifier utility. This tool is planned and coming soon. |
| 100068 | **Json Array Sorter** | `json-array-sorter` | ✅ Available | `json-engine` | `sort_array` | Json Array Sorter utility powered by json-engine. |
| 100299 | **Json Chart Generator** | `json-chart-generator` | ✅ Available | `dataviz-engine` | `chart` | Json Chart Generator utility powered by dataviz-engine. |
| 100062 | **Json Compare** | `json-compare` | ✅ Available | `json-engine` | `diff` | Json Compare utility powered by json-engine. |
| 100061 | **Json Diff** | `json-diff` | ✅ Available | `json-engine` | `diff` | Json Diff utility powered by json-engine. |
| 100066 | **Json Escape** | `json-escape` | ✅ Available | `json-engine` | `escape` | Json Escape utility powered by json-engine. |
| 100070 | **Json Flattener** | `json-flattener` | ✅ Available | `json-engine` | `flatten` | Json Flattener utility powered by json-engine. |
| 100069 | **Json Key Sorter** | `json-key-sorter` | ✅ Available | `json-engine` | `sort_keys` | Json Key Sorter utility powered by json-engine. |
| 100063 | **Json Path Query** | `json-path-query` | ✅ Available | `json-engine` | `path_query` | Json Path Query utility powered by json-engine. |
| 100065 | **Json Schema Generator** | `json-schema-generator` | ✅ Available | `json-engine` | `schema_generate` | Json Schema Generator utility powered by json-engine. |
| 100064 | **Json Schema Validator** | `json-schema-validator` | ✅ Available | `json-engine` | `schema_validate` | Json Schema Validator utility powered by json-engine. |
| 100140 | **Json To Code** | `json-to-code` | ✅ Available | `json-engine` | `to_code` | Json To Code utility powered by json-engine. |
| 100060 | **Json To Go Struct** | `json-to-go-struct` | ✅ Available | `json-engine` | `to_code` | Json To Go Struct utility powered by json-engine. |
| 100059 | **Json To Rust Struct** | `json-to-rust-struct` | ✅ Available | `json-engine` | `to_code` | Json To Rust Struct utility powered by json-engine. |
| 100072 | **Json To Table** | `json-to-table` | ✅ Available | `json-engine` | `to_table` | Json To Table utility powered by json-engine. |
| 100057 | **Json To Toml** | `json-to-toml` | ✅ Available | `json-engine` | `to_toml` | Json To Toml utility powered by json-engine. |
| 100058 | **Json To Typescript** | `json-to-typescript` | ✅ Available | `json-engine` | `to_code` | Json To Typescript utility powered by json-engine. |
| 100067 | **Json Unescape** | `json-unescape` | ✅ Available | `json-engine` | `unescape` | Json Unescape utility powered by json-engine. |
| 100071 | **Json Unflattener** | `json-unflattener` | ✅ Available | `json-engine` | `unflatten` | Json Unflattener utility powered by json-engine. |
| 281 | **MD5 Generator** | `md5-generator` | ✅ Available | `crypto-engine` | `hash` | Free online md5 generator utility. This tool is planned and coming soon. |
| 310 | **Markdown Table Generator** | `markdown-table-generator` | ✅ Available | `csv-engine` | `to_markdown` | Free online markdown table generator utility. This tool is planned and coming soon. |
| 100127 | **Messagepack Converter** | `messagepack-converter` | ✅ Available | `encoding-engine` | `msgpack_encode` | Messagepack Converter utility powered by encoding-engine. |
| 100126 | **Mime Word Decoder** | `mime-word-decoder` | ✅ Available | `encoding-engine` | `mime_word_decode` | Mime Word Decoder utility powered by encoding-engine. |
| 287 | **NanoID Generator** | `nanoid-generator` | ✅ Available | `crypto-engine` | `nanoid` | Free online nanoid generator utility. This tool is planned and coming soon. |
| 100305 | **Outlier Detector** | `outlier-detector` | ✅ Available | `dataviz-engine` | `outliers` | Outlier Detector utility powered by dataviz-engine. |
| 288 | **Password Generator** | `password-generator` | ✅ Available | `crypto-engine` | `password` | Free online password generator utility. This tool is planned and coming soon. |
| 100135 | **Patch Parser** | `patch-parser` | ✅ Available | `developer-engine` | `patch_parse` | Patch Parser utility powered by developer-engine. |
| 100302 | **Pivot Table Lite** | `pivot-table-lite` | ✅ Available | `dataviz-engine` | `pivot` | Pivot Table Lite utility powered by dataviz-engine. |
| 100110 | **Plist Viewer** | `plist-viewer` | ✅ Available | `json-engine` | `from_plist` | Plist Viewer utility powered by json-engine. |
| 641 | **Protobuf Decoder** | `protobuf-decoder` | ✅ Available | `encoding-engine` | `protobuf_inspect` | Free online protobuf decoder utility. This tool is planned and coming soon. |
| 100124 | **Punycode Converter** | `punycode-converter` | ✅ Available | `encoding-engine` | `punycode_encode` | Punycode Converter utility powered by encoding-engine. |
| 304 | **QR Code Generator** | `qr-code-generator` | ✅ Available | `qrbarcode-engine` | `qr_generate` | Free online qr code generator utility. This tool is planned and coming soon. |
| 305 | **QR Code Reader** | `qr-code-reader` | ✅ Available | `qrbarcode-engine` | `qr_decode` | Free online qr code reader utility. This tool is planned and coming soon. |
| 100139 | **Query String Parser** | `query-string-parser` | ✅ Available | `developer-engine` | `query_parse` | Query String Parser utility powered by developer-engine. |
| 100125 | **Quoted Printable Converter** | `quoted-printable-converter` | ✅ Available | `encoding-engine` | `quoted_printable_encode` | Quoted Printable Converter utility powered by encoding-engine. |
| 294 | **RGB to HEX** | `rgb-to-hex` | ✅ Available | `colordesign-engine` | `convert` | Free online rgb to hex utility. This tool is planned and coming soon. |
| 658 | **Regex Match Extractor** | `regex-match-extractor` | ✅ Available | `developer-engine` | `regex_test` | Free online regex match extractor utility. This tool is planned and coming soon. |
| 266 | **Regex Tester** | `regex-tester` | ✅ Available | `developer-engine` | `regex_test` | Free online regex tester utility. This tool is planned and coming soon. |
| 267 | **Regex Visualizer** | `regex-visualizer` | ✅ Available | `developer-engine` | `regex_explain` | Free online regex visualizer utility. This tool is planned and coming soon. |
| 282 | **SHA-1 Generator** | `sha-1-generator` | ✅ Available | `crypto-engine` | `hash` | Free online sha-1 generator utility. This tool is planned and coming soon. |
| 283 | **SHA-256 Generator** | `sha-256-generator` | ✅ Available | `crypto-engine` | `hash` | Free online sha-256 generator utility. This tool is planned and coming soon. |
| 284 | **SHA-512 Generator** | `sha-512-generator` | ✅ Available | `crypto-engine` | `hash` | Free online sha-512 generator utility. This tool is planned and coming soon. |
| 264 | **SQL Formatter** | `sql-formatter` | ✅ Available | `developer-engine` | `sql_format` | Free online sql formatter utility. This tool is planned and coming soon. |
| 265 | **SQL Minifier** | `sql-minifier` | ✅ Available | `developer-engine` | `sql_minify` | Free online sql minifier utility. This tool is planned and coming soon. |
| 100150 | **SQL Query Analyzer** | `sql-query-analyzer` | ✅ Available | `developer-engine` | `sql_analyze` | SQL Query Analyzer utility powered by developer-engine. |
| 631 | **SQL Query Builder** | `sql-query-builder` | ✅ Available | `developer-engine` | `sql_format` | Free online sql query builder utility. This tool is planned and coming soon. |
| 632 | **SQL Schema Visualizer** | `sql-schema-visualizer` | ✅ Available | `developer-engine` | `sql_erd` | Free online sql schema visualizer utility. This tool is planned and coming soon. |
| 100129 | **SQL Validator** | `sql-validator` | ✅ Available | `developer-engine` | `sql_validate` | SQL Validator utility powered by developer-engine. |
| 301 | **SVG Optimizer** | `svg-optimizer` | ✅ Available | `image-engine` | `svg_optimize` | Free online svg optimizer utility. This tool is planned and coming soon. |
| 302 | **SVG Viewer** | `svg-viewer` | ✅ Available | `image-engine` | `svg_info` | Free online svg viewer utility. This tool is planned and coming soon. |
| 643 | **Semantic Version Comparator** | `semantic-version-comparator` | ✅ Available | `developer-engine` | `semver_check` | Free online semantic version comparator utility. This tool is planned and coming soon. |
| 644 | **Semantic Version Generator** | `semantic-version-generator` | ✅ Available | `developer-engine` | `semver_check` | Free online semantic version generator utility. This tool is planned and coming soon. |
| 651 | **TOML Formatter** | `toml-formatter` | ✅ Available | `json-engine` | `toml_format` | Free online toml formatter utility. This tool is planned and coming soon. |
| 100108 | **Toml To Json** | `toml-to-json` | ✅ Available | `json-engine` | `from_toml` | Toml To Json utility powered by json-engine. |
| 100107 | **Toml Validator** | `toml-validator` | ✅ Available | `json-engine` | `toml_validate` | Toml Validator utility powered by json-engine. |
| 100132 | **Typescript Formatter** | `typescript-formatter` | ✅ Available | `developer-engine` | `js_format` | Typescript Formatter utility powered by developer-engine. |
| 286 | **ULID Generator** | `ulid-generator` | ✅ Available | `crypto-engine` | `ulid` | Free online ulid generator utility. This tool is planned and coming soon. |
| 275 | **URL Decoder** | `url-decoder` | ✅ Available | `encoding-engine` | `url_decode` | Free online url decoder utility. This tool is planned and coming soon. |
| 100121 | **URL Encode Decode** | `url-encode-decode` | ✅ Available | `encoding-engine` | `url_encode` | URL Encode Decode utility powered by encoding-engine. |
| 274 | **URL Encoder** | `url-encoder` | ✅ Available | `encoding-engine` | `url_encode` | Free online url encoder utility. This tool is planned and coming soon. |
| 100138 | **URL Parser** | `url-parser` | ✅ Available | `developer-engine` | `url_parse` | URL Parser utility powered by developer-engine. |
| 285 | **UUID Generator** | `uuid-generator` | ✅ Available | `crypto-engine` | `uuid` | Free online uuid generator utility. This tool is planned and coming soon. |
| 279 | **Unicode Decoder** | `unicode-decoder` | ✅ Available | `text-engine` | `unicode_unescape` | Free online unicode decoder utility. This tool is planned and coming soon. |
| 278 | **Unicode Encoder** | `unicode-encoder` | ✅ Available | `text-engine` | `unicode_escape` | Free online unicode encoder utility. This tool is planned and coming soon. |
| 291 | **Unix Timestamp Converter** | `unix-timestamp-converter` | ✅ Available | `datetime-engine` | `timestamp` | Free online unix timestamp converter utility. This tool is planned and coming soon. |
| 100148 | **Unused Import Checker** | `unused-import-checker` | ✅ Available | `developer-engine` | `unused_imports` | Unused Import Checker utility powered by developer-engine. |
| 256 | **XML Formatter** | `xml-formatter` | ✅ Available | `json-engine` | `xml_format` | Free online xml formatter utility. This tool is planned and coming soon. |
| 100104 | **XML Minifier** | `xml-minifier` | ✅ Available | `json-engine` | `xml_minify` | XML Minifier utility powered by json-engine. |
| 257 | **XML Validator** | `xml-validator` | ✅ Available | `json-engine` | `xml_validate` | Free online xml validator utility. This tool is planned and coming soon. |
| 255 | **XML to JSON** | `xml-to-json` | ✅ Available | `json-engine` | `from_xml` | Free online xml to json utility. This tool is planned and coming soon. |
| 251 | **YAML to JSON** | `yaml-to-json` | ✅ Available | `json-engine` | `from_yaml` | Free online yaml to json utility. This tool is planned and coming soon. |
| 100105 | **Yaml Formatter** | `yaml-formatter` | ✅ Available | `json-engine` | `yaml_format` | Yaml Formatter utility powered by json-engine. |
| 100133 | **Yaml Linter** | `yaml-linter` | ✅ Available | `developer-engine` | `lint_yaml` | Yaml Linter utility powered by developer-engine. |
| 100106 | **Yaml Validator** | `yaml-validator` | ✅ Available | `json-engine` | `yaml_validate` | Yaml Validator utility powered by json-engine. |
| 637 | **API Documentation Previewer** | `api-documentation-previewer` | ⏳ Coming Soon | — | — | Free online api documentation previewer utility. This tool is planned and coming soon. |
| 300 | **CSS Animation Generator** | `css-animation-generator` | ⏳ Coming Soon | — | — | Free online css animation generator utility. This tool is planned and coming soon. |
| 647 | **Changelog Generator** | `changelog-generator` | ⏳ Coming Soon | — | — | Free online changelog generator utility. This tool is planned and coming soon. |
| 645 | **Git Command Generator** | `git-command-generator` | ⏳ Coming Soon | — | — | Free online git command generator utility. This tool is planned and coming soon. |
| 646 | **GitHub README Generator** | `github-readme-generator` | ⏳ Coming Soon | — | — | Free online github readme generator utility. This tool is planned and coming soon. |
| 640 | **GraphQL Explorer** | `graphql-explorer` | ⏳ Coming Soon | — | — | Free online graphql explorer utility. This tool is planned and coming soon. |
| 648 | **License Generator** | `license-generator` | ⏳ Coming Soon | — | — | Free online license generator utility. This tool is planned and coming soon. |
| 657 | **Log Prettifier** | `log-prettifier` | ⏳ Coming Soon | — | — | Free online log prettifier utility. This tool is planned and coming soon. |
| 656 | **Log Viewer** | `log-viewer` | ⏳ Coming Soon | — | — | Free online log viewer utility. This tool is planned and coming soon. |
| 660 | **Markdown to JSX Converter** | `markdown-to-jsx-converter` | ⏳ Coming Soon | — | — | Free online markdown to jsx converter utility. This tool is planned and coming soon. |
| 636 | **Markdown to Mermaid** | `markdown-to-mermaid` | ⏳ Coming Soon | — | — | Free online markdown to mermaid utility. This tool is planned and coming soon. |
| 634 | **Mermaid Diagram Editor** | `mermaid-diagram-editor` | ⏳ Coming Soon | — | — | Free online mermaid diagram editor utility. This tool is planned and coming soon. |
| 638 | **OpenAPI/Swagger Viewer** | `openapi-swagger-viewer` | ⏳ Coming Soon | — | — | Free online openapi/swagger viewer utility. This tool is planned and coming soon. |
| 635 | **PlantUML Previewer** | `plantuml-previewer` | ⏳ Coming Soon | — | — | Free online plantuml previewer utility. This tool is planned and coming soon. |
| 268 | **Regex Generator** | `regex-generator` | ⏳ Coming Soon | — | — | Free online regex generator utility. This tool is planned and coming soon. |
| 654 | **SQL to JSON Converter** | `sql-to-json-converter` | ⏳ Coming Soon | — | — | Free online sql to json converter utility. This tool is planned and coming soon. |
| 303 | **SVG Path Editor** | `svg-path-editor` | ⏳ Coming Soon | — | — | Free online svg path editor utility. This tool is planned and coming soon. |
| 655 | **Stack Trace Formatter** | `stack-trace-formatter` | ⏳ Coming Soon | — | — | Free online stack trace formatter utility. This tool is planned and coming soon. |

### Everyday Tools (41 tools — 28 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 100309 | **Address Label Maker** | `address-label-maker` | ✅ Available | `qrbarcode-engine` | `qr_generate` | Address Label Maker utility powered by qrbarcode-engine. |
| 672 | **Age in Days Calculator** | `age-in-days-calculator` | ✅ Available | `datetime-engine` | `age` | Free online age in days calculator utility. This tool is planned and coming soon. |
| 100307 | **Barcode Label Maker** | `barcode-label-maker` | ✅ Available | `qrbarcode-engine` | `barcode_generate` | Barcode Label Maker utility powered by qrbarcode-engine. |
| 673 | **Birthday Countdown** | `birthday-countdown` | ✅ Available | `datetime-engine` | `countdown` | Free online birthday countdown utility. This tool is planned and coming soon. |
| 100232 | **Bulk QR Generator** | `bulk-qr-generator` | ✅ Available | `qrbarcode-engine` | `qr_bulk` | Bulk QR Generator utility powered by qrbarcode-engine. |
| 100220 | **Business Days Calculator** | `business-days-calculator` | ✅ Available | `datetime-engine` | `business_days` | Business Days Calculator utility powered by datetime-engine. |
| 663 | **Cooking Temperature Converter** | `cooking-temperature-converter` | ✅ Available | `units-engine` | `temperature` | Free online cooking temperature converter utility. This tool is planned and coming soon. |
| 100222 | **Cron Next Run** | `cron-next-run` | ✅ Available | `datetime-engine` | `cron_next` | Cron Next Run utility powered by datetime-engine. |
| 685 | **Daily Expense Splitter** | `daily-expense-splitter` | ✅ Available | `finance-engine` | `tip` | Free online daily expense splitter utility. This tool is planned and coming soon. |
| 100219 | **Date To Timestamp** | `date-to-timestamp` | ✅ Available | `datetime-engine` | `timestamp` | Date To Timestamp utility powered by datetime-engine. |
| 676 | **Electricity Bill Calculator** | `electricity-bill-calculator` | ✅ Available | `solar-engine` | `bill` | Free online electricity bill calculator utility. This tool is planned and coming soon. |
| 683 | **Loan Payoff Calculator** | `loan-payoff-calculator` | ✅ Available | `finance-engine` | `loan` | Free online loan payoff calculator utility. This tool is planned and coming soon. |
| 100311 | **Printable Sticker Sheet** | `printable-sticker-sheet` | ✅ Available | `qrbarcode-engine` | `qr_bulk` | Printable Sticker Sheet utility powered by qrbarcode-engine. |
| 100233 | **QR Code Styler** | `qr-code-styler` | ✅ Available | `qrbarcode-engine` | `qr_generate` | QR Code Styler utility powered by qrbarcode-engine. |
| 100235 | **QR Code To PNG** | `qr-code-to-png` | ✅ Available | `qrbarcode-engine` | `qr_generate` | QR Code To PNG utility powered by qrbarcode-engine. |
| 100234 | **QR Code To SVG** | `qr-code-to-svg` | ✅ Available | `qrbarcode-engine` | `qr_generate` | QR Code To SVG utility powered by qrbarcode-engine. |
| 100310 | **QR Label Sheet Generator** | `qr-label-sheet-generator` | ✅ Available | `qrbarcode-engine` | `qr_bulk` | QR Label Sheet Generator utility powered by qrbarcode-engine. |
| 662 | **Recipe Measurement Converter** | `recipe-measurement-converter` | ✅ Available | `units-engine` | `cooking` | Free online recipe measurement converter utility. This tool is planned and coming soon. |
| 675 | **Rent Split Calculator** | `rent-split-calculator` | ✅ Available | `finance-engine` | `tip` | Free online rent split calculator utility. This tool is planned and coming soon. |
| 684 | **Savings Goal Calculator** | `savings-goal-calculator` | ✅ Available | `finance-engine` | `investment` | Free online savings goal calculator utility. This tool is planned and coming soon. |
| 100308 | **Shipping Label Maker** | `shipping-label-maker` | ✅ Available | `qrbarcode-engine` | `qr_generate` | Shipping Label Maker utility powered by qrbarcode-engine. |
| 671 | **Time Zone Meeting Planner** | `time-zone-meeting-planner` | ✅ Available | `datetime-engine` | `meeting_planner` | Free online time zone meeting planner utility. This tool is planned and coming soon. |
| 100218 | **Timestamp To Date** | `timestamp-to-date` | ✅ Available | `datetime-engine` | `timestamp` | Timestamp To Date utility powered by datetime-engine. |
| 668 | **Trip Fuel Calculator** | `trip-fuel-calculator` | ✅ Available | `units-engine` | `convert` | Free online trip fuel calculator utility. This tool is planned and coming soon. |
| 100231 | **Vcard QR Generator** | `vcard-qr-generator` | ✅ Available | `qrbarcode-engine` | `qr_vcard` | Vcard QR Generator utility powered by qrbarcode-engine. |
| 680 | **Vehicle Mileage Calculator** | `vehicle-mileage-calculator` | ✅ Available | `units-engine` | `convert` | Free online vehicle mileage calculator utility. This tool is planned and coming soon. |
| 100221 | **Week Number Calculator** | `week-number-calculator` | ✅ Available | `datetime-engine` | `week_number` | Week Number Calculator utility powered by datetime-engine. |
| 100230 | **Wifi QR Generator** | `wifi-qr-generator` | ✅ Available | `qrbarcode-engine` | `qr_wifi` | Wifi QR Generator utility powered by qrbarcode-engine. |
| 682 | **Bike Maintenance Tracker** | `bike-maintenance-tracker` | ⏳ Coming Soon | — | — | Free online bike maintenance tracker utility. This tool is planned and coming soon. |
| 681 | **Car Maintenance Tracker** | `car-maintenance-tracker` | ⏳ Coming Soon | — | — | Free online car maintenance tracker utility. This tool is planned and coming soon. |
| 664 | **Cooking Timer** | `cooking-timer` | ⏳ Coming Soon | — | — | Free online cooking timer utility. This tool is planned and coming soon. |
| 661 | **Grocery List Maker** | `grocery-list-maker` | ⏳ Coming Soon | — | — | Free online grocery list maker utility. This tool is planned and coming soon. |
| 678 | **Internet Data Usage Calculator** | `internet-data-usage-calculator` | ⏳ Coming Soon | — | — | Free online internet data usage calculator utility. This tool is planned and coming soon. |
| 679 | **Mobile Recharge Comparison Calculator** | `mobile-recharge-comparison-calculator` | ⏳ Coming Soon | — | — | Free online mobile recharge comparison calculator utility. This tool is planned and coming soon. |
| 670 | **Packing Checklist Generator** | `packing-checklist-generator` | ⏳ Coming Soon | — | — | Free online packing checklist generator utility. This tool is planned and coming soon. |
| 669 | **Road Trip Planner** | `road-trip-planner` | ⏳ Coming Soon | — | — | Free online road trip planner utility. This tool is planned and coming soon. |
| 666 | **Sleep Calculator** | `sleep-calculator` | ⏳ Coming Soon | — | — | Free online sleep calculator utility. This tool is planned and coming soon. |
| 667 | **Travel Budget Calculator** | `travel-budget-calculator` | ⏳ Coming Soon | — | — | Free online travel budget calculator utility. This tool is planned and coming soon. |
| 677 | **Water Bill Calculator** | `water-bill-calculator` | ⏳ Coming Soon | — | — | Free online water bill calculator utility. This tool is planned and coming soon. |
| 665 | **Water Intake Tracker** | `water-intake-tracker` | ⏳ Coming Soon | — | — | Free online water intake tracker utility. This tool is planned and coming soon. |
| 674 | **Wedding Budget Calculator** | `wedding-budget-calculator` | ⏳ Coming Soon | — | — | Free online wedding budget calculator utility. This tool is planned and coming soon. |

### File Tools (74 tools — 66 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 100040 | **Archive List Files** | `archive-list-files` | ✅ Available | `archive-engine` | `list` | Archive List Files utility powered by archive-engine. |
| 100041 | **Archive Test Integrity** | `archive-test-integrity` | ✅ Available | `archive-engine` | `test` | Archive Test Integrity utility powered by archive-engine. |
| 416 | **Barcode Scanner** | `barcode-scanner` | ✅ Available | `qrbarcode-engine` | `barcode_read` | Free online barcode scanner utility. This tool is planned and coming soon. |
| 443 | **CSV Cleaner** | `csv-cleaner` | ✅ Available | `csv-engine` | `clean` | Free online csv cleaner utility. This tool is planned and coming soon. |
| 100079 | **CSV Column Extractor** | `csv-column-extractor` | ✅ Available | `csv-engine` | `select_columns` | CSV Column Extractor utility powered by csv-engine. |
| 100080 | **CSV Column Reorder** | `csv-column-reorder` | ✅ Available | `csv-engine` | `reorder_columns` | CSV Column Reorder utility powered by csv-engine. |
| 100087 | **CSV Compare** | `csv-compare` | ✅ Available | `csv-engine` | `diff` | CSV Compare utility powered by csv-engine. |
| 100078 | **CSV Deduplicator** | `csv-deduplicator` | ✅ Available | `csv-engine` | `dedupe` | CSV Deduplicator utility powered by csv-engine. |
| 100088 | **CSV Diff** | `csv-diff` | ✅ Available | `csv-engine` | `diff` | CSV Diff utility powered by csv-engine. |
| 442 | **CSV Editor** | `csv-editor` | ✅ Available | `csv-engine` | `write` | Free online csv editor utility. This tool is planned and coming soon. |
| 100089 | **CSV Encoding Converter** | `csv-encoding-converter` | ✅ Available | `csv-engine` | `encoding_convert` | CSV Encoding Converter utility powered by csv-engine. |
| 100081 | **CSV Filter** | `csv-filter` | ✅ Available | `csv-engine` | `filter` | CSV Filter utility powered by csv-engine. |
| 100076 | **CSV Merge** | `csv-merge` | ✅ Available | `csv-engine` | `merge` | CSV Merge utility powered by csv-engine. |
| 100082 | **CSV Sort** | `csv-sort` | ✅ Available | `csv-engine` | `sort` | CSV Sort utility powered by csv-engine. |
| 100077 | **CSV Split** | `csv-split` | ✅ Available | `csv-engine` | `split` | CSV Split utility powered by csv-engine. |
| 100085 | **CSV Statistics** | `csv-statistics` | ✅ Available | `csv-engine` | `statistics` | CSV Statistics utility powered by csv-engine. |
| 100083 | **CSV To Jsonl** | `csv-to-jsonl` | ✅ Available | `csv-engine` | `to_jsonl` | CSV To Jsonl utility powered by csv-engine. |
| 100074 | **CSV To TSV** | `csv-to-tsv` | ✅ Available | `csv-engine` | `convert` | CSV To TSV utility powered by csv-engine. |
| 100091 | **CSV To Xlsx** | `csv-to-xlsx` | ✅ Available | `spreadsheet-engine` | `from_csv` | CSV To Xlsx utility powered by spreadsheet-engine. |
| 100086 | **CSV Transpose** | `csv-transpose` | ✅ Available | `csv-engine` | `transpose` | CSV Transpose utility powered by csv-engine. |
| 100073 | **CSV Validator** | `csv-validator` | ✅ Available | `csv-engine` | `validate` | CSV Validator utility powered by csv-engine. |
| 441 | **CSV Viewer** | `csv-viewer` | ✅ Available | `csv-engine` | `parse` | Free online csv viewer utility. This tool is planned and coming soon. |
| 446 | **CSV to Excel** | `csv-to-excel` | ✅ Available | `spreadsheet-engine` | `from_csv` | Free online csv to excel utility. This tool is planned and coming soon. |
| 449 | **EPUB Reader** | `epub-reader` | ✅ Available | `document-engine` | `epub_inspect` | Free online epub reader utility. This tool is planned and coming soon. |
| 447 | **Excel Formula Helper** | `excel-formula-helper` | ✅ Available | `spreadsheet-engine` | `evaluate` | Free online excel formula helper utility. This tool is planned and coming soon. |
| 444 | **Excel Viewer** | `excel-viewer` | ✅ Available | `spreadsheet-engine` | `read` | Free online excel viewer utility. This tool is planned and coming soon. |
| 445 | **Excel to CSV** | `excel-to-csv` | ✅ Available | `spreadsheet-engine` | `to_csv` | Free online excel to csv utility. This tool is planned and coming soon. |
| 100039 | **File Archive Converter** | `file-archive-converter` | ✅ Available | `archive-engine` | `convert` | File Archive Converter utility powered by archive-engine. |
| 419 | **File Checksum Calculator** | `file-checksum-calculator` | ✅ Available | `crypto-engine` | `hash` | Free online file checksum calculator utility. This tool is planned and coming soon. |
| 435 | **File Checksum Verifier** | `file-checksum-verifier` | ✅ Available | `crypto-engine` | `hash` | Free online file checksum verifier utility. This tool is planned and coming soon. |
| 433 | **File Comparison** | `file-comparison` | ✅ Available | `text-engine` | `diff` | Free online file comparison utility. This tool is planned and coming soon. |
| 429 | **File Extension Detector** | `file-extension-detector` | ✅ Available | `developer-engine` | `mime` | Free online file extension detector utility. This tool is planned and coming soon. |
| 418 | **File Hash Checker** | `file-hash-checker` | ✅ Available | `crypto-engine` | `hash` | Free online file hash checker utility. This tool is planned and coming soon. |
| 436 | **File Metadata Viewer** | `file-metadata-viewer` | ✅ Available | `image-engine` | `info` | Free online file metadata viewer utility. This tool is planned and coming soon. |
| 431 | **File Signature Detector** | `file-signature-detector` | ✅ Available | `developer-engine` | `mime` | Free online file signature detector utility. This tool is planned and coming soon. |
| 432 | **File Size Converter** | `file-size-converter` | ✅ Available | `units-engine` | `convert` | Free online file size converter utility. This tool is planned and coming soon. |
| 425 | **GZIP Compressor** | `gzip-compressor` | ✅ Available | `archive-engine` | `create` | Free online gzip compressor utility. This tool is planned and coming soon. |
| 426 | **GZIP Decompressor** | `gzip-decompressor` | ✅ Available | `archive-engine` | `extract` | Free online gzip decompressor utility. This tool is planned and coming soon. |
| 100094 | **Json To Xlsx** | `json-to-xlsx` | ✅ Available | `spreadsheet-engine` | `from_json` | Json To Xlsx utility powered by spreadsheet-engine. |
| 100084 | **Jsonl To CSV** | `jsonl-to-csv` | ✅ Available | `csv-engine` | `from_jsonl` | Jsonl To CSV utility powered by csv-engine. |
| 430 | **MIME Type Detector** | `mime-type-detector` | ✅ Available | `developer-engine` | `mime` | Free online mime type detector utility. This tool is planned and coming soon. |
| 450 | **MOBI Reader** | `mobi-reader` | ✅ Available | `document-engine` | `mobi_metadata` | Free online mobi reader utility. This tool is planned and coming soon. |
| 100101 | **ODS To Xlsx** | `ods-to-xlsx` | ✅ Available | `spreadsheet-engine` | `convert` | ODS To Xlsx utility powered by spreadsheet-engine. |
| 417 | **Password Strength Checker** | `password-strength-checker` | ✅ Available | `crypto-engine` | `password_strength` | Free online password strength checker utility. This tool is planned and coming soon. |
| 415 | **QR Code Scanner** | `qr-code-scanner` | ✅ Available | `qrbarcode-engine` | `qr_decode` | Free online qr code scanner utility. This tool is planned and coming soon. |
| 100103 | **Spreadsheet Column Letter** | `spreadsheet-column-letter` | ✅ Available | `spreadsheet-engine` | `column_letter` | Spreadsheet Column Letter utility powered by spreadsheet-engine. |
| 424 | **TAR Extractor** | `tar-extractor` | ✅ Available | `archive-engine` | `extract` | Free online tar extractor utility. This tool is planned and coming soon. |
| 100075 | **TSV To CSV** | `tsv-to-csv` | ✅ Available | `csv-engine` | `convert` | TSV To CSV utility powered by csv-engine. |
| 100038 | **Tar Create** | `tar-create` | ✅ Available | `archive-engine` | `create` | Tar Create utility powered by archive-engine. |
| 448 | **XML Viewer** | `xml-viewer` | ✅ Available | `json-engine` | `xml_format` | Free online xml viewer utility. This tool is planned and coming soon. |
| 100098 | **Xlsx Cleaner** | `xlsx-cleaner` | ✅ Available | `spreadsheet-engine` | `clean` | Xlsx Cleaner utility powered by spreadsheet-engine. |
| 100100 | **Xlsx Diff** | `xlsx-diff` | ✅ Available | `spreadsheet-engine` | `diff` | Xlsx Diff utility powered by spreadsheet-engine. |
| 100099 | **Xlsx Formula Inspector** | `xlsx-formula-inspector` | ✅ Available | `spreadsheet-engine` | `formulas` | Xlsx Formula Inspector utility powered by spreadsheet-engine. |
| 100095 | **Xlsx Merge** | `xlsx-merge` | ✅ Available | `spreadsheet-engine` | `merge` | Xlsx Merge utility powered by spreadsheet-engine. |
| 100097 | **Xlsx Remove Duplicates** | `xlsx-remove-duplicates` | ✅ Available | `spreadsheet-engine` | `dedupe` | Xlsx Remove Duplicates utility powered by spreadsheet-engine. |
| 100096 | **Xlsx Split Sheets** | `xlsx-split-sheets` | ✅ Available | `spreadsheet-engine` | `split_sheets` | Xlsx Split Sheets utility powered by spreadsheet-engine. |
| 100090 | **Xlsx To CSV** | `xlsx-to-csv` | ✅ Available | `spreadsheet-engine` | `to_csv` | Xlsx To CSV utility powered by spreadsheet-engine. |
| 100093 | **Xlsx To Json** | `xlsx-to-json` | ✅ Available | `spreadsheet-engine` | `to_json` | Xlsx To Json utility powered by spreadsheet-engine. |
| 100102 | **Xlsx To ODS** | `xlsx-to-ods` | ✅ Available | `spreadsheet-engine` | `convert` | Xlsx To ODS utility powered by spreadsheet-engine. |
| 100092 | **Xlsx To PDF** | `xlsx-to-pdf` | ✅ Available | `spreadsheet-engine` | `to_html` | Xlsx To PDF utility powered by spreadsheet-engine. |
| 423 | **ZIP Compressor** | `zip-compressor` | ✅ Available | `archive-engine` | `create` | Free online zip compressor utility. This tool is planned and coming soon. |
| 422 | **ZIP Creator** | `zip-creator` | ✅ Available | `archive-engine` | `create` | Free online zip creator utility. This tool is planned and coming soon. |
| 421 | **ZIP Extractor** | `zip-extractor` | ✅ Available | `archive-engine` | `extract` | Free online zip extractor utility. This tool is planned and coming soon. |
| 100037 | **Zip Password Check** | `zip-password-check` | ✅ Available | `archive-engine` | `test` | Zip Password Check utility powered by archive-engine. |
| 100036 | **Zip Repair** | `zip-repair` | ✅ Available | `archive-engine` | `zip_repair` | Zip Repair utility powered by archive-engine. |
| 100035 | **Zip View** | `zip-view` | ✅ Available | `archive-engine` | `list` | Zip View utility powered by archive-engine. |
| 427 | **7Z Viewer/Extractor** | `7z-viewer-extractor` | ⏳ Coming Soon | `archive-engine` | `seven_zip_extract` | Free online 7z viewer/extractor utility. This tool is planned and coming soon. |
| 434 | **Binary File Comparison** | `binary-file-comparison` | ⏳ Coming Soon | — | — | Free online binary file comparison utility. This tool is planned and coming soon. |
| 438 | **Bulk File Renamer** | `bulk-file-renamer` | ⏳ Coming Soon | — | — | Free online bulk file renamer utility. This tool is planned and coming soon. |
| 420 | **Duplicate File Finder** | `duplicate-file-finder` | ⏳ Coming Soon | — | — | Free online duplicate file finder utility. This tool is planned and coming soon. |
| 440 | **File Joiner** | `file-joiner` | ⏳ Coming Soon | — | — | Free online file joiner utility. This tool is planned and coming soon. |
| 437 | **File Renamer** | `file-renamer` | ⏳ Coming Soon | — | — | Free online file renamer utility. This tool is planned and coming soon. |
| 439 | **File Splitter** | `file-splitter` | ⏳ Coming Soon | — | — | Free online file splitter utility. This tool is planned and coming soon. |
| 428 | **RAR Extractor** | `rar-extractor` | ⏳ Coming Soon | `archive-engine` | `rar_extract` | Free online rar extractor utility. This tool is planned and coming soon. |

### Image Tools (71 tools — 63 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 60 | **AVIF Converter** | `avif-converter` | ✅ Available | `image-engine` | `convert` | Free online avif converter utility. This tool is planned and coming soon. |
| 107 | **Base64 to Image** | `base64-to-image` | ✅ Available | `image-engine` | `from_data_url` | Free online base64 to image utility. This tool is planned and coming soon. |
| 80 | **Black and White Converter** | `black-and-white-converter` | ✅ Available | `image-engine` | `adjust` | Free online black and white converter utility. This tool is planned and coming soon. |
| 105 | **Dominant Color Detector** | `dominant-color-detector` | ✅ Available | `image-engine` | `dominant_color` | Free online dominant color detector utility. This tool is planned and coming soon. |
| 102 | **EXIF Editor** | `exif-editor` | ✅ Available | `image-engine` | `info` | Free online exif editor utility. This tool is planned and coming soon. |
| 101 | **EXIF Remover** | `exif-remover` | ✅ Available | `image-engine` | `strip_metadata` | Free online exif remover utility. This tool is planned and coming soon. |
| 100018 | **GIF Frame Extractor** | `gif-frame-extractor` | ✅ Available | `image-engine` | `gif_frames` | GIF Frame Extractor utility powered by image-engine. |
| 64 | **GIF to PNG** | `gif-to-png` | ✅ Available | `image-engine` | `convert` | Free online gif to png utility. This tool is planned and coming soon. |
| 79 | **Grayscale Converter** | `grayscale-converter` | ✅ Available | `image-engine` | `adjust` | Free online grayscale converter utility. This tool is planned and coming soon. |
| 96 | **ID Photo Maker** | `id-photo-maker` | ✅ Available | `image-engine` | `fit_print_size` | Free online id photo maker utility. This tool is planned and coming soon. |
| 100015 | **Image Average Color** | `image-average-color` | ✅ Available | `image-engine` | `dominant_color` | Image Average Color utility powered by image-engine. |
| 65 | **Image Background Remover** | `image-background-remover` | ✅ Available | `image-engine` | `remove_background` | Free online image background remover utility. This tool is planned and coming soon. |
| 67 | **Image Blur Tool** | `image-blur-tool` | ✅ Available | `image-engine` | `adjust` | Free online image blur tool utility. This tool is planned and coming soon. |
| 85 | **Image Border Generator** | `image-border-generator` | ✅ Available | `image-engine` | `add_border` | Free online image border generator utility. This tool is planned and coming soon. |
| 72 | **Image Brightness Editor** | `image-brightness-editor` | ✅ Available | `image-engine` | `adjust` | Free online image brightness editor utility. This tool is planned and coming soon. |
| 94 | **Image Caption Generator** | `image-caption-generator` | ✅ Available | `image-engine` | `watermark_text` | Free online image caption generator utility. This tool is planned and coming soon. |
| 103 | **Image Color Picker** | `image-color-picker` | ✅ Available | `image-engine` | `color_at` | Free online image color picker utility. This tool is planned and coming soon. |
| 99 | **Image Comparison Tool** | `image-comparison-tool` | ✅ Available | `image-engine` | `difference` | Free online image comparison tool utility. This tool is planned and coming soon. |
| 53 | **Image Compressor** | `image-compressor` | ✅ Available | `image-engine` | `compress` | Free online image compressor utility. This tool is planned and coming soon. |
| 73 | **Image Contrast Editor** | `image-contrast-editor` | ✅ Available | `image-engine` | `adjust` | Free online image contrast editor utility. This tool is planned and coming soon. |
| 54 | **Image Converter** | `image-converter` | ✅ Available | `image-engine` | `convert` | Free online image converter utility. This tool is planned and coming soon. |
| 51 | **Image Cropper** | `image-cropper` | ✅ Available | `image-engine` | `crop` | Free online image cropper utility. This tool is planned and coming soon. |
| 100013 | **Image DPI Changer** | `image-dpi-changer` | ✅ Available | `image-engine` | `set_dpi` | Image DPI Changer utility powered by image-engine. |
| 69 | **Image Denoiser** | `image-denoiser` | ✅ Available | `image-engine` | `adjust` | Free online image denoiser utility. This tool is planned and coming soon. |
| 100016 | **Image Dimensions** | `image-dimensions` | ✅ Available | `image-engine` | `info` | Image Dimensions utility powered by image-engine. |
| 75 | **Image Exposure Editor** | `image-exposure-editor` | ✅ Available | `image-engine` | `adjust` | Free online image exposure editor utility. This tool is planned and coming soon. |
| 78 | **Image Filters** | `image-filters` | ✅ Available | `image-engine` | `adjust` | Free online image filters utility. This tool is planned and coming soon. |
| 88 | **Image Flip** | `image-flip` | ✅ Available | `image-engine` | `flip` | Free online image flip utility. This tool is planned and coming soon. |
| 76 | **Image Hue Editor** | `image-hue-editor` | ✅ Available | `image-engine` | `adjust` | Free online image hue editor utility. This tool is planned and coming soon. |
| 100011 | **Image Invert** | `image-invert` | ✅ Available | `image-engine` | `adjust` | Image Invert utility powered by image-engine. |
| 100 | **Image Metadata Viewer** | `image-metadata-viewer` | ✅ Available | `image-engine` | `info` | Free online image metadata viewer utility. This tool is planned and coming soon. |
| 100021 | **Image Mosaic** | `image-mosaic` | ✅ Available | `image-engine` | `pixel_art` | Image Mosaic utility powered by image-engine. |
| 100020 | **Image Overlay** | `image-overlay` | ✅ Available | `image-engine` | `watermark_image` | Image Overlay utility powered by image-engine. |
| 104 | **Image Palette Extractor** | `image-palette-extractor` | ✅ Available | `colordesign-engine` | `image_palette` | Free online image palette extractor utility. This tool is planned and coming soon. |
| 100014 | **Image Palette Generator** | `image-palette-generator` | ✅ Available | `image-engine` | `palette` | Image Palette Generator utility powered by image-engine. |
| 82 | **Image Pixelator** | `image-pixelator` | ✅ Available | `image-engine` | `pixel_art` | Free online image pixelator utility. This tool is planned and coming soon. |
| 100012 | **Image Remove Border** | `image-remove-border` | ✅ Available | `image-engine` | `trim_border` | Image Remove Border utility powered by image-engine. |
| 52 | **Image Resizer** | `image-resizer` | ✅ Available | `image-engine` | `resize` | Free online image resizer utility. This tool is planned and coming soon. |
| 89 | **Image Rotate** | `image-rotate` | ✅ Available | `image-engine` | `rotate` | Free online image rotate utility. This tool is planned and coming soon. |
| 74 | **Image Saturation Editor** | `image-saturation-editor` | ✅ Available | `image-engine` | `adjust` | Free online image saturation editor utility. This tool is planned and coming soon. |
| 68 | **Image Sharpen Tool** | `image-sharpen-tool` | ✅ Available | `image-engine` | `adjust` | Free online image sharpen tool utility. This tool is planned and coming soon. |
| 100019 | **Image Sprite Generator** | `image-sprite-generator` | ✅ Available | `image-engine` | `sprite` | Image Sprite Generator utility powered by image-engine. |
| 77 | **Image Temperature Editor** | `image-temperature-editor` | ✅ Available | `image-engine` | `adjust` | Free online image temperature editor utility. This tool is planned and coming soon. |
| 83 | **Image Watermark** | `image-watermark` | ✅ Available | `image-engine` | `watermark_text` | Free online image watermark utility. This tool is planned and coming soon. |
| 108 | **Image to ASCII** | `image-to-ascii` | ✅ Available | `image-engine` | `to_ascii` | Free online image to ascii utility. This tool is planned and coming soon. |
| 106 | **Image to Base64** | `image-to-base64` | ✅ Available | `image-engine` | `to_data_url` | Free online image to base64 utility. This tool is planned and coming soon. |
| 109 | **Image to Pixel Art** | `image-to-pixel-art` | ✅ Available | `image-engine` | `pixel_art` | Free online image to pixel art utility. This tool is planned and coming soon. |
| 55 | **JPG to PNG** | `jpg-to-png` | ✅ Available | `image-engine` | `convert` | Free online jpg to png utility. This tool is planned and coming soon. |
| 100017 | **Jpeg To Webp** | `jpeg-to-webp` | ✅ Available | `image-engine` | `convert` | Jpeg To Webp utility powered by image-engine. |
| 93 | **Meme Generator** | `meme-generator` | ✅ Available | `image-engine` | `watermark_text` | Free online meme generator utility. This tool is planned and coming soon. |
| 56 | **PNG to JPG** | `png-to-jpg` | ✅ Available | `image-engine` | `convert` | Free online png to jpg utility. This tool is planned and coming soon. |
| 57 | **PNG to WebP** | `png-to-webp` | ✅ Available | `image-engine` | `convert` | Free online png to webp utility. This tool is planned and coming soon. |
| 95 | **Passport Photo Maker** | `passport-photo-maker` | ✅ Available | `image-engine` | `fit_print_size` | Free online passport photo maker utility. This tool is planned and coming soon. |
| 91 | **Photo Collage Maker** | `photo-collage-maker` | ✅ Available | `image-engine` | `contact_sheet` | Free online photo collage maker utility. This tool is planned and coming soon. |
| 92 | **Photo Grid Maker** | `photo-grid-maker` | ✅ Available | `image-engine` | `contact_sheet` | Free online photo grid maker utility. This tool is planned and coming soon. |
| 97 | **Profile Picture Maker** | `profile-picture-maker` | ✅ Available | `image-engine` | `crop_ratio` | Free online profile picture maker utility. This tool is planned and coming soon. |
| 86 | **Rounded Corner Generator** | `rounded-corner-generator` | ✅ Available | `image-engine` | `round_corners` | Free online rounded corner generator utility. This tool is planned and coming soon. |
| 63 | **SVG to JPG** | `svg-to-jpg` | ✅ Available | `image-engine` | `svg_to_raster` | Free online svg to jpg utility. This tool is planned and coming soon. |
| 62 | **SVG to PNG** | `svg-to-png` | ✅ Available | `image-engine` | `svg_to_raster` | Free online svg to png utility. This tool is planned and coming soon. |
| 81 | **Sepia Filter** | `sepia-filter` | ✅ Available | `image-engine` | `adjust` | Free online sepia filter utility. This tool is planned and coming soon. |
| 98 | **Thumbnail Maker** | `thumbnail-maker` | ✅ Available | `image-engine` | `resize` | Free online thumbnail maker utility. This tool is planned and coming soon. |
| 59 | **WebP to JPG** | `webp-to-jpg` | ✅ Available | `image-engine` | `convert` | Free online webp to jpg utility. This tool is planned and coming soon. |
| 58 | **WebP to PNG** | `webp-to-png` | ✅ Available | `image-engine` | `convert` | Free online webp to png utility. This tool is planned and coming soon. |
| 61 | **HEIC to JPG** | `heic-to-jpg` | ⏳ Coming Soon | `image-engine` | `convert` | Free online heic to jpg utility. This tool is planned and coming soon. |
| 66 | **Image Background Changer** | `image-background-changer` | ⏳ Coming Soon | — | — | Free online image background changer utility. This tool is planned and coming soon. |
| 71 | **Image Colorizer** | `image-colorizer` | ⏳ Coming Soon | — | — | Free online image colorizer utility. This tool is planned and coming soon. |
| 90 | **Image Perspective Correction** | `image-perspective-correction` | ⏳ Coming Soon | — | — | Free online image perspective correction utility. This tool is planned and coming soon. |
| 87 | **Image Shadow Generator** | `image-shadow-generator` | ⏳ Coming Soon | — | — | Free online image shadow generator utility. This tool is planned and coming soon. |
| 70 | **Image Upscaler** | `image-upscaler` | ⏳ Coming Soon | — | — | Free online image upscaler utility. This tool is planned and coming soon. |
| 84 | **Remove Image Watermark** | `remove-image-watermark` | ⏳ Coming Soon | — | — | Free online remove image watermark utility. This tool is planned and coming soon. |
| 110 | **Screenshot Beautifier** | `screenshot-beautifier` | ⏳ Coming Soon | — | — | Free online screenshot beautifier utility. This tool is planned and coming soon. |

### Math Tools (39 tools — 38 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 557 | **Algebra Calculator** | `algebra-calculator` | ✅ Available | `math-engine` | `linear_equation` | Free online algebra calculator utility. This tool is planned and coming soon. |
| 100152 | **Average Calculator** | `average-calculator` | ✅ Available | `math-engine` | `statistics` | Average Calculator utility powered by math-engine. |
| 100158 | **Binary Calculator** | `binary-calculator` | ✅ Available | `math-engine` | `binary_calc` | Binary Calculator utility powered by math-engine. |
| 576 | **Combination Calculator** | `combination-calculator` | ✅ Available | `math-engine` | `combinatorics` | Free online combination calculator utility. This tool is planned and coming soon. |
| 584 | **Coordinate Distance Calculator** | `coordinate-distance-calculator` | ✅ Available | `geo-engine` | `distance` | Free online coordinate distance calculator utility. This tool is planned and coming soon. |
| 563 | **Derivative Calculator** | `derivative-calculator` | ✅ Available | `math-engine` | `derivative` | Free online derivative calculator utility. This tool is planned and coming soon. |
| 562 | **Determinant Calculator** | `determinant-calculator` | ✅ Available | `math-engine` | `matrix` | Free online determinant calculator utility. This tool is planned and coming soon. |
| 558 | **Equation Solver** | `equation-solver` | ✅ Available | `math-engine` | `quadratic` | Free online equation solver utility. This tool is planned and coming soon. |
| 100157 | **Exponent Calculator** | `exponent-calculator` | ✅ Available | `math-engine` | `evaluate` | Exponent Calculator utility powered by math-engine. |
| 578 | **Fibonacci Generator** | `fibonacci-generator` | ✅ Available | `math-engine` | `sequence` | Free online fibonacci generator utility. This tool is planned and coming soon. |
| 556 | **Fraction Calculator** | `fraction-calculator` | ✅ Available | `math-engine` | `fraction` | Free online fraction calculator utility. This tool is planned and coming soon. |
| 569 | **GCD Calculator** | `gcd-calculator` | ✅ Available | `math-engine` | `gcd_lcm` | Free online gcd calculator utility. This tool is planned and coming soon. |
| 583 | **Geometry Calculator** | `geometry-calculator` | ✅ Available | `math-engine` | `triangle` | Free online geometry calculator utility. This tool is planned and coming soon. |
| 579 | **Graph Plotter** | `graph-plotter` | ✅ Available | `math-engine` | `plot` | Free online graph plotter utility. This tool is planned and coming soon. |
| 564 | **Integral Calculator** | `integral-calculator` | ✅ Available | `math-engine` | `integral` | Free online integral calculator utility. This tool is planned and coming soon. |
| 570 | **LCM Calculator** | `lcm-calculator` | ✅ Available | `math-engine` | `gcd_lcm` | Free online lcm calculator utility. This tool is planned and coming soon. |
| 565 | **Limit Calculator** | `limit-calculator` | ✅ Available | `math-engine` | `limit` | Free online limit calculator utility. This tool is planned and coming soon. |
| 100156 | **Logarithm Calculator** | `logarithm-calculator` | ✅ Available | `math-engine` | `evaluate` | Logarithm Calculator utility powered by math-engine. |
| 560 | **Matrix Calculator** | `matrix-calculator` | ✅ Available | `math-engine` | `matrix` | Free online matrix calculator utility. This tool is planned and coming soon. |
| 100154 | **Matrix Inverse** | `matrix-inverse` | ✅ Available | `math-engine` | `matrix` | Matrix Inverse utility powered by math-engine. |
| 561 | **Matrix Multiplication** | `matrix-multiplication` | ✅ Available | `math-engine` | `matrix` | Free online matrix multiplication utility. This tool is planned and coming soon. |
| 573 | **Mean/Median/Mode Calculator** | `mean-median-mode-calculator` | ✅ Available | `math-engine` | `statistics` | Free online mean/median/mode calculator utility. This tool is planned and coming soon. |
| 100159 | **Number To Words** | `number-to-words` | ✅ Available | `math-engine` | `number_to_words` | Number To Words utility powered by math-engine. |
| 571 | **Percentage Change Calculator** | `percentage-change-calculator` | ✅ Available | `math-engine` | `percentage` | Free online percentage change calculator utility. This tool is planned and coming soon. |
| 575 | **Permutation Calculator** | `permutation-calculator` | ✅ Available | `math-engine` | `combinatorics` | Free online permutation calculator utility. This tool is planned and coming soon. |
| 568 | **Prime Factorization** | `prime-factorization` | ✅ Available | `math-engine` | `prime_factorization` | Free online prime factorization utility. This tool is planned and coming soon. |
| 567 | **Prime Number Checker** | `prime-number-checker` | ✅ Available | `math-engine` | `prime_check` | Free online prime number checker utility. This tool is planned and coming soon. |
| 574 | **Probability Calculator** | `probability-calculator` | ✅ Available | `math-engine` | `probability` | Free online probability calculator utility. This tool is planned and coming soon. |
| 582 | **Pythagorean Theorem Calculator** | `pythagorean-theorem-calculator` | ✅ Available | `math-engine` | `triangle` | Free online pythagorean theorem calculator utility. This tool is planned and coming soon. |
| 559 | **Quadratic Equation Solver** | `quadratic-equation-solver` | ✅ Available | `math-engine` | `quadratic` | Free online quadratic equation solver utility. This tool is planned and coming soon. |
| 100151 | **Ratio Calculator** | `ratio-calculator` | ✅ Available | `math-engine` | `ratio` | Ratio Calculator utility powered by math-engine. |
| 566 | **Scientific Notation Converter** | `scientific-notation-converter` | ✅ Available | `math-engine` | `scientific_notation` | Free online scientific notation converter utility. This tool is planned and coming soon. |
| 577 | **Sequence Generator** | `sequence-generator` | ✅ Available | `math-engine` | `sequence` | Free online sequence generator utility. This tool is planned and coming soon. |
| 572 | **Standard Deviation Calculator** | `standard-deviation-calculator` | ✅ Available | `math-engine` | `statistics` | Free online standard deviation calculator utility. This tool is planned and coming soon. |
| 581 | **Triangle Calculator** | `triangle-calculator` | ✅ Available | `math-engine` | `triangle` | Free online triangle calculator utility. This tool is planned and coming soon. |
| 100155 | **Trigonometry Calculator** | `trigonometry-calculator` | ✅ Available | `math-engine` | `trigonometry` | Trigonometry Calculator utility powered by math-engine. |
| 580 | **Unit Circle Calculator** | `unit-circle-calculator` | ✅ Available | `math-engine` | `trigonometry` | Free online unit circle calculator utility. This tool is planned and coming soon. |
| 100153 | **Variance Calculator** | `variance-calculator` | ✅ Available | `math-engine` | `statistics` | Variance Calculator utility powered by math-engine. |
| 585 | **Slope Calculator** | `slope-calculator` | ⏳ Coming Soon | — | — | Free online slope calculator utility. This tool is planned and coming soon. |

### PDF & Documents (75 tools — 63 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 26 | **Add PDF Page Numbers** | `add-pdf-page-numbers` | ✅ Available | `pdf-engine` | `add_page_numbers` | Free online add pdf page numbers utility. This tool is planned and coming soon. |
| 29 | **Add PDF Password** | `add-pdf-password` | ✅ Available | `pdf-engine` | `protect` | Free online add pdf password utility. This tool is planned and coming soon. |
| 27 | **Add PDF Watermark** | `add-pdf-watermark` | ✅ Available | `pdf-engine` | `add_watermark` | Free online add pdf watermark utility. This tool is planned and coming soon. |
| 37 | **Annotate PDF** | `annotate-pdf` | ✅ Available | `pdf-engine` | `annotate` | Free online annotate pdf utility. This tool is planned and coming soon. |
| 41 | **Change PDF Page Size** | `change-pdf-page-size` | ✅ Available | `pdf-engine` | `resize_pages` | Free online change pdf page size utility. This tool is planned and coming soon. |
| 3 | **Compress PDF** | `compress-pdf` | ✅ Available | `pdf-engine` | `compress` | Free online compress pdf utility. This tool is planned and coming soon. |
| 39 | **Crop PDF** | `crop-pdf` | ✅ Available | `pdf-engine` | `crop` | Free online crop pdf utility. This tool is planned and coming soon. |
| 23 | **Delete PDF Pages** | `delete-pdf-pages` | ✅ Available | `pdf-engine` | `delete_pages` | Free online delete pdf pages utility. This tool is planned and coming soon. |
| 100276 | **Docx Metadata Viewer** | `docx-metadata-viewer` | ✅ Available | `document-engine` | `inspect` | Docx Metadata Viewer utility powered by document-engine. |
| 100277 | **Docx Template Fill** | `docx-template-fill` | ✅ Available | `document-engine` | `fill_template` | Docx Template Fill utility powered by document-engine. |
| 100274 | **Docx To Html** | `docx-to-html` | ✅ Available | `document-engine` | `to_html` | Docx To Html utility powered by document-engine. |
| 100275 | **Docx To Text** | `docx-to-text` | ✅ Available | `document-engine` | `to_text` | Docx To Text utility powered by document-engine. |
| 38 | **Draw on PDF** | `draw-on-pdf` | ✅ Available | `pdf-engine` | `annotate` | Free online draw on pdf utility. This tool is planned and coming soon. |
| 25 | **Duplicate PDF Pages** | `duplicate-pdf-pages` | ✅ Available | `pdf-engine` | `duplicate_pages` | Free online duplicate pdf pages utility. This tool is planned and coming soon. |
| 100246 | **Ebook Cover Extractor** | `ebook-cover-extractor` | ✅ Available | `document-engine` | `epub_cover` | Ebook Cover Extractor utility powered by document-engine. |
| 100247 | **Ebook Format Converter** | `ebook-format-converter` | ✅ Available | `document-engine` | `epub_convert` | Ebook Format Converter utility powered by document-engine. |
| 100245 | **Ebook Metadata Editor** | `ebook-metadata-editor` | ✅ Available | `document-engine` | `epub_set_metadata` | Ebook Metadata Editor utility powered by document-engine. |
| 100244 | **Epub To Html** | `epub-to-html` | ✅ Available | `document-engine` | `epub_to_html` | Epub To Html utility powered by document-engine. |
| 100248 | **Epub Validator** | `epub-validator` | ✅ Available | `document-engine` | `epub_validate` | Epub Validator utility powered by document-engine. |
| 24 | **Extract PDF Pages** | `extract-pdf-pages` | ✅ Available | `pdf-engine` | `extract_pages` | Free online extract pdf pages utility. This tool is planned and coming soon. |
| 34 | **Fill PDF Forms** | `fill-pdf-forms` | ✅ Available | `pdf-engine` | `fill_form` | Free online fill pdf forms utility. This tool is planned and coming soon. |
| 49 | **Flatten PDF Forms** | `flatten-pdf-forms` | ✅ Available | `pdf-engine` | `fill_form` | Free online flatten pdf forms utility. This tool is planned and coming soon. |
| 36 | **Highlight PDF** | `highlight-pdf` | ✅ Available | `pdf-engine` | `annotate` | Free online highlight pdf utility. This tool is planned and coming soon. |
| 11 | **JPG to PDF** | `jpg-to-pdf` | ✅ Available | `pdf-engine` | `from_images` | Free online jpg to pdf utility. This tool is planned and coming soon. |
| 18 | **Markdown to PDF** | `markdown-to-pdf` | ✅ Available | `pdf-engine` | `from_markdown` | Free online markdown to pdf utility. This tool is planned and coming soon. |
| 1 | **Merge PDF** | `merge-pdf` | ✅ Available | `pdf-engine` | `merge` | Free online merge pdf utility. This tool is planned and coming soon. |
| 100278 | **Odt To Docx** | `odt-to-docx` | ✅ Available | `document-engine` | `convert` | Odt To Docx utility powered by document-engine. |
| 100000 | **PDF Add Header Footer** | `pdf-add-header-footer` | ✅ Available | `pdf-engine` | `add_header_footer` | PDF Add Header Footer utility powered by pdf-engine. |
| 100005 | **PDF Bookmark Editor** | `pdf-bookmark-editor` | ✅ Available | `pdf-engine` | `set_bookmarks` | PDF Bookmark Editor utility powered by pdf-engine. |
| 47 | **PDF Comparison** | `pdf-comparison` | ✅ Available | `pdf-engine` | `compare` | Free online pdf comparison utility. This tool is planned and coming soon. |
| 100004 | **PDF Form Field Inspector** | `pdf-form-field-inspector` | ✅ Available | `pdf-engine` | `form_fields` | PDF Form Field Inspector utility powered by pdf-engine. |
| 46 | **PDF Image Extractor** | `pdf-image-extractor` | ✅ Available | `pdf-engine` | `extract_images` | Free online pdf image extractor utility. This tool is planned and coming soon. |
| 100008 | **PDF Linearize** | `pdf-linearize` | ✅ Available | `pdf-engine` | `linearize` | PDF Linearize utility powered by pdf-engine. |
| 42 | **PDF Metadata Editor** | `pdf-metadata-editor` | ✅ Available | `pdf-engine` | `set_metadata` | Free online pdf metadata editor utility. This tool is planned and coming soon. |
| 43 | **PDF Metadata Viewer** | `pdf-metadata-viewer` | ✅ Available | `pdf-engine` | `metadata` | Free online pdf metadata viewer utility. This tool is planned and coming soon. |
| 100006 | **PDF Outline Extractor** | `pdf-outline-extractor` | ✅ Available | `pdf-engine` | `outlines` | PDF Outline Extractor utility powered by pdf-engine. |
| 100002 | **PDF Overlay** | `pdf-overlay` | ✅ Available | `pdf-engine` | `overlay` | PDF Overlay utility powered by pdf-engine. |
| 100007 | **PDF Page Size Checker** | `pdf-page-size-checker` | ✅ Available | `pdf-engine` | `page_sizes` | PDF Page Size Checker utility powered by pdf-engine. |
| 100001 | **PDF Permission Inspector** | `pdf-permission-inspector` | ✅ Available | `pdf-engine` | `permissions` | PDF Permission Inspector utility powered by pdf-engine. |
| 100009 | **PDF Redact** | `pdf-redact` | ✅ Available | `pdf-engine` | `redact` | PDF Redact utility powered by pdf-engine. |
| 48 | **PDF Repair** | `pdf-repair` | ✅ Available | `pdf-engine` | `repair` | Free online pdf repair utility. This tool is planned and coming soon. |
| 100010 | **PDF Signature Inspector** | `pdf-signature-inspector` | ✅ Available | `pdf-engine` | `signatures` | PDF Signature Inspector utility powered by pdf-engine. |
| 100003 | **PDF Stamp** | `pdf-stamp` | ✅ Available | `pdf-engine` | `stamp` | PDF Stamp utility powered by pdf-engine. |
| 45 | **PDF Text Extractor** | `pdf-text-extractor` | ✅ Available | `pdf-engine` | `extract_text` | Free online pdf text extractor utility. This tool is planned and coming soon. |
| 19 | **PDF to EPUB** | `pdf-to-epub` | ✅ Available | `document-engine` | `epub_to_html` | Free online pdf to epub utility. This tool is planned and coming soon. |
| 50 | **PDF to Grayscale** | `pdf-to-grayscale` | ✅ Available | `pdf-engine` | `grayscale` | Free online pdf to grayscale utility. This tool is planned and coming soon. |
| 16 | **PDF to HTML** | `pdf-to-html` | ✅ Available | `pdf-engine` | `from_html` | Free online pdf to html utility. This tool is planned and coming soon. |
| 17 | **PDF to Markdown** | `pdf-to-markdown` | ✅ Available | `pdf-engine` | `to_markdown` | Free online pdf to markdown utility. This tool is planned and coming soon. |
| 15 | **PDF to Text** | `pdf-to-text` | ✅ Available | `pdf-engine` | `extract_text` | Free online pdf to text utility. This tool is planned and coming soon. |
| 4 | **PDF to Word** | `pdf-to-word` | ✅ Available | `pdf-engine` | `to_docx` | Free online pdf to word utility. This tool is planned and coming soon. |
| 13 | **PNG to PDF** | `png-to-pdf` | ✅ Available | `pdf-engine` | `from_images` | Free online png to pdf utility. This tool is planned and coming soon. |
| 100282 | **Pptx Metadata Viewer** | `pptx-metadata-viewer` | ✅ Available | `document-engine` | `pptx_slides` | Pptx Metadata Viewer utility powered by document-engine. |
| 100281 | **Pptx To PDF** | `pptx-to-pdf` | ✅ Available | `document-engine` | `pptx_slides` | Pptx To PDF utility powered by document-engine. |
| 32 | **Protect PDF** | `protect-pdf` | ✅ Available | `pdf-engine` | `protect` | Free online protect pdf utility. This tool is planned and coming soon. |
| 100280 | **RTF To Html** | `rtf-to-html` | ✅ Available | `document-engine` | `to_html` | RTF To Html utility powered by document-engine. |
| 100279 | **RTF To Text** | `rtf-to-text` | ✅ Available | `document-engine` | `to_text` | RTF To Text utility powered by document-engine. |
| 22 | **Rearrange PDF Pages** | `rearrange-pdf-pages` | ✅ Available | `pdf-engine` | `reorder` | Free online rearrange pdf pages utility. This tool is planned and coming soon. |
| 30 | **Remove PDF Password** | `remove-pdf-password` | ✅ Available | `pdf-engine` | `unlock` | Free online remove pdf password utility. This tool is planned and coming soon. |
| 40 | **Resize PDF Pages** | `resize-pdf-pages` | ✅ Available | `pdf-engine` | `resize_pages` | Free online resize pdf pages utility. This tool is planned and coming soon. |
| 21 | **Rotate PDF** | `rotate-pdf` | ✅ Available | `pdf-engine` | `rotate` | Free online rotate pdf utility. This tool is planned and coming soon. |
| 2 | **Split PDF** | `split-pdf` | ✅ Available | `pdf-engine` | `split` | Free online split pdf utility. This tool is planned and coming soon. |
| 31 | **Unlock PDF** | `unlock-pdf` | ✅ Available | `pdf-engine` | `unlock` | Free online unlock pdf utility. This tool is planned and coming soon. |
| 5 | **Word to PDF** | `word-to-pdf` | ✅ Available | `document-engine` | `to_html` | Free online word to pdf utility. This tool is planned and coming soon. |
| 35 | **Edit PDF Text** | `edit-pdf-text` | ⏳ Coming Soon | — | — | Free online edit pdf text utility. This tool is planned and coming soon. |
| 7 | **Excel to PDF** | `excel-to-pdf` | ⏳ Coming Soon | — | — | Free online excel to pdf utility. This tool is planned and coming soon. |
| 44 | **PDF OCR** | `pdf-ocr` | ⏳ Coming Soon | — | — | Free online pdf ocr utility. This tool is planned and coming soon. |
| 20 | **PDF to CSV** | `pdf-to-csv` | ⏳ Coming Soon | — | — | Free online pdf to csv utility. This tool is planned and coming soon. |
| 6 | **PDF to Excel** | `pdf-to-excel` | ⏳ Coming Soon | — | — | Free online pdf to excel utility. This tool is planned and coming soon. |
| 10 | **PDF to JPG** | `pdf-to-jpg` | ⏳ Coming Soon | `pdf-engine` | `to_images` | Free online pdf to jpg utility. This tool is planned and coming soon. |
| 12 | **PDF to PNG** | `pdf-to-png` | ⏳ Coming Soon | `pdf-engine` | `to_images` | Free online pdf to png utility. This tool is planned and coming soon. |
| 8 | **PDF to PowerPoint** | `pdf-to-powerpoint` | ⏳ Coming Soon | — | — | Free online pdf to powerpoint utility. This tool is planned and coming soon. |
| 14 | **PDF to WebP** | `pdf-to-webp` | ⏳ Coming Soon | `pdf-engine` | `to_images` | Free online pdf to webp utility. This tool is planned and coming soon. |
| 9 | **PowerPoint to PDF** | `powerpoint-to-pdf` | ⏳ Coming Soon | — | — | Free online powerpoint to pdf utility. This tool is planned and coming soon. |
| 28 | **Remove PDF Watermark** | `remove-pdf-watermark` | ⏳ Coming Soon | — | — | Free online remove pdf watermark utility. This tool is planned and coming soon. |
| 33 | **Sign PDF** | `sign-pdf` | ⏳ Coming Soon | — | — | Free online sign pdf utility. This tool is planned and coming soon. |

### Productivity (39 tools — 19 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 100295 | **Break Even Analysis** | `break-even-analysis` | ✅ Available | `business-engine` | `break_even` | Break Even Analysis utility powered by business-engine. |
| 100296 | **Business Valuation Estimator** | `business-valuation-estimator` | ✅ Available | `business-engine` | `valuation` | Business Valuation Estimator utility powered by business-engine. |
| 398 | **Calendar Generator** | `calendar-generator` | ✅ Available | `datetime-engine` | `calendar` | Free online calendar generator utility. This tool is planned and coming soon. |
| 100294 | **Cash Flow Forecast** | `cash-flow-forecast` | ✅ Available | `business-engine` | `cash_flow` | Cash Flow Forecast utility powered by business-engine. |
| 100292 | **Commission Calculator** | `commission-calculator` | ✅ Available | `business-engine` | `commission` | Commission Calculator utility powered by business-engine. |
| 100290 | **Discount Pricing Calculator** | `discount-pricing-calculator` | ✅ Available | `business-engine` | `discount_pricing` | Discount Pricing Calculator utility powered by business-engine. |
| 100288 | **Inventory Turnover Calculator** | `inventory-turnover-calculator` | ✅ Available | `business-engine` | `inventory_turnover` | Inventory Turnover Calculator utility powered by business-engine. |
| 100283 | **Invoice Generator** | `invoice-generator` | ✅ Available | `business-engine` | `document` | Invoice Generator utility powered by business-engine. |
| 100287 | **Markup Margin Calculator** | `markup-margin-calculator` | ✅ Available | `business-engine` | `markup_margin` | Markup Margin Calculator utility powered by business-engine. |
| 100293 | **Payroll Estimator** | `payroll-estimator` | ✅ Available | `business-engine` | `payroll` | Payroll Estimator utility powered by business-engine. |
| 393 | **Pomodoro Timer** | `pomodoro-timer` | ✅ Available | `datetime-engine` | `pomodoro` | Free online pomodoro timer utility. This tool is planned and coming soon. |
| 100286 | **Profit Loss Calculator** | `profit-loss-calculator` | ✅ Available | `business-engine` | `profit_loss` | Profit Loss Calculator utility powered by business-engine. |
| 100285 | **Purchase Order Generator** | `purchase-order-generator` | ✅ Available | `business-engine` | `document` | Purchase Order Generator utility powered by business-engine. |
| 100284 | **Quotation Generator** | `quotation-generator` | ✅ Available | `business-engine` | `document` | Quotation Generator utility powered by business-engine. |
| 406 | **Random Number Generator** | `random-number-generator` | ✅ Available | `math-engine` | `random_numbers` | Free online random number generator utility. This tool is planned and coming soon. |
| 100291 | **Sales Tax Calculator** | `sales-tax-calculator` | ✅ Available | `business-engine` | `sales_tax` | Sales Tax Calculator utility powered by business-engine. |
| 100289 | **Stock Reorder Calculator** | `stock-reorder-calculator` | ✅ Available | `business-engine` | `reorder` | Stock Reorder Calculator utility powered by business-engine. |
| 394 | **Stopwatch** | `stopwatch` | ✅ Available | `datetime-engine` | `format` | Free online stopwatch utility. This tool is planned and coming soon. |
| 100297 | **Unit Economics Calculator** | `unit-economics-calculator` | ✅ Available | `business-engine` | `unit_economics` | Unit Economics Calculator utility powered by business-engine. |
| 396 | **Alarm Clock** | `alarm-clock` | ⏳ Coming Soon | — | — | Free online alarm clock utility. This tool is planned and coming soon. |
| 404 | **Clipboard Manager** | `clipboard-manager` | ⏳ Coming Soon | — | — | Free online clipboard manager utility. This tool is planned and coming soon. |
| 410 | **Coin Flip** | `coin-flip` | ⏳ Coming Soon | — | — | Free online coin flip utility. This tool is planned and coming soon. |
| 395 | **Countdown Timer** | `countdown-timer` | ⏳ Coming Soon | — | — | Free online countdown timer utility. This tool is planned and coming soon. |
| 400 | **Daily Planner** | `daily-planner` | ⏳ Coming Soon | — | — | Free online daily planner utility. This tool is planned and coming soon. |
| 408 | **Decision Wheel** | `decision-wheel` | ⏳ Coming Soon | — | — | Free online decision wheel utility. This tool is planned and coming soon. |
| 411 | **Dice Roller** | `dice-roller` | ⏳ Coming Soon | — | — | Free online dice roller utility. This tool is planned and coming soon. |
| 399 | **Event Planner** | `event-planner` | ⏳ Coming Soon | — | — | Free online event planner utility. This tool is planned and coming soon. |
| 413 | **Group Maker** | `group-maker` | ⏳ Coming Soon | — | — | Free online group maker utility. This tool is planned and coming soon. |
| 401 | **Habit Tracker** | `habit-tracker` | ⏳ Coming Soon | — | — | Free online habit tracker utility. This tool is planned and coming soon. |
| 392 | **Kanban Board** | `kanban-board` | ⏳ Coming Soon | — | — | Free online kanban board utility. This tool is planned and coming soon. |
| 405 | **Meeting Agenda Generator** | `meeting-agenda-generator` | ⏳ Coming Soon | — | — | Free online meeting agenda generator utility. This tool is planned and coming soon. |
| 402 | **Notes App** | `notes-app` | ⏳ Coming Soon | — | — | Free online notes app utility. This tool is planned and coming soon. |
| 414 | **Raffle Picker** | `raffle-picker` | ⏳ Coming Soon | — | — | Free online raffle picker utility. This tool is planned and coming soon. |
| 407 | **Random Name Picker** | `random-name-picker` | ⏳ Coming Soon | — | — | Free online random name picker utility. This tool is planned and coming soon. |
| 409 | **Spin the Wheel** | `spin-the-wheel` | ⏳ Coming Soon | — | — | Free online spin the wheel utility. This tool is planned and coming soon. |
| 403 | **Sticky Notes** | `sticky-notes` | ⏳ Coming Soon | — | — | Free online sticky notes utility. This tool is planned and coming soon. |
| 412 | **Team Randomizer** | `team-randomizer` | ⏳ Coming Soon | — | — | Free online team randomizer utility. This tool is planned and coming soon. |
| 391 | **To-do List** | `to-do-list` | ⏳ Coming Soon | — | — | Free online to-do list utility. This tool is planned and coming soon. |
| 397 | **World Clock** | `world-clock` | ⏳ Coming Soon | — | — | Free online world clock utility. This tool is planned and coming soon. |

### SEO & Website (25 tools — 5 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 550 | **HTML Source Viewer** | `html-source-viewer` | ✅ Available | `developer-engine` | `html_format` | Free online html source viewer utility. This tool is planned and coming soon. |
| 543 | **Heading Hierarchy Checker** | `heading-hierarchy-checker` | ✅ Available | `developer-engine` | `lint_markdown` | Free online heading hierarchy checker utility. This tool is planned and coming soon. |
| 548 | **QR Code Generator for URLs** | `qr-code-generator-for-urls` | ✅ Available | `qrbarcode-engine` | `qr_url` | Free online qr code generator for urls utility. This tool is planned and coming soon. |
| 546 | **URL Slug Generator** | `url-slug-generator` | ✅ Available | `text-engine` | `slug` | Free online url slug generator utility. This tool is planned and coming soon. |
| 542 | **Word Count Analyzer** | `word-count-analyzer` | ✅ Available | `text-engine` | `stats` | Free online word count analyzer utility. This tool is planned and coming soon. |
| 544 | **Broken Link Checker** | `broken-link-checker` | ⏳ Coming Soon | — | — | Free online broken link checker utility. This tool is planned and coming soon. |
| 539 | **Canonical URL Checker** | `canonical-url-checker` | ⏳ Coming Soon | — | — | Free online canonical url checker utility. This tool is planned and coming soon. |
| 553 | **DNS Checker** | `dns-checker` | ⏳ Coming Soon | — | — | Free online dns checker utility. This tool is planned and coming soon. |
| 552 | **Favicon Checker** | `favicon-checker` | ⏳ Coming Soon | — | — | Free online favicon checker utility. This tool is planned and coming soon. |
| 541 | **Keyword Density Checker** | `keyword-density-checker` | ⏳ Coming Soon | — | — | Free online keyword density checker utility. This tool is planned and coming soon. |
| 555 | **Lighthouse Report Viewer** | `lighthouse-report-viewer` | ⏳ Coming Soon | — | — | Free online lighthouse report viewer utility. This tool is planned and coming soon. |
| 532 | **Meta Tag Analyzer** | `meta-tag-analyzer` | ⏳ Coming Soon | — | — | Free online meta tag analyzer utility. This tool is planned and coming soon. |
| 531 | **Meta Tag Generator** | `meta-tag-generator` | ⏳ Coming Soon | — | — | Free online meta tag generator utility. This tool is planned and coming soon. |
| 533 | **Open Graph Preview** | `open-graph-preview` | ⏳ Coming Soon | — | — | Free online open graph preview utility. This tool is planned and coming soon. |
| 549 | **Page Size Analyzer** | `page-size-analyzer` | ⏳ Coming Soon | — | — | Free online page size analyzer utility. This tool is planned and coming soon. |
| 545 | **Redirect Checker** | `redirect-checker` | ⏳ Coming Soon | — | — | Free online redirect checker utility. This tool is planned and coming soon. |
| 536 | **Robots.txt Tester** | `robots-txt-tester` | ⏳ Coming Soon | — | — | Free online robots.txt tester utility. This tool is planned and coming soon. |
| 540 | **SERP Snippet Preview** | `serp-snippet-preview` | ⏳ Coming Soon | — | — | Free online serp snippet preview utility. This tool is planned and coming soon. |
| 537 | **Schema Markup Generator** | `schema-markup-generator` | ⏳ Coming Soon | — | — | Free online schema markup generator utility. This tool is planned and coming soon. |
| 535 | **Sitemap Generator** | `sitemap-generator` | ⏳ Coming Soon | — | — | Free online sitemap generator utility. This tool is planned and coming soon. |
| 538 | **Structured Data Validator** | `structured-data-validator` | ⏳ Coming Soon | — | — | Free online structured data validator utility. This tool is planned and coming soon. |
| 534 | **Twitter Card Preview** | `twitter-card-preview` | ⏳ Coming Soon | — | — | Free online twitter card preview utility. This tool is planned and coming soon. |
| 547 | **UTM Builder** | `utm-builder` | ⏳ Coming Soon | — | — | Free online utm builder utility. This tool is planned and coming soon. |
| 551 | **Website Screenshot Tool** | `website-screenshot-tool` | ⏳ Coming Soon | — | — | Free online website screenshot tool utility. This tool is planned and coming soon. |
| 554 | **Website Uptime Monitor** | `website-uptime-monitor` | ⏳ Coming Soon | — | — | Free online website uptime monitor utility. This tool is planned and coming soon. |

### Security & Network (58 tools — 25 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 459 | **AES Decryption Tool** | `aes-decryption-tool` | ✅ Available | `crypto-engine` | `decrypt` | Free online aes decryption tool utility. This tool is planned and coming soon. |
| 458 | **AES Encryption Tool** | `aes-encryption-tool` | ✅ Available | `crypto-engine` | `encrypt` | Free online aes encryption tool utility. This tool is planned and coming soon. |
| 100116 | **Argon2 Hash Generator** | `argon2-hash-generator` | ✅ Available | `crypto-engine` | `argon2_hash` | Argon2 Hash Generator utility powered by crypto-engine. |
| 100115 | **Bcrypt Hash Generator** | `bcrypt-hash-generator` | ✅ Available | `crypto-engine` | `bcrypt_hash` | Bcrypt Hash Generator utility powered by crypto-engine. |
| 100112 | **Blake3 Generator** | `blake3-generator` | ✅ Available | `crypto-engine` | `hash` | Blake3 Generator utility powered by crypto-engine. |
| 461 | **Certificate Viewer** | `certificate-viewer` | ✅ Available | `crypto-engine` | `certificate_decode` | Free online certificate viewer utility. This tool is planned and coming soon. |
| 466 | **EXIF Data Remover** | `exif-data-remover` | ✅ Available | `image-engine` | `strip_metadata` | Free online exif data remover utility. This tool is planned and coming soon. |
| 454 | **Hash Calculator** | `hash-calculator` | ✅ Available | `crypto-engine` | `hash` | Free online hash calculator utility. This tool is planned and coming soon. |
| 100113 | **Hmac Generator** | `hmac-generator` | ✅ Available | `crypto-engine` | `hmac` | Hmac Generator utility powered by crypto-engine. |
| 100114 | **Hmac Verifier** | `hmac-verifier` | ✅ Available | `crypto-engine` | `hmac_verify` | Hmac Verifier utility powered by crypto-engine. |
| 463 | **JWT Inspector** | `jwt-inspector` | ✅ Available | `crypto-engine` | `jwt_verify` | Free online jwt inspector utility. This tool is planned and coming soon. |
| 467 | **Metadata Stripper** | `metadata-stripper` | ✅ Available | `image-engine` | `strip_metadata` | Free online metadata stripper utility. This tool is planned and coming soon. |
| 451 | **Password Generator** | `password-generator` | ✅ Available | `crypto-engine` | `password` | Free online password generator utility. This tool is planned and coming soon. |
| 452 | **Password Strength Tester** | `password-strength-tester` | ✅ Available | `crypto-engine` | `password_strength` | Free online password strength tester utility. This tool is planned and coming soon. |
| 100120 | **Pem Parser** | `pem-parser` | ✅ Available | `crypto-engine` | `pem_parse` | Pem Parser utility powered by crypto-engine. |
| 465 | **QR-based 2FA Generator** | `qr-based-2fa-generator` | ✅ Available | `crypto-engine` | `totp_uri` | Free online qr-based 2fa generator utility. This tool is planned and coming soon. |
| 460 | **RSA Key Pair Generator** | `rsa-key-pair-generator` | ✅ Available | `crypto-engine` | `rsa_generate` | Free online rsa key pair generator utility. This tool is planned and coming soon. |
| 100119 | **RSA Sign Verify** | `rsa-sign-verify` | ✅ Available | `crypto-engine` | `rsa_sign` | RSA Sign Verify utility powered by crypto-engine. |
| 455 | **SHA Checksum Verifier** | `sha-checksum-verifier` | ✅ Available | `crypto-engine` | `hash` | Free online sha checksum verifier utility. This tool is planned and coming soon. |
| 462 | **SSL Certificate Decoder** | `ssl-certificate-decoder` | ✅ Available | `crypto-engine` | `certificate_decode` | Free online ssl certificate decoder utility. This tool is planned and coming soon. |
| 100111 | **Sha3 Generator** | `sha3-generator` | ✅ Available | `crypto-engine` | `hash` | Sha3 Generator utility powered by crypto-engine. |
| 464 | **TOTP Authenticator** | `totp-authenticator` | ✅ Available | `crypto-engine` | `totp` | Free online totp authenticator utility. This tool is planned and coming soon. |
| 457 | **Text Decryption Tool** | `text-decryption-tool` | ✅ Available | `crypto-engine` | `decrypt` | Free online text decryption tool utility. This tool is planned and coming soon. |
| 456 | **Text Encryption Tool** | `text-encryption-tool` | ✅ Available | `crypto-engine` | `encrypt` | Free online text encryption tool utility. This tool is planned and coming soon. |
| 489 | **User Agent Parser** | `user-agent-parser` | ✅ Available | `developer-engine` | `user_agent` | Free online user agent parser utility. This tool is planned and coming soon. |
| 497 | **API Request Builder** | `api-request-builder` | ⏳ Coming Soon | — | — | Free online api request builder utility. This tool is planned and coming soon. |
| 480 | **CIDR Calculator** | `cidr-calculator` | ⏳ Coming Soon | — | — | Free online cidr calculator utility. This tool is planned and coming soon. |
| 470 | **CSP Header Generator** | `csp-header-generator` | ⏳ Coming Soon | — | — | Free online csp header generator utility. This tool is planned and coming soon. |
| 473 | **Cookie Policy Generator** | `cookie-policy-generator` | ⏳ Coming Soon | — | — | Free online cookie policy generator utility. This tool is planned and coming soon. |
| 483 | **DNS Propagation Checker** | `dns-propagation-checker` | ⏳ Coming Soon | — | — | Free online dns propagation checker utility. This tool is planned and coming soon. |
| 482 | **DNS Record Lookup** | `dns-record-lookup` | ⏳ Coming Soon | — | — | Free online dns record lookup utility. This tool is planned and coming soon. |
| 485 | **Domain Availability Checker** | `domain-availability-checker` | ⏳ Coming Soon | — | — | Free online domain availability checker utility. This tool is planned and coming soon. |
| 493 | **Download Speed Tester** | `download-speed-tester` | ⏳ Coming Soon | — | — | Free online download speed tester utility. This tool is planned and coming soon. |
| 469 | **HTTP Header Analyzer** | `http-header-analyzer` | ⏳ Coming Soon | — | — | Free online http header analyzer utility. This tool is planned and coming soon. |
| 487 | **HTTP Header Viewer** | `http-header-viewer` | ⏳ Coming Soon | — | — | Free online http header viewer utility. This tool is planned and coming soon. |
| 486 | **HTTP Status Checker** | `http-status-checker` | ⏳ Coming Soon | — | — | Free online http status checker utility. This tool is planned and coming soon. |
| 476 | **IP Address Lookup** | `ip-address-lookup` | ⏳ Coming Soon | — | — | Free online ip address lookup utility. This tool is planned and coming soon. |
| 500 | **IP Subnet Range Calculator** | `ip-subnet-range-calculator` | ⏳ Coming Soon | — | — | Free online ip subnet range calculator utility. This tool is planned and coming soon. |
| 477 | **IPv4 Calculator** | `ipv4-calculator` | ⏳ Coming Soon | — | — | Free online ipv4 calculator utility. This tool is planned and coming soon. |
| 478 | **IPv6 Calculator** | `ipv6-calculator` | ⏳ Coming Soon | — | — | Free online ipv6 calculator utility. This tool is planned and coming soon. |
| 495 | **Latency Tester** | `latency-tester` | ⏳ Coming Soon | — | — | Free online latency tester utility. This tool is planned and coming soon. |
| 481 | **MAC Address Lookup** | `mac-address-lookup` | ⏳ Coming Soon | — | — | Free online mac address lookup utility. This tool is planned and coming soon. |
| 453 | **Passphrase Generator** | `passphrase-generator` | ⏳ Coming Soon | — | — | Free online passphrase generator utility. This tool is planned and coming soon. |
| 475 | **Password Leak Checker** | `password-leak-checker` | ⏳ Coming Soon | — | — | Free online password leak checker utility. This tool is planned and coming soon. |
| 491 | **Ping Tester** | `ping-tester` | ⏳ Coming Soon | — | — | Free online ping tester utility. This tool is planned and coming soon. |
| 490 | **Port Reference Lookup** | `port-reference-lookup` | ⏳ Coming Soon | — | — | Free online port reference lookup utility. This tool is planned and coming soon. |
| 472 | **Privacy Policy Generator** | `privacy-policy-generator` | ⏳ Coming Soon | — | — | Free online privacy policy generator utility. This tool is planned and coming soon. |
| 498 | **REST API Tester** | `rest-api-tester` | ⏳ Coming Soon | — | — | Free online rest api tester utility. This tool is planned and coming soon. |
| 474 | **Robots.txt Generator** | `robots-txt-generator` | ⏳ Coming Soon | — | — | Free online robots.txt generator utility. This tool is planned and coming soon. |
| 471 | **Security Header Checker** | `security-header-checker` | ⏳ Coming Soon | — | — | Free online security header checker utility. This tool is planned and coming soon. |
| 479 | **Subnet Calculator** | `subnet-calculator` | ⏳ Coming Soon | — | — | Free online subnet calculator utility. This tool is planned and coming soon. |
| 492 | **Traceroute** | `traceroute` | ⏳ Coming Soon | — | — | Free online traceroute utility. This tool is planned and coming soon. |
| 488 | **URL Redirect Checker** | `url-redirect-checker` | ⏳ Coming Soon | — | — | Free online url redirect checker utility. This tool is planned and coming soon. |
| 468 | **URL Safety Checker** | `url-safety-checker` | ⏳ Coming Soon | — | — | Free online url safety checker utility. This tool is planned and coming soon. |
| 494 | **Upload Speed Tester** | `upload-speed-tester` | ⏳ Coming Soon | — | — | Free online upload speed tester utility. This tool is planned and coming soon. |
| 484 | **WHOIS Lookup** | `whois-lookup` | ⏳ Coming Soon | — | — | Free online whois lookup utility. This tool is planned and coming soon. |
| 496 | **WebSocket Tester** | `websocket-tester` | ⏳ Coming Soon | — | — | Free online websocket tester utility. This tool is planned and coming soon. |
| 499 | **Webhook Tester** | `webhook-tester` | ⏳ Coming Soon | — | — | Free online webhook tester utility. This tool is planned and coming soon. |

### Social Media (20 tools — 8 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 620 | **Instagram Post Resizer** | `instagram-post-resizer` | ✅ Available | `colordesign-engine` | `social_dimensions` | Free online instagram post resizer utility. This tool is planned and coming soon. |
| 628 | **Meme Maker** | `meme-maker` | ✅ Available | `image-engine` | `watermark_text` | Free online meme maker utility. This tool is planned and coming soon. |
| 627 | **Profile Picture Cropper** | `profile-picture-cropper` | ✅ Available | `image-engine` | `crop_ratio` | Free online profile picture cropper utility. This tool is planned and coming soon. |
| 626 | **Social Media Image Resizer** | `social-media-image-resizer` | ✅ Available | `colordesign-engine` | `social_dimensions` | Free online social media image resizer utility. This tool is planned and coming soon. |
| 630 | **Social Media Preview Generator** | `social-media-preview-generator` | ✅ Available | `image-engine` | `og_image` | Free online social media preview generator utility. This tool is planned and coming soon. |
| 618 | **Video Caption Formatter** | `video-caption-formatter` | ✅ Available | `video-engine` | `subtitle_convert` | Free online video caption formatter utility. This tool is planned and coming soon. |
| 624 | **X/Twitter Character Counter** | `x-twitter-character-counter` | ✅ Available | `text-engine` | `stats` | Free online x/twitter character counter utility. This tool is planned and coming soon. |
| 612 | **YouTube Thumbnail Maker** | `youtube-thumbnail-maker` | ✅ Available | `image-engine` | `og_image` | Free online youtube thumbnail maker utility. This tool is planned and coming soon. |
| 629 | **Emoji Picker** | `emoji-picker` | ⏳ Coming Soon | — | — | Free online emoji picker utility. This tool is planned and coming soon. |
| 619 | **Instagram Bio Generator** | `instagram-bio-generator` | ⏳ Coming Soon | — | — | Free online instagram bio generator utility. This tool is planned and coming soon. |
| 621 | **Instagram Hashtag Generator** | `instagram-hashtag-generator` | ⏳ Coming Soon | — | — | Free online instagram hashtag generator utility. This tool is planned and coming soon. |
| 622 | **LinkedIn Headline Generator** | `linkedin-headline-generator` | ⏳ Coming Soon | — | — | Free online linkedin headline generator utility. This tool is planned and coming soon. |
| 623 | **LinkedIn Post Formatter** | `linkedin-post-formatter` | ⏳ Coming Soon | — | — | Free online linkedin post formatter utility. This tool is planned and coming soon. |
| 625 | **Social Media Content Calendar** | `social-media-content-calendar` | ⏳ Coming Soon | — | — | Free online social media content calendar utility. This tool is planned and coming soon. |
| 614 | **YouTube Description Generator** | `youtube-description-generator` | ⏳ Coming Soon | — | — | Free online youtube description generator utility. This tool is planned and coming soon. |
| 615 | **YouTube Hashtag Generator** | `youtube-hashtag-generator` | ⏳ Coming Soon | — | — | Free online youtube hashtag generator utility. This tool is planned and coming soon. |
| 617 | **YouTube Tag Formatter** | `youtube-tag-formatter` | ⏳ Coming Soon | — | — | Free online youtube tag formatter utility. This tool is planned and coming soon. |
| 611 | **YouTube Thumbnail Downloader** | `youtube-thumbnail-downloader` | ⏳ Coming Soon | — | — | Free online youtube thumbnail downloader utility. This tool is planned and coming soon. |
| 616 | **YouTube Timestamp Generator** | `youtube-timestamp-generator` | ⏳ Coming Soon | — | — | Free online youtube timestamp generator utility. This tool is planned and coming soon. |
| 613 | **YouTube Title Analyzer** | `youtube-title-analyzer` | ⏳ Coming Soon | — | — | Free online youtube title analyzer utility. This tool is planned and coming soon. |

### Text Tools (60 tools — 54 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 222 | **ASCII to Text** | `ascii-to-text` | ✅ Available | `text-engine` | `decode_codes` | Free online ascii to text utility. This tool is planned and coming soon. |
| 100048 | **Ascii Converter** | `ascii-converter` | ✅ Available | `text-engine` | `unicode_escape` | Ascii Converter utility powered by text-engine. |
| 218 | **Binary to Text** | `binary-to-text` | ✅ Available | `text-engine` | `decode_codes` | Free online binary to text utility. This tool is planned and coming soon. |
| 202 | **Character Counter** | `character-counter` | ✅ Available | `text-engine` | `stats` | Free online character counter utility. This tool is planned and coming soon. |
| 225 | **Find and Replace** | `find-and-replace` | ✅ Available | `text-engine` | `find_replace` | Free online find and replace utility. This tool is planned and coming soon. |
| 239 | **HTML to Markdown** | `html-to-markdown` | ✅ Available | `text-engine` | `html_to_markdown` | Free online html to markdown utility. This tool is planned and coming soon. |
| 100051 | **Hex To Text** | `hex-to-text` | ✅ Available | `text-engine` | `decode_codes` | Hex To Text utility powered by text-engine. |
| 100042 | **Line Counter** | `line-counter` | ✅ Available | `text-engine` | `stats` | Line Counter utility powered by text-engine. |
| 228 | **Lorem Ipsum Generator** | `lorem-ipsum-generator` | ✅ Available | `text-engine` | `lorem` | Free online lorem ipsum generator utility. This tool is planned and coming soon. |
| 209 | **Lowercase Converter** | `lowercase-converter` | ✅ Available | `text-engine` | `case` | Free online lowercase converter utility. This tool is planned and coming soon. |
| 100056 | **Markdown Preview Parser** | `markdown-preview-parser` | ✅ Available | `text-engine` | `markdown_parse` | Markdown Preview Parser utility powered by text-engine. |
| 237 | **Markdown Previewer** | `markdown-previewer` | ✅ Available | `text-engine` | `markdown_to_html` | Free online markdown previewer utility. This tool is planned and coming soon. |
| 240 | **Markdown to DOCX** | `markdown-to-docx` | ✅ Available | `document-engine` | `from_markdown` | Free online markdown to docx utility. This tool is planned and coming soon. |
| 238 | **Markdown to HTML** | `markdown-to-html` | ✅ Available | `text-engine` | `markdown_to_html` | Free online markdown to html utility. This tool is planned and coming soon. |
| 220 | **Morse Code to Text** | `morse-code-to-text` | ✅ Available | `text-engine` | `morse_decode` | Free online morse code to text utility. This tool is planned and coming soon. |
| 204 | **Paragraph Counter** | `paragraph-counter` | ✅ Available | `text-engine` | `stats` | Free online paragraph counter utility. This tool is planned and coming soon. |
| 229 | **Random Text Generator** | `random-text-generator` | ✅ Available | `text-engine` | `random_text` | Free online random text generator utility. This tool is planned and coming soon. |
| 234 | **Readability Checker** | `readability-checker` | ✅ Available | `text-engine` | `stats` | Free online readability checker utility. This tool is planned and coming soon. |
| 205 | **Reading Time Calculator** | `reading-time-calculator` | ✅ Available | `text-engine` | `stats` | Free online reading time calculator utility. This tool is planned and coming soon. |
| 213 | **Remove Duplicate Lines** | `remove-duplicate-lines` | ✅ Available | `text-engine` | `lines` | Free online remove duplicate lines utility. This tool is planned and coming soon. |
| 100043 | **Remove Empty Lines** | `remove-empty-lines` | ✅ Available | `text-engine` | `lines` | Remove Empty Lines utility powered by text-engine. |
| 212 | **Remove Extra Spaces** | `remove-extra-spaces` | ✅ Available | `text-engine` | `cleanup` | Free online remove extra spaces utility. This tool is planned and coming soon. |
| 215 | **Reverse Text** | `reverse-text` | ✅ Available | `text-engine` | `reverse_text` | Free online reverse text utility. This tool is planned and coming soon. |
| 216 | **Reverse Words** | `reverse-words` | ✅ Available | `text-engine` | `reverse_text` | Free online reverse words utility. This tool is planned and coming soon. |
| 100049 | **Rot13** | `rot13` | ✅ Available | `text-engine` | `rot` | Rot13 utility powered by text-engine. |
| 211 | **Sentence Case Converter** | `sentence-case-converter` | ✅ Available | `text-engine` | `case` | Free online sentence case converter utility. This tool is planned and coming soon. |
| 203 | **Sentence Counter** | `sentence-counter` | ✅ Available | `text-engine` | `stats` | Free online sentence counter utility. This tool is planned and coming soon. |
| 230 | **Slug Generator** | `slug-generator` | ✅ Available | `text-engine` | `slug` | Free online slug generator utility. This tool is planned and coming soon. |
| 214 | **Sort Lines Alphabetically** | `sort-lines-alphabetically` | ✅ Available | `text-engine` | `lines` | Free online sort lines alphabetically utility. This tool is planned and coming soon. |
| 206 | **Speaking Time Calculator** | `speaking-time-calculator` | ✅ Available | `text-engine` | `stats` | Free online speaking time calculator utility. This tool is planned and coming soon. |
| 207 | **Text Case Converter** | `text-case-converter` | ✅ Available | `text-engine` | `case` | Free online text case converter utility. This tool is planned and coming soon. |
| 100046 | **Text Cleaner** | `text-cleaner` | ✅ Available | `text-engine` | `cleanup` | Text Cleaner utility powered by text-engine. |
| 227 | **Text Comparer** | `text-comparer` | ✅ Available | `text-engine` | `diff` | Free online text comparer utility. This tool is planned and coming soon. |
| 244 | **Text Decryptor** | `text-decryptor` | ✅ Available | `crypto-engine` | `decrypt` | Free online text decryptor utility. This tool is planned and coming soon. |
| 226 | **Text Diff Checker** | `text-diff-checker` | ✅ Available | `text-engine` | `diff` | Free online text diff checker utility. This tool is planned and coming soon. |
| 243 | **Text Encryptor** | `text-encryptor` | ✅ Available | `crypto-engine` | `encrypt` | Free online text encryptor utility. This tool is planned and coming soon. |
| 100055 | **Text Extractor** | `text-extractor` | ✅ Available | `text-engine` | `extract` | Text Extractor utility powered by text-engine. |
| 100045 | **Text Formatter** | `text-formatter` | ✅ Available | `text-engine` | `wrap` | Text Formatter utility powered by text-engine. |
| 100052 | **Text Joiner** | `text-joiner` | ✅ Available | `text-engine` | `join` | Text Joiner utility powered by text-engine. |
| 100053 | **Text Splitter** | `text-splitter` | ✅ Available | `text-engine` | `split` | Text Splitter utility powered by text-engine. |
| 100050 | **Text To Hex** | `text-to-hex` | ✅ Available | `text-engine` | `encode_codes` | Text To Hex utility powered by text-engine. |
| 100044 | **Text To Speech Prep** | `text-to-speech-prep` | ✅ Available | `text-engine` | `tts_prep` | Text To Speech Prep utility powered by text-engine. |
| 100054 | **Text Truncator** | `text-truncator` | ✅ Available | `text-engine` | `truncate` | Text Truncator utility powered by text-engine. |
| 221 | **Text to ASCII** | `text-to-ascii` | ✅ Available | `text-engine` | `encode_codes` | Free online text to ascii utility. This tool is planned and coming soon. |
| 217 | **Text to Binary** | `text-to-binary` | ✅ Available | `text-engine` | `encode_codes` | Free online text to binary utility. This tool is planned and coming soon. |
| 219 | **Text to Morse Code** | `text-to-morse-code` | ✅ Available | `text-engine` | `morse_encode` | Free online text to morse code utility. This tool is planned and coming soon. |
| 241 | **Text to PDF** | `text-to-pdf` | ✅ Available | `pdf-engine` | `from_text` | Free online text to pdf utility. This tool is planned and coming soon. |
| 242 | **Text to QR Code** | `text-to-qr-code` | ✅ Available | `qrbarcode-engine` | `qr_generate` | Free online text to qr code utility. This tool is planned and coming soon. |
| 223 | **Text to Unicode** | `text-to-unicode` | ✅ Available | `text-engine` | `unicode_escape` | Free online text to unicode utility. This tool is planned and coming soon. |
| 210 | **Title Case Converter** | `title-case-converter` | ✅ Available | `text-engine` | `case` | Free online title case converter utility. This tool is planned and coming soon. |
| 224 | **Unicode Inspector** | `unicode-inspector` | ✅ Available | `text-engine` | `unicode_inspect` | Free online unicode inspector utility. This tool is planned and coming soon. |
| 100047 | **Unicode Normalizer** | `unicode-normalizer` | ✅ Available | `text-engine` | `unicode_normalize` | Unicode Normalizer utility powered by text-engine. |
| 208 | **Uppercase Converter** | `uppercase-converter` | ✅ Available | `text-engine` | `case` | Free online uppercase converter utility. This tool is planned and coming soon. |
| 201 | **Word Counter** | `word-counter` | ✅ Available | `text-engine` | `stats` | Free online word counter utility. This tool is planned and coming soon. |
| 245 | **Clipboard Formatter** | `clipboard-formatter` | ⏳ Coming Soon | — | — | Free online clipboard formatter utility. This tool is planned and coming soon. |
| 233 | **Grammar Checker** | `grammar-checker` | ⏳ Coming Soon | — | — | Free online grammar checker utility. This tool is planned and coming soon. |
| 231 | **Hashtag Generator** | `hashtag-generator` | ⏳ Coming Soon | — | — | Free online hashtag generator utility. This tool is planned and coming soon. |
| 232 | **Keyword Density Checker** | `keyword-density-checker` | ⏳ Coming Soon | — | — | Free online keyword density checker utility. This tool is planned and coming soon. |
| 236 | **Markdown Editor** | `markdown-editor` | ⏳ Coming Soon | — | — | Free online markdown editor utility. This tool is planned and coming soon. |
| 235 | **Plagiarism Checker** | `plagiarism-checker` | ⏳ Coming Soon | — | — | Free online plagiarism checker utility. This tool is planned and coming soon. |

### Video Tools (56 tools — 24 Available)

| ID | Tool Name | Slug | Status | Engine | Action | Description |
| :---: | :--- | :--- | :---: | :--- | :--- | :--- |
| 159 | **GIF Compressor** | `gif-compressor` | ✅ Available | `image-engine` | `gif_resize` | Free online gif compressor utility. This tool is planned and coming soon. |
| 158 | **GIF Maker** | `gif-maker` | ✅ Available | `video-engine` | `gif_from_frames` | Free online gif maker utility. This tool is planned and coming soon. |
| 160 | **GIF Resizer** | `gif-resizer` | ✅ Available | `image-engine` | `gif_resize` | Free online gif resizer utility. This tool is planned and coming soon. |
| 123 | **MP4 to GIF** | `mp4-to-gif` | ✅ Available | `video-engine` | `gif_from_frames` | Free online mp4 to gif utility. This tool is planned and coming soon. |
| 140 | **SRT to VTT** | `srt-to-vtt` | ✅ Available | `video-engine` | `subtitle_convert` | Free online srt to vtt utility. This tool is planned and coming soon. |
| 138 | **Subtitle Extractor** | `subtitle-extractor` | ✅ Available | `video-engine` | `extract_subtitles` | Free online subtitle extractor utility. This tool is planned and coming soon. |
| 141 | **VTT to SRT** | `vtt-to-srt` | ✅ Available | `video-engine` | `subtitle_convert` | Free online vtt to srt utility. This tool is planned and coming soon. |
| 135 | **Video Aspect Ratio Converter** | `video-aspect-ratio-converter` | ✅ Available | `video-engine` | `aspect_ratio` | Free online video aspect ratio converter utility. This tool is planned and coming soon. |
| 100022 | **Video Concat** | `video-concat` | ✅ Available | `video-engine` | `concat` | Video Concat utility powered by video-engine. |
| 112 | **Video Cutter** | `video-cutter` | ✅ Available | `video-engine` | `cut` | Free online video cutter utility. This tool is planned and coming soon. |
| 100023 | **Video Extract Audio** | `video-extract-audio` | ✅ Available | `video-engine` | `extract_audio_track` | Video Extract Audio utility powered by video-engine. |
| 127 | **Video Frame Extractor** | `video-frame-extractor` | ✅ Available | `video-engine` | `frame_plan` | Free online video frame extractor utility. This tool is planned and coming soon. |
| 100025 | **Video Frame Rate Check** | `video-frame-rate-check` | ✅ Available | `video-engine` | `inspect` | Video Frame Rate Check utility powered by video-engine. |
| 100027 | **Video Loop** | `video-loop` | ✅ Available | `video-engine` | `loop` | Video Loop utility powered by video-engine. |
| 114 | **Video Merger** | `video-merger` | ✅ Available | `video-engine` | `concat` | Free online video merger utility. This tool is planned and coming soon. |
| 150 | **Video Metadata Viewer** | `video-metadata-viewer` | ✅ Available | `video-engine` | `inspect` | Free online video metadata viewer utility. This tool is planned and coming soon. |
| 142 | **Video Mute Tool** | `video-mute-tool` | ✅ Available | `video-engine` | `mute` | Free online video mute tool utility. This tool is planned and coming soon. |
| 100026 | **Video Remove Audio** | `video-remove-audio` | ✅ Available | `video-engine` | `remove_audio` | Video Remove Audio utility powered by video-engine. |
| 100024 | **Video Resolution Check** | `video-resolution-check` | ✅ Available | `video-engine` | `inspect` | Video Resolution Check utility powered by video-engine. |
| 115 | **Video Splitter** | `video-splitter` | ✅ Available | `video-engine` | `split` | Free online video splitter utility. This tool is planned and coming soon. |
| 126 | **Video Thumbnail Extractor** | `video-thumbnail-extractor` | ✅ Available | `video-engine` | `frame_plan` | Free online video thumbnail extractor utility. This tool is planned and coming soon. |
| 154 | **Video Thumbnail Maker** | `video-thumbnail-maker` | ✅ Available | `video-engine` | `contact_sheet` | Free online video thumbnail maker utility. This tool is planned and coming soon. |
| 113 | **Video Trimmer** | `video-trimmer` | ✅ Available | `video-engine` | `trim` | Free online video trimmer utility. This tool is planned and coming soon. |
| 124 | **Video to Images** | `video-to-images` | ✅ Available | `video-engine` | `frame_plan` | Free online video to images utility. This tool is planned and coming soon. |
| 121 | **AVI to MP4** | `avi-to-mp4` | ⏳ Coming Soon | `video-engine` | `transcode` | Free online avi to mp4 utility. This tool is planned and coming soon. |
| 157 | **Browser Tab Recorder** | `browser-tab-recorder` | ⏳ Coming Soon | — | — | Free online browser tab recorder utility. This tool is planned and coming soon. |
| 122 | **MKV to MP4** | `mkv-to-mp4` | ⏳ Coming Soon | `video-engine` | `transcode` | Free online mkv to mp4 utility. This tool is planned and coming soon. |
| 120 | **MOV to MP4** | `mov-to-mp4` | ⏳ Coming Soon | `video-engine` | `transcode` | Free online mov to mp4 utility. This tool is planned and coming soon. |
| 118 | **MP4 to WebM** | `mp4-to-webm` | ⏳ Coming Soon | `video-engine` | `transcode` | Free online mp4 to webm utility. This tool is planned and coming soon. |
| 131 | **Reverse Video** | `reverse-video` | ⏳ Coming Soon | `video-engine` | `transcode` | Free online reverse video utility. This tool is planned and coming soon. |
| 155 | **Screen Recorder** | `screen-recorder` | ⏳ Coming Soon | — | — | Free online screen recorder utility. This tool is planned and coming soon. |
| 129 | **Slow Motion Creator** | `slow-motion-creator` | ⏳ Coming Soon | `video-engine` | `speed_calculator` | Free online slow motion creator utility. This tool is planned and coming soon. |
| 139 | **Subtitle Merger** | `subtitle-merger` | ⏳ Coming Soon | — | — | Free online subtitle merger utility. This tool is planned and coming soon. |
| 130 | **Time-lapse Creator** | `time-lapse-creator` | ⏳ Coming Soon | `video-engine` | `speed_calculator` | Free online time-lapse creator utility. This tool is planned and coming soon. |
| 143 | **Video Audio Replacement** | `video-audio-replacement` | ⏳ Coming Soon | — | — | Free online video audio replacement utility. This tool is planned and coming soon. |
| 152 | **Video Brightness Editor** | `video-brightness-editor` | ⏳ Coming Soon | — | — | Free online video brightness editor utility. This tool is planned and coming soon. |
| 148 | **Video Collage Maker** | `video-collage-maker` | ⏳ Coming Soon | — | — | Free online video collage maker utility. This tool is planned and coming soon. |
| 151 | **Video Color Correction** | `video-color-correction` | ⏳ Coming Soon | — | — | Free online video color correction utility. This tool is planned and coming soon. |
| 149 | **Video Comparison Tool** | `video-comparison-tool` | ⏳ Coming Soon | — | — | Free online video comparison tool utility. This tool is planned and coming soon. |
| 116 | **Video Compressor** | `video-compressor` | ⏳ Coming Soon | `video-engine` | `transcode` | Free online video compressor utility. This tool is planned and coming soon. |
| 117 | **Video Converter** | `video-converter` | ⏳ Coming Soon | `video-engine` | `transcode` | Free online video converter utility. This tool is planned and coming soon. |
| 133 | **Video Cropper** | `video-cropper` | ⏳ Coming Soon | `video-engine` | `crop_calculator` | Free online video cropper utility. This tool is planned and coming soon. |
| 111 | **Video Editor** | `video-editor` | ⏳ Coming Soon | — | — | Free online video editor utility. This tool is planned and coming soon. |
| 144 | **Video Fade In/Out** | `video-fade-in-out` | ⏳ Coming Soon | — | — | Free online video fade in/out utility. This tool is planned and coming soon. |
| 146 | **Video Overlay Editor** | `video-overlay-editor` | ⏳ Coming Soon | — | — | Free online video overlay editor utility. This tool is planned and coming soon. |
| 134 | **Video Resizer** | `video-resizer` | ⏳ Coming Soon | `video-engine` | `resize_calculator` | Free online video resizer utility. This tool is planned and coming soon. |
| 132 | **Video Rotation** | `video-rotation` | ⏳ Coming Soon | `video-engine` | `transcode` | Free online video rotation utility. This tool is planned and coming soon. |
| 128 | **Video Speed Changer** | `video-speed-changer` | ⏳ Coming Soon | `video-engine` | `speed_calculator` | Free online video speed changer utility. This tool is planned and coming soon. |
| 153 | **Video Stabilization** | `video-stabilization` | ⏳ Coming Soon | — | — | Free online video stabilization utility. This tool is planned and coming soon. |
| 137 | **Video Subtitle Editor** | `video-subtitle-editor` | ⏳ Coming Soon | — | — | Free online video subtitle editor utility. This tool is planned and coming soon. |
| 147 | **Video Text Overlay** | `video-text-overlay` | ⏳ Coming Soon | — | — | Free online video text overlay utility. This tool is planned and coming soon. |
| 145 | **Video Transition Editor** | `video-transition-editor` | ⏳ Coming Soon | — | — | Free online video transition editor utility. This tool is planned and coming soon. |
| 136 | **Video Watermark** | `video-watermark` | ⏳ Coming Soon | `video-engine` | `transcode` | Free online video watermark utility. This tool is planned and coming soon. |
| 125 | **Video to Audio** | `video-to-audio` | ⏳ Coming Soon | `audio-engine` | `to_video` | Free online video to audio utility. This tool is planned and coming soon. |
| 119 | **WebM to MP4** | `webm-to-mp4` | ⏳ Coming Soon | `video-engine` | `transcode` | Free online webm to mp4 utility. This tool is planned and coming soon. |
| 156 | **Webcam Recorder** | `webcam-recorder` | ⏳ Coming Soon | — | — | Free online webcam recorder utility. This tool is planned and coming soon. |
