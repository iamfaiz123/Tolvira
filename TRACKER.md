# Dev Toolbox — Tool Testing & Fix Tracker

> **Progress**: **1** of **765** live utilities tested & verified (**0.1%**)

## Overall Progress Dashboard

| Status | Count | Percentage (of Live) | Meaning |
| :--- | :---: | :---: | :--- |
| ✅ **Tested & Passed** | `1` | 0.1% | Tested thoroughly in browser; works as expected |
| 🔧 **Fixed & Verified** | `0` | 0.0% | Had an issue, fixed and re-verified |
| ⚠️ **Issue Found / Needs Fix** | `0` | 0.0% | Bug or unexpected behavior flagged for investigation |
| ⏳ **Untested** | `764` | 99.9% | Live tool waiting to be tested |
| 💤 **Queued (Coming Soon)** | `255` | — | Defined in catalog, queued for WASM engine integration |
| **Total Tools** | **`1020`** | **100%** | Full platform inventory |

---

## CLI Quick Reference

You can update this tracker directly using the `scripts/track.py` script:

```bash
# Show current testing progress
python3 scripts/track.py status

# Mark a tool as tested & working
python3 scripts/track.py mark <slug-or-id> pass "worked with sample.pdf"

# Mark an issue found on a tool
python3 scripts/track.py mark <slug-or-id> issue "fails with large files"

# Mark an issue as fixed
python3 scripts/track.py mark <slug-or-id> fixed "adjusted memory buffer limit"

# View all tools with open issues
python3 scripts/track.py issues

# Get the next untested tool to work on
python3 scripts/track.py next [category-name]
```

---

## Recent Testing Activity Log

| Timestamp | Tool | Status | Notes |
| :--- | :--- | :---: | :--- |
| 2026-11-01T20:43 | `compress-pdf` | ✅ Passed | Tested full width and compression |

---

## Category Testing Progress

| Category | Total | Live | Passed | Fixed | Issues | Untested | Progress |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **AI Tools** | 25 | 0 | 0 | 0 | 0 | 0 | **0%** |
| **Audio Tools** | 47 | 40 | 0 | 0 | 0 | 40 | **0%** |
| **Calculators & Converters** | 151 | 141 | 0 | 0 | 0 | 141 | **0%** |
| **Creative Tools** | 25 | 1 | 0 | 0 | 0 | 1 | **0%** |
| **Design Tools** | 57 | 50 | 0 | 0 | 0 | 50 | **0%** |
| **Developer Tools** | 158 | 140 | 0 | 0 | 0 | 140 | **0%** |
| **Everyday Tools** | 41 | 28 | 0 | 0 | 0 | 28 | **0%** |
| **File Tools** | 74 | 66 | 0 | 0 | 0 | 66 | **0%** |
| **Image Tools** | 71 | 63 | 0 | 0 | 0 | 63 | **0%** |
| **Math Tools** | 39 | 38 | 0 | 0 | 0 | 38 | **0%** |
| **PDF & Documents** | 75 | 63 | 1 | 0 | 0 | 62 | **2%** |
| **Productivity** | 39 | 19 | 0 | 0 | 0 | 19 | **0%** |
| **SEO & Website** | 25 | 5 | 0 | 0 | 0 | 5 | **0%** |
| **Security & Network** | 58 | 25 | 0 | 0 | 0 | 25 | **0%** |
| **Social Media** | 20 | 8 | 0 | 0 | 0 | 8 | **0%** |
| **Text Tools** | 59 | 54 | 0 | 0 | 0 | 54 | **0%** |
| **Video Tools** | 56 | 24 | 0 | 0 | 0 | 24 | **0%** |

---

## Detailed Tool Checklist

### AI Tools (25 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 586 | **AI Text Summarizer** | `ai-text-summarizer` | — | 💤 Queued | — |
| [ ] | 587 | **AI Paraphraser** | `ai-paraphraser` | — | 💤 Queued | — |
| [ ] | 588 | **AI Grammar Fixer** | `ai-grammar-fixer` | — | 💤 Queued | — |
| [ ] | 589 | **AI Email Writer** | `ai-email-writer` | — | 💤 Queued | — |
| [ ] | 590 | **AI Cover Letter Generator** | `ai-cover-letter-generator` | — | 💤 Queued | — |
| [ ] | 591 | **AI Resume Reviewer** | `ai-resume-reviewer` | — | 💤 Queued | — |
| [ ] | 592 | **AI Resume Builder** | `ai-resume-builder` | — | 💤 Queued | — |
| [ ] | 593 | **AI PDF Question Answering** | `ai-pdf-question-answering` | — | 💤 Queued | — |
| [ ] | 594 | **AI Document Summarizer** | `ai-document-summarizer` | — | 💤 Queued | — |
| [ ] | 595 | **AI Image Caption Generator** | `ai-image-caption-generator` | — | 💤 Queued | — |
| [ ] | 596 | **AI Image Background Remover** | `ai-image-background-remover` | — | 💤 Queued | — |
| [ ] | 597 | **AI Object Remover** | `ai-object-remover` | — | 💤 Queued | — |
| [ ] | 598 | **AI Photo Enhancer** | `ai-photo-enhancer` | — | 💤 Queued | — |
| [ ] | 599 | **AI Image Upscaler** | `ai-image-upscaler` | — | 💤 Queued | — |
| [ ] | 600 | **AI Speech-to-Text** | `ai-speech-to-text` | — | 💤 Queued | — |
| [ ] | 601 | **AI Text-to-Speech** | `ai-text-to-speech` | — | 💤 Queued | — |
| [ ] | 602 | **AI Subtitle Generator** | `ai-subtitle-generator` | — | 💤 Queued | — |
| [ ] | 603 | **AI Translation Tool** | `ai-translation-tool` | — | 💤 Queued | — |
| [ ] | 604 | **AI Meeting Summarizer** | `ai-meeting-summarizer` | — | 💤 Queued | — |
| [ ] | 605 | **AI Code Explainer** | `ai-code-explainer` | — | 💤 Queued | — |
| [ ] | 606 | **AI SQL Generator** | `ai-sql-generator` | — | 💤 Queued | — |
| [ ] | 607 | **AI Regex Generator** | `ai-regex-generator` | — | 💤 Queued | — |
| [ ] | 608 | **AI Test Case Generator** | `ai-test-case-generator` | — | 💤 Queued | — |
| [ ] | 609 | **AI Commit Message Generator** | `ai-commit-message-generator` | — | 💤 Queued | — |
| [ ] | 610 | **AI Presentation Generator** | `ai-presentation-generator` | — | 💤 Queued | — |

### Audio Tools (47 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 161 | **Audio Cutter** | `audio-cutter` | `audio-engine` | ⏳ Untested | — |
| [ ] | 162 | **Audio Trimmer** | `audio-trimmer` | `audio-engine` | ⏳ Untested | — |
| [ ] | 163 | **Audio Merger** | `audio-merger` | `audio-engine` | ⏳ Untested | — |
| [ ] | 164 | **Audio Splitter** | `audio-splitter` | `audio-engine` | ⏳ Untested | — |
| [ ] | 165 | **Audio Compressor** | `audio-compressor` | `audio-engine` | ⏳ Untested | — |
| [ ] | 166 | **Audio Converter** | `audio-converter` | `audio-engine` | ⏳ Untested | — |
| [ ] | 167 | **MP3 to WAV** | `mp3-to-wav` | `audio-engine` | ⏳ Untested | — |
| [ ] | 168 | **WAV to MP3** | `wav-to-mp3` | `audio-engine` | ⏳ Untested | — |
| [ ] | 169 | **MP3 to AAC** | `mp3-to-aac` | `audio-engine` | ⏳ Untested | — |
| [ ] | 170 | **FLAC to MP3** | `flac-to-mp3` | `audio-engine` | ⏳ Untested | — |
| [ ] | 171 | **M4A to MP3** | `m4a-to-mp3` | `audio-engine` | ⏳ Untested | — |
| [ ] | 172 | **OGG to MP3** | `ogg-to-mp3` | `audio-engine` | ⏳ Untested | — |
| [ ] | 173 | **Audio Volume Booster** | `audio-volume-booster` | `audio-engine` | ⏳ Untested | — |
| [ ] | 174 | **Audio Normalizer** | `audio-normalizer` | `audio-engine` | ⏳ Untested | — |
| [ ] | 175 | **Audio Speed Changer** | `audio-speed-changer` | `audio-engine` | ⏳ Untested | — |
| [ ] | 176 | **Audio Pitch Changer** | `audio-pitch-changer` | `audio-engine` | ⏳ Untested | — |
| [ ] | 177 | **Audio Reverse** | `audio-reverse` | `audio-engine` | ⏳ Untested | — |
| [ ] | 178 | **Audio Fade In/Out** | `audio-fade-in-out` | `audio-engine` | ⏳ Untested | — |
| [ ] | 179 | **Audio Noise Remover** | `audio-noise-remover` | `audio-engine` | ⏳ Untested | — |
| [ ] | 180 | **Audio Silence Remover** | `audio-silence-remover` | `audio-engine` | ⏳ Untested | — |
| [ ] | 181 | **Audio Waveform Visualizer** | `audio-waveform-visualizer` | `audio-engine` | ⏳ Untested | — |
| [ ] | 182 | **Audio Spectrum Visualizer** | `audio-spectrum-visualizer` | `audio-engine` | ⏳ Untested | — |
| [ ] | 183 | **Audio Equalizer** | `audio-equalizer` | `audio-engine` | ⏳ Untested | — |
| [ ] | 184 | **Audio Bass Booster** | `audio-bass-booster` | `audio-engine` | ⏳ Untested | — |
| [ ] | 185 | **Audio Treble Booster** | `audio-treble-booster` | `audio-engine` | ⏳ Untested | — |
| [ ] | 186 | **Audio Channel Converter** | `audio-channel-converter` | `audio-engine` | ⏳ Untested | — |
| [ ] | 187 | **Mono to Stereo** | `mono-to-stereo` | `audio-engine` | ⏳ Untested | — |
| [ ] | 188 | **Stereo to Mono** | `stereo-to-mono` | `audio-engine` | ⏳ Untested | — |
| [ ] | 189 | **Audio Metadata Editor** | `audio-metadata-editor` | — | 💤 Queued | — |
| [ ] | 190 | **Audio Metadata Viewer** | `audio-metadata-viewer` | `audio-engine` | ⏳ Untested | — |
| [ ] | 191 | **ID3 Tag Editor** | `id3-tag-editor` | — | 💤 Queued | — |
| [ ] | 192 | **Album Art Editor** | `album-art-editor` | — | 💤 Queued | — |
| [ ] | 193 | **Audio to Spectrogram** | `audio-to-spectrogram` | `audio-engine` | ⏳ Untested | — |
| [ ] | 194 | **Audio Recorder** | `audio-recorder` | — | 💤 Queued | — |
| [ ] | 195 | **Voice Recorder** | `voice-recorder` | — | 💤 Queued | — |
| [ ] | 196 | **Microphone Tester** | `microphone-tester` | — | 💤 Queued | — |
| [ ] | 197 | **Speaker Tester** | `speaker-tester` | — | 💤 Queued | — |
| [ ] | 198 | **Audio Frequency Generator** | `audio-frequency-generator` | `audio-engine` | ⏳ Untested | — |
| [ ] | 199 | **White Noise Generator** | `white-noise-generator` | `audio-engine` | ⏳ Untested | — |
| [ ] | 200 | **Audio Loop Maker** | `audio-loop-maker` | `audio-engine` | ⏳ Untested | — |
| [ ] | 100028 | **Audio Concat** | `audio-concat` | `audio-engine` | ⏳ Untested | — |
| [ ] | 100029 | **Audio Fade Out** | `audio-fade-out` | `audio-engine` | ⏳ Untested | — |
| [ ] | 100030 | **Audio Extract From Video** | `audio-extract-from-video` | `audio-engine` | ⏳ Untested | — |
| [ ] | 100031 | **Audio Duration** | `audio-duration` | `audio-engine` | ⏳ Untested | — |
| [ ] | 100032 | **Audio Sample Rate Converter** | `audio-sample-rate-converter` | `audio-engine` | ⏳ Untested | — |
| [ ] | 100033 | **Audio Join** | `audio-join` | `audio-engine` | ⏳ Untested | — |
| [ ] | 100034 | **Audio Visualizer** | `audio-visualizer` | `audio-engine` | ⏳ Untested | — |

### Calculators & Converters (151 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 311 | **Basic Calculator** | `basic-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 312 | **Scientific Calculator** | `scientific-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 313 | **Percentage Calculator** | `percentage-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 314 | **Discount Calculator** | `discount-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 315 | **GST Calculator** | `gst-calculator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 316 | **Income Tax Calculator** | `income-tax-calculator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 317 | **Salary Calculator** | `salary-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 318 | **Take-home Salary Calculator** | `take-home-salary-calculator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 319 | **EMI Calculator** | `emi-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 320 | **Loan Calculator** | `loan-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 321 | **Mortgage Calculator** | `mortgage-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 322 | **Interest Calculator** | `interest-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 323 | **Compound Interest Calculator** | `compound-interest-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 324 | **Simple Interest Calculator** | `simple-interest-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 325 | **SIP Calculator** | `sip-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 326 | **Mutual Fund Return Calculator** | `mutual-fund-return-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 327 | **FD Calculator** | `fd-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 328 | **RD Calculator** | `rd-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 329 | **PPF Calculator** | `ppf-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 330 | **Inflation Calculator** | `inflation-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 331 | **Currency Calculator** | `currency-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 332 | **Profit Margin Calculator** | `profit-margin-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 333 | **Markup Calculator** | `markup-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 334 | **Break-even Calculator** | `break-even-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 335 | **ROI Calculator** | `roi-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 336 | **CAGR Calculator** | `cagr-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 337 | **Age Calculator** | `age-calculator` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 338 | **Date Difference Calculator** | `date-difference-calculator` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 339 | **Days Between Dates** | `days-between-dates` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 340 | **Time Calculator** | `time-calculator` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 341 | **Working Days Calculator** | `working-days-calculator` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 342 | **Salary Hike Calculator** | `salary-hike-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 343 | **Overtime Calculator** | `overtime-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 344 | **Freelance Hourly Rate Calculator** | `freelance-hourly-rate-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 345 | **Tip Calculator** | `tip-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 346 | **Split Bill Calculator** | `split-bill-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 347 | **Fuel Cost Calculator** | `fuel-cost-calculator` | `units-engine` | ⏳ Untested | — |
| [ ] | 348 | **Mileage Calculator** | `mileage-calculator` | `units-engine` | ⏳ Untested | — |
| [ ] | 349 | **Speed Calculator** | `speed-calculator` | `units-engine` | ⏳ Untested | — |
| [ ] | 350 | **Distance Calculator** | `distance-calculator` | `geo-engine` | ⏳ Untested | — |
| [ ] | 351 | **BMI Calculator** | `bmi-calculator` | — | 💤 Queued | — |
| [ ] | 352 | **BMR Calculator** | `bmr-calculator` | — | 💤 Queued | — |
| [ ] | 353 | **Calorie Calculator** | `calorie-calculator` | — | 💤 Queued | — |
| [ ] | 354 | **Macro Calculator** | `macro-calculator` | — | 💤 Queued | — |
| [ ] | 355 | **Protein Intake Calculator** | `protein-intake-calculator` | — | 💤 Queued | — |
| [ ] | 356 | **Body Fat Calculator** | `body-fat-calculator` | — | 💤 Queued | — |
| [ ] | 357 | **Ideal Weight Calculator** | `ideal-weight-calculator` | — | 💤 Queued | — |
| [ ] | 358 | **Water Intake Calculator** | `water-intake-calculator` | — | 💤 Queued | — |
| [ ] | 359 | **Pregnancy Due Date Calculator** | `pregnancy-due-date-calculator` | — | 💤 Queued | — |
| [ ] | 360 | **Ovulation Calculator** | `ovulation-calculator` | — | 💤 Queued | — |
| [ ] | 361 | **Length Converter** | `length-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 362 | **Weight Converter** | `weight-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 363 | **Temperature Converter** | `temperature-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 364 | **Area Converter** | `area-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 365 | **Volume Converter** | `volume-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 366 | **Speed Converter** | `speed-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 367 | **Time Converter** | `time-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 368 | **Pressure Converter** | `pressure-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 369 | **Energy Converter** | `energy-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 370 | **Power Converter** | `power-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 371 | **Data Storage Converter** | `data-storage-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 372 | **Binary to Decimal** | `binary-to-decimal` | `math-engine` | ⏳ Untested | — |
| [ ] | 373 | **Decimal to Binary** | `decimal-to-binary` | `math-engine` | ⏳ Untested | — |
| [ ] | 374 | **Binary to Hexadecimal** | `binary-to-hexadecimal` | `math-engine` | ⏳ Untested | — |
| [ ] | 375 | **Hexadecimal to Decimal** | `hexadecimal-to-decimal` | `math-engine` | ⏳ Untested | — |
| [ ] | 376 | **Octal Converter** | `octal-converter` | `math-engine` | ⏳ Untested | — |
| [ ] | 377 | **Roman Numeral Converter** | `roman-numeral-converter` | `math-engine` | ⏳ Untested | — |
| [ ] | 378 | **Feet to Meters** | `feet-to-meters` | `units-engine` | ⏳ Untested | — |
| [ ] | 379 | **Inches to Centimeters** | `inches-to-centimeters` | `units-engine` | ⏳ Untested | — |
| [ ] | 380 | **Miles to Kilometers** | `miles-to-kilometers` | `units-engine` | ⏳ Untested | — |
| [ ] | 381 | **KG to Pounds** | `kg-to-pounds` | `units-engine` | ⏳ Untested | — |
| [ ] | 382 | **Celsius to Fahrenheit** | `celsius-to-fahrenheit` | `units-engine` | ⏳ Untested | — |
| [ ] | 383 | **Liters to Gallons** | `liters-to-gallons` | `units-engine` | ⏳ Untested | — |
| [ ] | 384 | **MB to GB** | `mb-to-gb` | `units-engine` | ⏳ Untested | — |
| [ ] | 385 | **Mbps to MB/s** | `mbps-to-mb-s` | `units-engine` | ⏳ Untested | — |
| [ ] | 386 | **Pixels to Inches** | `pixels-to-inches` | `units-engine` | ⏳ Untested | — |
| [ ] | 387 | **DPI Calculator** | `dpi-calculator` | `units-engine` | ⏳ Untested | — |
| [ ] | 388 | **Aspect Ratio Calculator** | `aspect-ratio-calculator` | `video-engine` | ⏳ Untested | — |
| [ ] | 389 | **Screen Resolution Calculator** | `screen-resolution-calculator` | `units-engine` | ⏳ Untested | — |
| [ ] | 390 | **Time Zone Converter** | `time-zone-converter` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 100160 | **Angle Converter** | `angle-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 100161 | **Force Converter** | `force-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 100162 | **Density Converter** | `density-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 100163 | **Frequency Converter** | `frequency-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 100164 | **Torque Converter** | `torque-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 100165 | **Shoe Size Converter** | `shoe-size-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 100166 | **Paper Size Converter** | `paper-size-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 100167 | **Lumpsum Calculator** | `lumpsum-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 100168 | **IRR Calculator** | `irr-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 100169 | **NPV Calculator** | `npv-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 100170 | **Hourly To Salary Calculator** | `hourly-to-salary-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 100171 | **Tax Bracket Calculator** | `tax-bracket-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 100172 | **GST Inclusive Exclusive Calculator** | `gst-inclusive-exclusive-calculator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100173 | **GST Invoice Validator** | `gst-invoice-validator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100174 | **Gstin Format Validator** | `gstin-format-validator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100175 | **Hsn Sac Lookup** | `hsn-sac-lookup` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100176 | **GST Rate Calculator** | `gst-rate-calculator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100177 | **Input Tax Credit Calculator** | `input-tax-credit-calculator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100178 | **Gstr Data Validator** | `gstr-data-validator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100179 | **Invoice Number Validator** | `invoice-number-validator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100180 | **TDS Calculator** | `tds-calculator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100181 | **Professional Tax Calculator** | `professional-tax-calculator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100182 | **Capital Gains Estimator** | `capital-gains-estimator` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100183 | **Bank Statement Cleaner** | `bank-statement-cleaner` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100184 | **Bank Statement To CSV** | `bank-statement-to-csv` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100185 | **Bank Statement To Tally** | `bank-statement-to-tally` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100186 | **Purchase Register Reconciliation** | `purchase-register-reconciliation` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100187 | **Sales Register Reconciliation** | `sales-register-reconciliation` | `indiatax-engine` | ⏳ Untested | — |
| [ ] | 100188 | **Cement Calculator** | `cement-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100189 | **Concrete Calculator** | `concrete-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100190 | **Sand Calculator** | `sand-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100191 | **Aggregate Calculator** | `aggregate-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100192 | **Brick Calculator** | `brick-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100193 | **Block Calculator** | `block-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100194 | **Mortar Calculator** | `mortar-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100195 | **Plaster Calculator** | `plaster-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100196 | **Paint Calculator** | `paint-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100197 | **Tile Calculator** | `tile-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100198 | **Flooring Calculator** | `flooring-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100199 | **Roofing Calculator** | `roofing-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100200 | **Steel Weight Calculator** | `steel-weight-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100201 | **Rebar Calculator** | `rebar-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100202 | **Excavation Calculator** | `excavation-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100203 | **Wall Area Calculator** | `wall-area-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100204 | **Staircase Calculator** | `staircase-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100205 | **Paver Calculator** | `paver-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100206 | **Water Tank Capacity Calculator** | `water-tank-capacity-calculator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100207 | **Construction Cost Estimator** | `construction-cost-estimator` | `construction-engine` | ⏳ Untested | — |
| [ ] | 100208 | **Solar Panel Size Calculator** | `solar-panel-size-calculator` | `solar-engine` | ⏳ Untested | — |
| [ ] | 100209 | **Solar ROI Calculator** | `solar-roi-calculator` | `solar-engine` | ⏳ Untested | — |
| [ ] | 100210 | **Solar Savings Calculator** | `solar-savings-calculator` | `solar-engine` | ⏳ Untested | — |
| [ ] | 100211 | **Solar Roof Area Calculator** | `solar-roof-area-calculator` | `solar-engine` | ⏳ Untested | — |
| [ ] | 100212 | **Solar Battery Sizing** | `solar-battery-sizing` | `solar-engine` | ⏳ Untested | — |
| [ ] | 100213 | **Inverter Sizing Calculator** | `inverter-sizing-calculator` | `solar-engine` | ⏳ Untested | — |
| [ ] | 100214 | **Backup Time Calculator** | `backup-time-calculator` | `solar-engine` | ⏳ Untested | — |
| [ ] | 100215 | **Appliance Energy Calculator** | `appliance-energy-calculator` | `solar-engine` | ⏳ Untested | — |
| [ ] | 100216 | **Electricity Unit Cost Calculator** | `electricity-unit-cost-calculator` | `solar-engine` | ⏳ Untested | — |
| [ ] | 100217 | **Carbon Footprint Estimator** | `carbon-footprint-estimator` | `solar-engine` | ⏳ Untested | — |
| [ ] | 100261 | **Geojson Formatter** | `geojson-formatter` | `geo-engine` | ⏳ Untested | — |
| [ ] | 100262 | **Geojson Validator** | `geojson-validator` | `geo-engine` | ⏳ Untested | — |
| [ ] | 100263 | **Geojson To CSV** | `geojson-to-csv` | `geo-engine` | ⏳ Untested | — |
| [ ] | 100264 | **CSV To Geojson** | `csv-to-geojson` | `geo-engine` | ⏳ Untested | — |
| [ ] | 100265 | **Geojson To KML** | `geojson-to-kml` | `geo-engine` | ⏳ Untested | — |
| [ ] | 100266 | **KML To Geojson** | `kml-to-geojson` | `geo-engine` | ⏳ Untested | — |
| [ ] | 100267 | **WKT Viewer** | `wkt-viewer` | `geo-engine` | ⏳ Untested | — |
| [ ] | 100268 | **Coordinate Converter** | `coordinate-converter` | `geo-engine` | ⏳ Untested | — |
| [ ] | 100269 | **Area From Coordinates** | `area-from-coordinates` | `geo-engine` | ⏳ Untested | — |
| [ ] | 100270 | **Geojson Simplifier** | `geojson-simplifier` | `geo-engine` | ⏳ Untested | — |
| [ ] | 100271 | **Geojson Bbox** | `geojson-bbox` | `geo-engine` | ⏳ Untested | — |
| [ ] | 100272 | **Geohash Encoder Decoder** | `geohash-encoder-decoder` | `geo-engine` | ⏳ Untested | — |
| [ ] | 100273 | **UTM Latlon Converter** | `utm-latlon-converter` | `geo-engine` | ⏳ Untested | — |

### Creative Tools (25 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 686 | **Whiteboard** | `whiteboard` | — | 💤 Queued | — |
| [ ] | 687 | **Mind Map Maker** | `mind-map-maker` | — | 💤 Queued | — |
| [ ] | 688 | **Flowchart Maker** | `flowchart-maker` | — | 💤 Queued | — |
| [ ] | 689 | **Diagram Editor** | `diagram-editor` | — | 💤 Queued | — |
| [ ] | 690 | **Presentation Editor** | `presentation-editor` | — | 💤 Queued | — |
| [ ] | 691 | **Spreadsheet Editor** | `spreadsheet-editor` | — | 💤 Queued | — |
| [ ] | 692 | **Document Editor** | `document-editor` | — | 💤 Queued | — |
| [ ] | 693 | **Rich Text Editor** | `rich-text-editor` | — | 💤 Queued | — |
| [ ] | 694 | **Code Editor** | `code-editor` | — | 💤 Queued | — |
| [ ] | 695 | **HTML Playground** | `html-playground` | — | 💤 Queued | — |
| [ ] | 696 | **CSS Playground** | `css-playground` | — | 💤 Queued | — |
| [ ] | 697 | **JavaScript Playground** | `javascript-playground` | — | 💤 Queued | — |
| [ ] | 698 | **SQL Playground** | `sql-playground` | — | 💤 Queued | — |
| [ ] | 699 | **Python Playground** | `python-playground` | — | 💤 Queued | — |
| [ ] | 700 | **Rust Playground** | `rust-playground` | — | 💤 Queued | — |
| [ ] | 701 | **Image Annotation Tool** | `image-annotation-tool` | — | 💤 Queued | — |
| [ ] | 702 | **Screenshot Editor** | `screenshot-editor` | — | 💤 Queued | — |
| [ ] | 703 | **Screenshot Capture Tool** | `screenshot-capture-tool` | — | 💤 Queued | — |
| [ ] | 704 | **Browser-based OCR** | `browser-based-ocr` | — | 💤 Queued | — |
| [ ] | 705 | **Barcode Reader** | `barcode-reader` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 706 | **Webcam Image Capture** | `webcam-image-capture` | — | 💤 Queued | — |
| [ ] | 707 | **Screen Color Picker** | `screen-color-picker` | — | 💤 Queued | — |
| [ ] | 708 | **Screen Ruler** | `screen-ruler` | — | 💤 Queued | — |
| [ ] | 709 | **Virtual Keyboard** | `virtual-keyboard` | — | 💤 Queued | — |
| [ ] | 710 | **File Previewer** | `file-previewer` | — | 💤 Queued | — |

### Design Tools (57 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 501 | **Color Picker** | `color-picker` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 502 | **HEX Color Converter** | `hex-color-converter` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 503 | **RGB Color Converter** | `rgb-color-converter` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 504 | **HSL Color Converter** | `hsl-color-converter` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 505 | **CMYK Converter** | `cmyk-converter` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 506 | **Gradient Generator** | `gradient-generator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 507 | **Gradient Palette Generator** | `gradient-palette-generator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 508 | **Color Palette Generator** | `color-palette-generator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 509 | **Complementary Color Finder** | `complementary-color-finder` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 510 | **Analogous Color Finder** | `analogous-color-finder` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 511 | **Contrast Checker** | `contrast-checker` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 512 | **WCAG Accessibility Checker** | `wcag-accessibility-checker` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 513 | **Font Pairing Generator** | `font-pairing-generator` | `font-engine` | ⏳ Untested | — |
| [ ] | 514 | **Font Previewer** | `font-previewer` | `font-engine` | ⏳ Untested | — |
| [ ] | 515 | **Google Fonts Previewer** | `google-fonts-previewer` | — | 💤 Queued | — |
| [ ] | 516 | **Favicon Generator** | `favicon-generator` | `image-engine` | ⏳ Untested | — |
| [ ] | 517 | **App Icon Generator** | `app-icon-generator` | `image-engine` | ⏳ Untested | — |
| [ ] | 518 | **Logo Maker** | `logo-maker` | — | 💤 Queued | — |
| [ ] | 519 | **SVG Icon Library** | `svg-icon-library` | — | 💤 Queued | — |
| [ ] | 520 | **CSS Pattern Generator** | `css-pattern-generator` | — | 💤 Queued | — |
| [ ] | 521 | **Glassmorphism Generator** | `glassmorphism-generator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 522 | **Neumorphism Generator** | `neumorphism-generator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 523 | **Box Shadow Generator** | `box-shadow-generator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 524 | **Border Radius Generator** | `border-radius-generator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 525 | **Social Media Post Resizer** | `social-media-post-resizer` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 526 | **Instagram Story Resizer** | `instagram-story-resizer` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 527 | **YouTube Thumbnail Resizer** | `youtube-thumbnail-resizer` | — | 💤 Queued | — |
| [ ] | 528 | **Open Graph Image Generator** | `open-graph-image-generator` | `image-engine` | ⏳ Untested | — |
| [ ] | 529 | **CSS Grid Generator** | `css-grid-generator` | — | 💤 Queued | — |
| [ ] | 530 | **CSS Flexbox Playground** | `css-flexbox-playground` | — | 💤 Queued | — |
| [ ] | 100223 | **Rgb To Hsl** | `rgb-to-hsl` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 100224 | **Accessible Color Pairing** | `accessible-color-pairing` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 100225 | **Color Blindness Simulator** | `color-blindness-simulator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 100226 | **CSS Unit Converter** | `css-unit-converter` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 100227 | **Spacing Scale Generator** | `spacing-scale-generator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 100228 | **Typography Scale Generator** | `typography-scale-generator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 100229 | **Favicon Preview** | `favicon-preview` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 100236 | **Font Metadata Viewer** | `font-metadata-viewer` | `font-engine` | ⏳ Untested | — |
| [ ] | 100237 | **Font Converter** | `font-converter` | `font-engine` | ⏳ Untested | — |
| [ ] | 100238 | **Woff To TTF** | `woff-to-ttf` | `font-engine` | ⏳ Untested | — |
| [ ] | 100239 | **TTF To Woff** | `ttf-to-woff` | `font-engine` | ⏳ Untested | — |
| [ ] | 100240 | **OTF Inspector** | `otf-inspector` | `font-engine` | ⏳ Untested | — |
| [ ] | 100241 | **Font Subsetter** | `font-subsetter` | `font-engine` | ⏳ Untested | — |
| [ ] | 100242 | **Font Character Map** | `font-character-map` | `font-engine` | ⏳ Untested | — |
| [ ] | 100243 | **Font Size Checker** | `font-size-checker` | `font-engine` | ⏳ Untested | — |
| [ ] | 100249 | **STL Viewer** | `stl-viewer` | `mesh3d-engine` | ⏳ Untested | — |
| [ ] | 100250 | **STL To OBJ** | `stl-to-obj` | `mesh3d-engine` | ⏳ Untested | — |
| [ ] | 100251 | **OBJ To STL** | `obj-to-stl` | `mesh3d-engine` | ⏳ Untested | — |
| [ ] | 100252 | **Gltf Viewer** | `gltf-viewer` | `mesh3d-engine` | ⏳ Untested | — |
| [ ] | 100253 | **GLB To Gltf** | `glb-to-gltf` | `mesh3d-engine` | ⏳ Untested | — |
| [ ] | 100254 | **Gltf Optimizer** | `gltf-optimizer` | `mesh3d-engine` | ⏳ Untested | — |
| [ ] | 100255 | **Mesh Inspector** | `mesh-inspector` | `mesh3d-engine` | ⏳ Untested | — |
| [ ] | 100256 | **Mesh Volume Calculator** | `mesh-volume-calculator` | `mesh3d-engine` | ⏳ Untested | — |
| [ ] | 100257 | **Mesh Scale** | `mesh-scale` | `mesh3d-engine` | ⏳ Untested | — |
| [ ] | 100258 | **Mesh Rotate** | `mesh-rotate` | `mesh3d-engine` | ⏳ Untested | — |
| [ ] | 100259 | **Mesh Repair Basic** | `mesh-repair-basic` | `mesh3d-engine` | ⏳ Untested | — |
| [ ] | 100260 | **3d Model Thumbnail** | `3d-model-thumbnail` | `mesh3d-engine` | ⏳ Untested | — |

### Developer Tools (158 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 246 | **JSON Formatter** | `json-formatter` | `json-engine` | ⏳ Untested | — |
| [ ] | 247 | **JSON Validator** | `json-validator` | `json-engine` | ⏳ Untested | — |
| [ ] | 248 | **JSON Minifier** | `json-minifier` | `json-engine` | ⏳ Untested | — |
| [ ] | 249 | **JSON Beautifier** | `json-beautifier` | `json-engine` | ⏳ Untested | — |
| [ ] | 250 | **JSON to YAML** | `json-to-yaml` | `json-engine` | ⏳ Untested | — |
| [ ] | 251 | **YAML to JSON** | `yaml-to-json` | `json-engine` | ⏳ Untested | — |
| [ ] | 252 | **JSON to CSV** | `json-to-csv` | `json-engine` | ⏳ Untested | — |
| [ ] | 253 | **CSV to JSON** | `csv-to-json` | `json-engine` | ⏳ Untested | — |
| [ ] | 254 | **JSON to XML** | `json-to-xml` | `json-engine` | ⏳ Untested | — |
| [ ] | 255 | **XML to JSON** | `xml-to-json` | `json-engine` | ⏳ Untested | — |
| [ ] | 256 | **XML Formatter** | `xml-formatter` | `json-engine` | ⏳ Untested | — |
| [ ] | 257 | **XML Validator** | `xml-validator` | `json-engine` | ⏳ Untested | — |
| [ ] | 258 | **HTML Formatter** | `html-formatter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 259 | **HTML Minifier** | `html-minifier` | `developer-engine` | ⏳ Untested | — |
| [ ] | 260 | **CSS Formatter** | `css-formatter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 261 | **CSS Minifier** | `css-minifier` | `developer-engine` | ⏳ Untested | — |
| [ ] | 262 | **JavaScript Formatter** | `javascript-formatter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 263 | **JavaScript Minifier** | `javascript-minifier` | `developer-engine` | ⏳ Untested | — |
| [ ] | 264 | **SQL Formatter** | `sql-formatter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 265 | **SQL Minifier** | `sql-minifier` | `developer-engine` | ⏳ Untested | — |
| [ ] | 266 | **Regex Tester** | `regex-tester` | `developer-engine` | ⏳ Untested | — |
| [ ] | 267 | **Regex Visualizer** | `regex-visualizer` | `developer-engine` | ⏳ Untested | — |
| [ ] | 268 | **Regex Generator** | `regex-generator` | — | 💤 Queued | — |
| [ ] | 269 | **JWT Decoder** | `jwt-decoder` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 270 | **JWT Generator** | `jwt-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 271 | **JWT Validator** | `jwt-validator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 272 | **Base64 Encoder** | `base64-encoder` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 273 | **Base64 Decoder** | `base64-decoder` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 274 | **URL Encoder** | `url-encoder` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 275 | **URL Decoder** | `url-decoder` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 276 | **HTML Entity Encoder** | `html-entity-encoder` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 277 | **HTML Entity Decoder** | `html-entity-decoder` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 278 | **Unicode Encoder** | `unicode-encoder` | `text-engine` | ⏳ Untested | — |
| [ ] | 279 | **Unicode Decoder** | `unicode-decoder` | `text-engine` | ⏳ Untested | — |
| [ ] | 280 | **Hash Generator** | `hash-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 281 | **MD5 Generator** | `md5-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 282 | **SHA-1 Generator** | `sha-1-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 283 | **SHA-256 Generator** | `sha-256-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 284 | **SHA-512 Generator** | `sha-512-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 285 | **UUID Generator** | `uuid-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 286 | **ULID Generator** | `ulid-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 287 | **NanoID Generator** | `nanoid-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 289 | **Cron Expression Parser** | `cron-expression-parser` | `developer-engine` | ⏳ Untested | — |
| [ ] | 290 | **Cron Expression Generator** | `cron-expression-generator` | `developer-engine` | ⏳ Untested | — |
| [ ] | 291 | **Unix Timestamp Converter** | `unix-timestamp-converter` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 292 | **Epoch Converter** | `epoch-converter` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 293 | **Color Code Converter** | `color-code-converter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 294 | **RGB to HEX** | `rgb-to-hex` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 295 | **HEX to RGB** | `hex-to-rgb` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 296 | **HSL Converter** | `hsl-converter` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 297 | **CSS Gradient Generator** | `css-gradient-generator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 298 | **CSS Box Shadow Generator** | `css-box-shadow-generator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 299 | **CSS Border Radius Generator** | `css-border-radius-generator` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 300 | **CSS Animation Generator** | `css-animation-generator` | — | 💤 Queued | — |
| [ ] | 301 | **SVG Optimizer** | `svg-optimizer` | `image-engine` | ⏳ Untested | — |
| [ ] | 302 | **SVG Viewer** | `svg-viewer` | `image-engine` | ⏳ Untested | — |
| [ ] | 303 | **SVG Path Editor** | `svg-path-editor` | — | 💤 Queued | — |
| [ ] | 304 | **QR Code Generator** | `qr-code-generator` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 305 | **QR Code Reader** | `qr-code-reader` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 306 | **Barcode Generator** | `barcode-generator` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 307 | **Diff Viewer** | `diff-viewer` | `developer-engine` | ⏳ Untested | — |
| [ ] | 308 | **Gitignore Generator** | `gitignore-generator` | `developer-engine` | ⏳ Untested | — |
| [ ] | 309 | **Dockerfile Formatter** | `dockerfile-formatter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 310 | **Markdown Table Generator** | `markdown-table-generator` | `csv-engine` | ⏳ Untested | — |
| [ ] | 631 | **SQL Query Builder** | `sql-query-builder` | `developer-engine` | ⏳ Untested | — |
| [ ] | 632 | **SQL Schema Visualizer** | `sql-schema-visualizer` | `developer-engine` | ⏳ Untested | — |
| [ ] | 633 | **ER Diagram Generator** | `er-diagram-generator` | `developer-engine` | ⏳ Untested | — |
| [ ] | 634 | **Mermaid Diagram Editor** | `mermaid-diagram-editor` | — | 💤 Queued | — |
| [ ] | 635 | **PlantUML Previewer** | `plantuml-previewer` | — | 💤 Queued | — |
| [ ] | 636 | **Markdown to Mermaid** | `markdown-to-mermaid` | — | 💤 Queued | — |
| [ ] | 637 | **API Documentation Previewer** | `api-documentation-previewer` | — | 💤 Queued | — |
| [ ] | 638 | **OpenAPI/Swagger Viewer** | `openapi-swagger-viewer` | — | 💤 Queued | — |
| [ ] | 639 | **GraphQL Query Formatter** | `graphql-query-formatter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 640 | **GraphQL Explorer** | `graphql-explorer` | — | 💤 Queued | — |
| [ ] | 641 | **Protobuf Decoder** | `protobuf-decoder` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 642 | **JWT Expiration Calculator** | `jwt-expiration-calculator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 643 | **Semantic Version Comparator** | `semantic-version-comparator` | `developer-engine` | ⏳ Untested | — |
| [ ] | 644 | **Semantic Version Generator** | `semantic-version-generator` | `developer-engine` | ⏳ Untested | — |
| [ ] | 645 | **Git Command Generator** | `git-command-generator` | — | 💤 Queued | — |
| [ ] | 646 | **GitHub README Generator** | `github-readme-generator` | — | 💤 Queued | — |
| [ ] | 647 | **Changelog Generator** | `changelog-generator` | — | 💤 Queued | — |
| [ ] | 648 | **License Generator** | `license-generator` | — | 💤 Queued | — |
| [ ] | 649 | **Environment Variable Formatter** | `environment-variable-formatter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 650 | **.env Editor** | `env-editor` | `developer-engine` | ⏳ Untested | — |
| [ ] | 651 | **TOML Formatter** | `toml-formatter` | `json-engine` | ⏳ Untested | — |
| [ ] | 652 | **INI Formatter** | `ini-formatter` | `json-engine` | ⏳ Untested | — |
| [ ] | 653 | **CSV to SQL Converter** | `csv-to-sql-converter` | `csv-engine` | ⏳ Untested | — |
| [ ] | 654 | **SQL to JSON Converter** | `sql-to-json-converter` | — | 💤 Queued | — |
| [ ] | 655 | **Stack Trace Formatter** | `stack-trace-formatter` | — | 💤 Queued | — |
| [ ] | 656 | **Log Viewer** | `log-viewer` | — | 💤 Queued | — |
| [ ] | 657 | **Log Prettifier** | `log-prettifier` | — | 💤 Queued | — |
| [ ] | 658 | **Regex Match Extractor** | `regex-match-extractor` | `developer-engine` | ⏳ Untested | — |
| [ ] | 659 | **HTML to JSX Converter** | `html-to-jsx-converter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 660 | **Markdown to JSX Converter** | `markdown-to-jsx-converter` | — | 💤 Queued | — |
| [ ] | 100057 | **Json To Toml** | `json-to-toml` | `json-engine` | ⏳ Untested | — |
| [ ] | 100058 | **Json To Typescript** | `json-to-typescript` | `json-engine` | ⏳ Untested | — |
| [ ] | 100059 | **Json To Rust Struct** | `json-to-rust-struct` | `json-engine` | ⏳ Untested | — |
| [ ] | 100060 | **Json To Go Struct** | `json-to-go-struct` | `json-engine` | ⏳ Untested | — |
| [ ] | 100061 | **Json Diff** | `json-diff` | `json-engine` | ⏳ Untested | — |
| [ ] | 100062 | **Json Compare** | `json-compare` | `json-engine` | ⏳ Untested | — |
| [ ] | 100063 | **Json Path Query** | `json-path-query` | `json-engine` | ⏳ Untested | — |
| [ ] | 100064 | **Json Schema Validator** | `json-schema-validator` | `json-engine` | ⏳ Untested | — |
| [ ] | 100065 | **Json Schema Generator** | `json-schema-generator` | `json-engine` | ⏳ Untested | — |
| [ ] | 100066 | **Json Escape** | `json-escape` | `json-engine` | ⏳ Untested | — |
| [ ] | 100067 | **Json Unescape** | `json-unescape` | `json-engine` | ⏳ Untested | — |
| [ ] | 100068 | **Json Array Sorter** | `json-array-sorter` | `json-engine` | ⏳ Untested | — |
| [ ] | 100069 | **Json Key Sorter** | `json-key-sorter` | `json-engine` | ⏳ Untested | — |
| [ ] | 100070 | **Json Flattener** | `json-flattener` | `json-engine` | ⏳ Untested | — |
| [ ] | 100071 | **Json Unflattener** | `json-unflattener` | `json-engine` | ⏳ Untested | — |
| [ ] | 100072 | **Json To Table** | `json-to-table` | `json-engine` | ⏳ Untested | — |
| [ ] | 100104 | **XML Minifier** | `xml-minifier` | `json-engine` | ⏳ Untested | — |
| [ ] | 100105 | **Yaml Formatter** | `yaml-formatter` | `json-engine` | ⏳ Untested | — |
| [ ] | 100106 | **Yaml Validator** | `yaml-validator` | `json-engine` | ⏳ Untested | — |
| [ ] | 100107 | **Toml Validator** | `toml-validator` | `json-engine` | ⏳ Untested | — |
| [ ] | 100108 | **Toml To Json** | `toml-to-json` | `json-engine` | ⏳ Untested | — |
| [ ] | 100109 | **INI To Json** | `ini-to-json` | `json-engine` | ⏳ Untested | — |
| [ ] | 100110 | **Plist Viewer** | `plist-viewer` | `json-engine` | ⏳ Untested | — |
| [ ] | 100117 | **Hex Encoder** | `hex-encoder` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 100118 | **Hex Decoder** | `hex-decoder` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 100121 | **URL Encode Decode** | `url-encode-decode` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 100122 | **Base64 Encode Decode** | `base64-encode-decode` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 100123 | **Html Entity Encode Decode** | `html-entity-encode-decode` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 100124 | **Punycode Converter** | `punycode-converter` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 100125 | **Quoted Printable Converter** | `quoted-printable-converter` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 100126 | **Mime Word Decoder** | `mime-word-decoder` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 100127 | **Messagepack Converter** | `messagepack-converter` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 100128 | **Cbor Viewer** | `cbor-viewer` | `encoding-engine` | ⏳ Untested | — |
| [ ] | 100129 | **SQL Validator** | `sql-validator` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100130 | **Graphql Validator** | `graphql-validator` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100131 | **Graphql Schema Viewer** | `graphql-schema-viewer` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100132 | **Typescript Formatter** | `typescript-formatter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100133 | **Yaml Linter** | `yaml-linter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100134 | **Docker Compose Validator** | `docker-compose-validator` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100135 | **Patch Parser** | `patch-parser` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100136 | **Chmod Calculator** | `chmod-calculator` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100137 | **Http Status Lookup** | `http-status-lookup` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100138 | **URL Parser** | `url-parser` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100139 | **Query String Parser** | `query-string-parser` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100140 | **Json To Code** | `json-to-code` | `json-engine` | ⏳ Untested | — |
| [ ] | 100141 | **Code Complexity Analyzer** | `code-complexity-analyzer` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100142 | **Code Line Counter** | `code-line-counter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100143 | **Code Minifier** | `code-minifier` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100144 | **Code Beautifier** | `code-beautifier` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100145 | **Code Diff** | `code-diff` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100146 | **Code Duplicate Detector** | `code-duplicate-detector` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100147 | **Import Sorter** | `import-sorter` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100148 | **Unused Import Checker** | `unused-import-checker` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100149 | **Dependency License Scanner** | `dependency-license-scanner` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100150 | **SQL Query Analyzer** | `sql-query-analyzer` | `developer-engine` | ⏳ Untested | — |
| [ ] | 100298 | **CSV Chart Generator** | `csv-chart-generator` | `dataviz-engine` | ⏳ Untested | — |
| [ ] | 100299 | **Json Chart Generator** | `json-chart-generator` | `dataviz-engine` | ⏳ Untested | — |
| [ ] | 100300 | **Histogram Generator** | `histogram-generator` | `dataviz-engine` | ⏳ Untested | — |
| [ ] | 100301 | **Frequency Distribution** | `frequency-distribution` | `dataviz-engine` | ⏳ Untested | — |
| [ ] | 100302 | **Pivot Table Lite** | `pivot-table-lite` | `dataviz-engine` | ⏳ Untested | — |
| [ ] | 100303 | **Data Profiler** | `data-profiler` | `dataviz-engine` | ⏳ Untested | — |
| [ ] | 100304 | **Descriptive Statistics** | `descriptive-statistics` | `dataviz-engine` | ⏳ Untested | — |
| [ ] | 100305 | **Outlier Detector** | `outlier-detector` | `dataviz-engine` | ⏳ Untested | — |
| [ ] | 100306 | **Data Sampler** | `data-sampler` | `dataviz-engine` | ⏳ Untested | — |

### Everyday Tools (41 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 661 | **Grocery List Maker** | `grocery-list-maker` | — | 💤 Queued | — |
| [ ] | 662 | **Recipe Measurement Converter** | `recipe-measurement-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 663 | **Cooking Temperature Converter** | `cooking-temperature-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 664 | **Cooking Timer** | `cooking-timer` | — | 💤 Queued | — |
| [ ] | 665 | **Water Intake Tracker** | `water-intake-tracker` | — | 💤 Queued | — |
| [ ] | 666 | **Sleep Calculator** | `sleep-calculator` | — | 💤 Queued | — |
| [ ] | 667 | **Travel Budget Calculator** | `travel-budget-calculator` | — | 💤 Queued | — |
| [ ] | 668 | **Trip Fuel Calculator** | `trip-fuel-calculator` | `units-engine` | ⏳ Untested | — |
| [ ] | 669 | **Road Trip Planner** | `road-trip-planner` | — | 💤 Queued | — |
| [ ] | 670 | **Packing Checklist Generator** | `packing-checklist-generator` | — | 💤 Queued | — |
| [ ] | 671 | **Time Zone Meeting Planner** | `time-zone-meeting-planner` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 672 | **Age in Days Calculator** | `age-in-days-calculator` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 673 | **Birthday Countdown** | `birthday-countdown` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 674 | **Wedding Budget Calculator** | `wedding-budget-calculator` | — | 💤 Queued | — |
| [ ] | 675 | **Rent Split Calculator** | `rent-split-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 676 | **Electricity Bill Calculator** | `electricity-bill-calculator` | `solar-engine` | ⏳ Untested | — |
| [ ] | 677 | **Water Bill Calculator** | `water-bill-calculator` | — | 💤 Queued | — |
| [ ] | 678 | **Internet Data Usage Calculator** | `internet-data-usage-calculator` | — | 💤 Queued | — |
| [ ] | 679 | **Mobile Recharge Comparison Calculator** | `mobile-recharge-comparison-calculator` | — | 💤 Queued | — |
| [ ] | 680 | **Vehicle Mileage Calculator** | `vehicle-mileage-calculator` | `units-engine` | ⏳ Untested | — |
| [ ] | 681 | **Car Maintenance Tracker** | `car-maintenance-tracker` | — | 💤 Queued | — |
| [ ] | 682 | **Bike Maintenance Tracker** | `bike-maintenance-tracker` | — | 💤 Queued | — |
| [ ] | 683 | **Loan Payoff Calculator** | `loan-payoff-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 684 | **Savings Goal Calculator** | `savings-goal-calculator` | `finance-engine` | ⏳ Untested | — |
| [ ] | 685 | **Daily Expense Splitter** | `daily-expense-splitter` | `finance-engine` | ⏳ Untested | — |
| [ ] | 100218 | **Timestamp To Date** | `timestamp-to-date` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 100219 | **Date To Timestamp** | `date-to-timestamp` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 100220 | **Business Days Calculator** | `business-days-calculator` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 100221 | **Week Number Calculator** | `week-number-calculator` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 100222 | **Cron Next Run** | `cron-next-run` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 100230 | **Wifi QR Generator** | `wifi-qr-generator` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 100231 | **Vcard QR Generator** | `vcard-qr-generator` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 100232 | **Bulk QR Generator** | `bulk-qr-generator` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 100233 | **QR Code Styler** | `qr-code-styler` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 100234 | **QR Code To SVG** | `qr-code-to-svg` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 100235 | **QR Code To PNG** | `qr-code-to-png` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 100307 | **Barcode Label Maker** | `barcode-label-maker` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 100308 | **Shipping Label Maker** | `shipping-label-maker` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 100309 | **Address Label Maker** | `address-label-maker` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 100310 | **QR Label Sheet Generator** | `qr-label-sheet-generator` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 100311 | **Printable Sticker Sheet** | `printable-sticker-sheet` | `qrbarcode-engine` | ⏳ Untested | — |

### File Tools (74 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 415 | **QR Code Scanner** | `qr-code-scanner` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 416 | **Barcode Scanner** | `barcode-scanner` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 417 | **Password Strength Checker** | `password-strength-checker` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 418 | **File Hash Checker** | `file-hash-checker` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 419 | **File Checksum Calculator** | `file-checksum-calculator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 420 | **Duplicate File Finder** | `duplicate-file-finder` | — | 💤 Queued | — |
| [ ] | 421 | **ZIP Extractor** | `zip-extractor` | `archive-engine` | ⏳ Untested | — |
| [ ] | 422 | **ZIP Creator** | `zip-creator` | `archive-engine` | ⏳ Untested | — |
| [ ] | 423 | **ZIP Compressor** | `zip-compressor` | `archive-engine` | ⏳ Untested | — |
| [ ] | 424 | **TAR Extractor** | `tar-extractor` | `archive-engine` | ⏳ Untested | — |
| [ ] | 425 | **GZIP Compressor** | `gzip-compressor` | `archive-engine` | ⏳ Untested | — |
| [ ] | 426 | **GZIP Decompressor** | `gzip-decompressor` | `archive-engine` | ⏳ Untested | — |
| [ ] | 427 | **7Z Viewer/Extractor** | `7z-viewer-extractor` | `archive-engine` | 💤 Queued | — |
| [ ] | 428 | **RAR Extractor** | `rar-extractor` | `archive-engine` | 💤 Queued | — |
| [ ] | 429 | **File Extension Detector** | `file-extension-detector` | `developer-engine` | ⏳ Untested | — |
| [ ] | 430 | **MIME Type Detector** | `mime-type-detector` | `developer-engine` | ⏳ Untested | — |
| [ ] | 431 | **File Signature Detector** | `file-signature-detector` | `developer-engine` | ⏳ Untested | — |
| [ ] | 432 | **File Size Converter** | `file-size-converter` | `units-engine` | ⏳ Untested | — |
| [ ] | 433 | **File Comparison** | `file-comparison` | `text-engine` | ⏳ Untested | — |
| [ ] | 434 | **Binary File Comparison** | `binary-file-comparison` | — | 💤 Queued | — |
| [ ] | 435 | **File Checksum Verifier** | `file-checksum-verifier` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 436 | **File Metadata Viewer** | `file-metadata-viewer` | `image-engine` | ⏳ Untested | — |
| [ ] | 437 | **File Renamer** | `file-renamer` | — | 💤 Queued | — |
| [ ] | 438 | **Bulk File Renamer** | `bulk-file-renamer` | — | 💤 Queued | — |
| [ ] | 439 | **File Splitter** | `file-splitter` | — | 💤 Queued | — |
| [ ] | 440 | **File Joiner** | `file-joiner` | — | 💤 Queued | — |
| [ ] | 441 | **CSV Viewer** | `csv-viewer` | `csv-engine` | ⏳ Untested | — |
| [ ] | 442 | **CSV Editor** | `csv-editor` | `csv-engine` | ⏳ Untested | — |
| [ ] | 443 | **CSV Cleaner** | `csv-cleaner` | `csv-engine` | ⏳ Untested | — |
| [ ] | 444 | **Excel Viewer** | `excel-viewer` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 445 | **Excel to CSV** | `excel-to-csv` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 446 | **CSV to Excel** | `csv-to-excel` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 447 | **Excel Formula Helper** | `excel-formula-helper` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 448 | **XML Viewer** | `xml-viewer` | `json-engine` | ⏳ Untested | — |
| [ ] | 449 | **EPUB Reader** | `epub-reader` | `document-engine` | ⏳ Untested | — |
| [ ] | 450 | **MOBI Reader** | `mobi-reader` | `document-engine` | ⏳ Untested | — |
| [ ] | 100035 | **Zip View** | `zip-view` | `archive-engine` | ⏳ Untested | — |
| [ ] | 100036 | **Zip Repair** | `zip-repair` | `archive-engine` | ⏳ Untested | — |
| [ ] | 100037 | **Zip Password Check** | `zip-password-check` | `archive-engine` | ⏳ Untested | — |
| [ ] | 100038 | **Tar Create** | `tar-create` | `archive-engine` | ⏳ Untested | — |
| [ ] | 100039 | **File Archive Converter** | `file-archive-converter` | `archive-engine` | ⏳ Untested | — |
| [ ] | 100040 | **Archive List Files** | `archive-list-files` | `archive-engine` | ⏳ Untested | — |
| [ ] | 100041 | **Archive Test Integrity** | `archive-test-integrity` | `archive-engine` | ⏳ Untested | — |
| [ ] | 100073 | **CSV Validator** | `csv-validator` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100074 | **CSV To TSV** | `csv-to-tsv` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100075 | **TSV To CSV** | `tsv-to-csv` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100076 | **CSV Merge** | `csv-merge` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100077 | **CSV Split** | `csv-split` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100078 | **CSV Deduplicator** | `csv-deduplicator` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100079 | **CSV Column Extractor** | `csv-column-extractor` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100080 | **CSV Column Reorder** | `csv-column-reorder` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100081 | **CSV Filter** | `csv-filter` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100082 | **CSV Sort** | `csv-sort` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100083 | **CSV To Jsonl** | `csv-to-jsonl` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100084 | **Jsonl To CSV** | `jsonl-to-csv` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100085 | **CSV Statistics** | `csv-statistics` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100086 | **CSV Transpose** | `csv-transpose` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100087 | **CSV Compare** | `csv-compare` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100088 | **CSV Diff** | `csv-diff` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100089 | **CSV Encoding Converter** | `csv-encoding-converter` | `csv-engine` | ⏳ Untested | — |
| [ ] | 100090 | **Xlsx To CSV** | `xlsx-to-csv` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100091 | **CSV To Xlsx** | `csv-to-xlsx` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100092 | **Xlsx To PDF** | `xlsx-to-pdf` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100093 | **Xlsx To Json** | `xlsx-to-json` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100094 | **Json To Xlsx** | `json-to-xlsx` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100095 | **Xlsx Merge** | `xlsx-merge` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100096 | **Xlsx Split Sheets** | `xlsx-split-sheets` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100097 | **Xlsx Remove Duplicates** | `xlsx-remove-duplicates` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100098 | **Xlsx Cleaner** | `xlsx-cleaner` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100099 | **Xlsx Formula Inspector** | `xlsx-formula-inspector` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100100 | **Xlsx Diff** | `xlsx-diff` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100101 | **ODS To Xlsx** | `ods-to-xlsx` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100102 | **Xlsx To ODS** | `xlsx-to-ods` | `spreadsheet-engine` | ⏳ Untested | — |
| [ ] | 100103 | **Spreadsheet Column Letter** | `spreadsheet-column-letter` | `spreadsheet-engine` | ⏳ Untested | — |

### Image Tools (71 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 51 | **Image Cropper** | `image-cropper` | `image-engine` | ⏳ Untested | — |
| [ ] | 52 | **Image Resizer** | `image-resizer` | `image-engine` | ⏳ Untested | — |
| [ ] | 53 | **Image Compressor** | `image-compressor` | `image-engine` | ⏳ Untested | — |
| [ ] | 54 | **Image Converter** | `image-converter` | `image-engine` | ⏳ Untested | — |
| [ ] | 55 | **JPG to PNG** | `jpg-to-png` | `image-engine` | ⏳ Untested | — |
| [ ] | 56 | **PNG to JPG** | `png-to-jpg` | `image-engine` | ⏳ Untested | — |
| [ ] | 57 | **PNG to WebP** | `png-to-webp` | `image-engine` | ⏳ Untested | — |
| [ ] | 58 | **WebP to PNG** | `webp-to-png` | `image-engine` | ⏳ Untested | — |
| [ ] | 59 | **WebP to JPG** | `webp-to-jpg` | `image-engine` | ⏳ Untested | — |
| [ ] | 60 | **AVIF Converter** | `avif-converter` | `image-engine` | ⏳ Untested | — |
| [ ] | 61 | **HEIC to JPG** | `heic-to-jpg` | `image-engine` | 💤 Queued | — |
| [ ] | 62 | **SVG to PNG** | `svg-to-png` | `image-engine` | ⏳ Untested | — |
| [ ] | 63 | **SVG to JPG** | `svg-to-jpg` | `image-engine` | ⏳ Untested | — |
| [ ] | 64 | **GIF to PNG** | `gif-to-png` | `image-engine` | ⏳ Untested | — |
| [ ] | 65 | **Image Background Remover** | `image-background-remover` | `image-engine` | ⏳ Untested | — |
| [ ] | 66 | **Image Background Changer** | `image-background-changer` | — | 💤 Queued | — |
| [ ] | 67 | **Image Blur Tool** | `image-blur-tool` | `image-engine` | ⏳ Untested | — |
| [ ] | 68 | **Image Sharpen Tool** | `image-sharpen-tool` | `image-engine` | ⏳ Untested | — |
| [ ] | 69 | **Image Denoiser** | `image-denoiser` | `image-engine` | ⏳ Untested | — |
| [ ] | 70 | **Image Upscaler** | `image-upscaler` | — | 💤 Queued | — |
| [ ] | 71 | **Image Colorizer** | `image-colorizer` | — | 💤 Queued | — |
| [ ] | 72 | **Image Brightness Editor** | `image-brightness-editor` | `image-engine` | ⏳ Untested | — |
| [ ] | 73 | **Image Contrast Editor** | `image-contrast-editor` | `image-engine` | ⏳ Untested | — |
| [ ] | 74 | **Image Saturation Editor** | `image-saturation-editor` | `image-engine` | ⏳ Untested | — |
| [ ] | 75 | **Image Exposure Editor** | `image-exposure-editor` | `image-engine` | ⏳ Untested | — |
| [ ] | 76 | **Image Hue Editor** | `image-hue-editor` | `image-engine` | ⏳ Untested | — |
| [ ] | 77 | **Image Temperature Editor** | `image-temperature-editor` | `image-engine` | ⏳ Untested | — |
| [ ] | 78 | **Image Filters** | `image-filters` | `image-engine` | ⏳ Untested | — |
| [ ] | 79 | **Grayscale Converter** | `grayscale-converter` | `image-engine` | ⏳ Untested | — |
| [ ] | 80 | **Black and White Converter** | `black-and-white-converter` | `image-engine` | ⏳ Untested | — |
| [ ] | 81 | **Sepia Filter** | `sepia-filter` | `image-engine` | ⏳ Untested | — |
| [ ] | 82 | **Image Pixelator** | `image-pixelator` | `image-engine` | ⏳ Untested | — |
| [ ] | 83 | **Image Watermark** | `image-watermark` | `image-engine` | ⏳ Untested | — |
| [ ] | 84 | **Remove Image Watermark** | `remove-image-watermark` | — | 💤 Queued | — |
| [ ] | 85 | **Image Border Generator** | `image-border-generator` | `image-engine` | ⏳ Untested | — |
| [ ] | 86 | **Rounded Corner Generator** | `rounded-corner-generator` | `image-engine` | ⏳ Untested | — |
| [ ] | 87 | **Image Shadow Generator** | `image-shadow-generator` | — | 💤 Queued | — |
| [ ] | 88 | **Image Flip** | `image-flip` | `image-engine` | ⏳ Untested | — |
| [ ] | 89 | **Image Rotate** | `image-rotate` | `image-engine` | ⏳ Untested | — |
| [ ] | 90 | **Image Perspective Correction** | `image-perspective-correction` | — | 💤 Queued | — |
| [ ] | 91 | **Photo Collage Maker** | `photo-collage-maker` | `image-engine` | ⏳ Untested | — |
| [ ] | 92 | **Photo Grid Maker** | `photo-grid-maker` | `image-engine` | ⏳ Untested | — |
| [ ] | 93 | **Meme Generator** | `meme-generator` | `image-engine` | ⏳ Untested | — |
| [ ] | 94 | **Image Caption Generator** | `image-caption-generator` | `image-engine` | ⏳ Untested | — |
| [ ] | 95 | **Passport Photo Maker** | `passport-photo-maker` | `image-engine` | ⏳ Untested | — |
| [ ] | 96 | **ID Photo Maker** | `id-photo-maker` | `image-engine` | ⏳ Untested | — |
| [ ] | 97 | **Profile Picture Maker** | `profile-picture-maker` | `image-engine` | ⏳ Untested | — |
| [ ] | 98 | **Thumbnail Maker** | `thumbnail-maker` | `image-engine` | ⏳ Untested | — |
| [ ] | 99 | **Image Comparison Tool** | `image-comparison-tool` | `image-engine` | ⏳ Untested | — |
| [ ] | 100 | **Image Metadata Viewer** | `image-metadata-viewer` | `image-engine` | ⏳ Untested | — |
| [ ] | 101 | **EXIF Remover** | `exif-remover` | `image-engine` | ⏳ Untested | — |
| [ ] | 102 | **EXIF Editor** | `exif-editor` | `image-engine` | ⏳ Untested | — |
| [ ] | 103 | **Image Color Picker** | `image-color-picker` | `image-engine` | ⏳ Untested | — |
| [ ] | 104 | **Image Palette Extractor** | `image-palette-extractor` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 105 | **Dominant Color Detector** | `dominant-color-detector` | `image-engine` | ⏳ Untested | — |
| [ ] | 106 | **Image to Base64** | `image-to-base64` | `image-engine` | ⏳ Untested | — |
| [ ] | 107 | **Base64 to Image** | `base64-to-image` | `image-engine` | ⏳ Untested | — |
| [ ] | 108 | **Image to ASCII** | `image-to-ascii` | `image-engine` | ⏳ Untested | — |
| [ ] | 109 | **Image to Pixel Art** | `image-to-pixel-art` | `image-engine` | ⏳ Untested | — |
| [ ] | 110 | **Screenshot Beautifier** | `screenshot-beautifier` | — | 💤 Queued | — |
| [ ] | 100011 | **Image Invert** | `image-invert` | `image-engine` | ⏳ Untested | — |
| [ ] | 100012 | **Image Remove Border** | `image-remove-border` | `image-engine` | ⏳ Untested | — |
| [ ] | 100013 | **Image DPI Changer** | `image-dpi-changer` | `image-engine` | ⏳ Untested | — |
| [ ] | 100014 | **Image Palette Generator** | `image-palette-generator` | `image-engine` | ⏳ Untested | — |
| [ ] | 100015 | **Image Average Color** | `image-average-color` | `image-engine` | ⏳ Untested | — |
| [ ] | 100016 | **Image Dimensions** | `image-dimensions` | `image-engine` | ⏳ Untested | — |
| [ ] | 100017 | **Jpeg To Webp** | `jpeg-to-webp` | `image-engine` | ⏳ Untested | — |
| [ ] | 100018 | **GIF Frame Extractor** | `gif-frame-extractor` | `image-engine` | ⏳ Untested | — |
| [ ] | 100019 | **Image Sprite Generator** | `image-sprite-generator` | `image-engine` | ⏳ Untested | — |
| [ ] | 100020 | **Image Overlay** | `image-overlay` | `image-engine` | ⏳ Untested | — |
| [ ] | 100021 | **Image Mosaic** | `image-mosaic` | `image-engine` | ⏳ Untested | — |

### Math Tools (39 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 556 | **Fraction Calculator** | `fraction-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 557 | **Algebra Calculator** | `algebra-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 558 | **Equation Solver** | `equation-solver` | `math-engine` | ⏳ Untested | — |
| [ ] | 559 | **Quadratic Equation Solver** | `quadratic-equation-solver` | `math-engine` | ⏳ Untested | — |
| [ ] | 560 | **Matrix Calculator** | `matrix-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 561 | **Matrix Multiplication** | `matrix-multiplication` | `math-engine` | ⏳ Untested | — |
| [ ] | 562 | **Determinant Calculator** | `determinant-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 563 | **Derivative Calculator** | `derivative-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 564 | **Integral Calculator** | `integral-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 565 | **Limit Calculator** | `limit-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 566 | **Scientific Notation Converter** | `scientific-notation-converter` | `math-engine` | ⏳ Untested | — |
| [ ] | 567 | **Prime Number Checker** | `prime-number-checker` | `math-engine` | ⏳ Untested | — |
| [ ] | 568 | **Prime Factorization** | `prime-factorization` | `math-engine` | ⏳ Untested | — |
| [ ] | 569 | **GCD Calculator** | `gcd-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 570 | **LCM Calculator** | `lcm-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 571 | **Percentage Change Calculator** | `percentage-change-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 572 | **Standard Deviation Calculator** | `standard-deviation-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 573 | **Mean/Median/Mode Calculator** | `mean-median-mode-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 574 | **Probability Calculator** | `probability-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 575 | **Permutation Calculator** | `permutation-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 576 | **Combination Calculator** | `combination-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 577 | **Sequence Generator** | `sequence-generator` | `math-engine` | ⏳ Untested | — |
| [ ] | 578 | **Fibonacci Generator** | `fibonacci-generator` | `math-engine` | ⏳ Untested | — |
| [ ] | 579 | **Graph Plotter** | `graph-plotter` | `math-engine` | ⏳ Untested | — |
| [ ] | 580 | **Unit Circle Calculator** | `unit-circle-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 581 | **Triangle Calculator** | `triangle-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 582 | **Pythagorean Theorem Calculator** | `pythagorean-theorem-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 583 | **Geometry Calculator** | `geometry-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 584 | **Coordinate Distance Calculator** | `coordinate-distance-calculator` | `geo-engine` | ⏳ Untested | — |
| [ ] | 585 | **Slope Calculator** | `slope-calculator` | — | 💤 Queued | — |
| [ ] | 100151 | **Ratio Calculator** | `ratio-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 100152 | **Average Calculator** | `average-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 100153 | **Variance Calculator** | `variance-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 100154 | **Matrix Inverse** | `matrix-inverse` | `math-engine` | ⏳ Untested | — |
| [ ] | 100155 | **Trigonometry Calculator** | `trigonometry-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 100156 | **Logarithm Calculator** | `logarithm-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 100157 | **Exponent Calculator** | `exponent-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 100158 | **Binary Calculator** | `binary-calculator` | `math-engine` | ⏳ Untested | — |
| [ ] | 100159 | **Number To Words** | `number-to-words` | `math-engine` | ⏳ Untested | — |

### PDF & Documents (75 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [x] | 3 | **Compress PDF** | `compress-pdf` | `pdf-engine` | ✅ Passed | Tested full width and compression |
| [ ] | 1 | **Merge PDF** | `merge-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 2 | **Split PDF** | `split-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 4 | **PDF to Word** | `pdf-to-word` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 5 | **Word to PDF** | `word-to-pdf` | `document-engine` | ⏳ Untested | — |
| [ ] | 6 | **PDF to Excel** | `pdf-to-excel` | — | 💤 Queued | — |
| [ ] | 7 | **Excel to PDF** | `excel-to-pdf` | — | 💤 Queued | — |
| [ ] | 8 | **PDF to PowerPoint** | `pdf-to-powerpoint` | — | 💤 Queued | — |
| [ ] | 9 | **PowerPoint to PDF** | `powerpoint-to-pdf` | — | 💤 Queued | — |
| [ ] | 10 | **PDF to JPG** | `pdf-to-jpg` | `pdf-engine` | 💤 Queued | — |
| [ ] | 11 | **JPG to PDF** | `jpg-to-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 12 | **PDF to PNG** | `pdf-to-png` | `pdf-engine` | 💤 Queued | — |
| [ ] | 13 | **PNG to PDF** | `png-to-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 14 | **PDF to WebP** | `pdf-to-webp` | `pdf-engine` | 💤 Queued | — |
| [ ] | 15 | **PDF to Text** | `pdf-to-text` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 16 | **PDF to HTML** | `pdf-to-html` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 17 | **PDF to Markdown** | `pdf-to-markdown` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 18 | **Markdown to PDF** | `markdown-to-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 19 | **PDF to EPUB** | `pdf-to-epub` | `document-engine` | ⏳ Untested | — |
| [ ] | 20 | **PDF to CSV** | `pdf-to-csv` | — | 💤 Queued | — |
| [ ] | 21 | **Rotate PDF** | `rotate-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 22 | **Rearrange PDF Pages** | `rearrange-pdf-pages` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 23 | **Delete PDF Pages** | `delete-pdf-pages` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 24 | **Extract PDF Pages** | `extract-pdf-pages` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 25 | **Duplicate PDF Pages** | `duplicate-pdf-pages` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 26 | **Add PDF Page Numbers** | `add-pdf-page-numbers` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 27 | **Add PDF Watermark** | `add-pdf-watermark` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 28 | **Remove PDF Watermark** | `remove-pdf-watermark` | — | 💤 Queued | — |
| [ ] | 29 | **Add PDF Password** | `add-pdf-password` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 30 | **Remove PDF Password** | `remove-pdf-password` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 31 | **Unlock PDF** | `unlock-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 32 | **Protect PDF** | `protect-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 33 | **Sign PDF** | `sign-pdf` | — | 💤 Queued | — |
| [ ] | 34 | **Fill PDF Forms** | `fill-pdf-forms` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 35 | **Edit PDF Text** | `edit-pdf-text` | — | 💤 Queued | — |
| [ ] | 36 | **Highlight PDF** | `highlight-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 37 | **Annotate PDF** | `annotate-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 38 | **Draw on PDF** | `draw-on-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 39 | **Crop PDF** | `crop-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 40 | **Resize PDF Pages** | `resize-pdf-pages` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 41 | **Change PDF Page Size** | `change-pdf-page-size` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 42 | **PDF Metadata Editor** | `pdf-metadata-editor` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 43 | **PDF Metadata Viewer** | `pdf-metadata-viewer` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 44 | **PDF OCR** | `pdf-ocr` | — | 💤 Queued | — |
| [ ] | 45 | **PDF Text Extractor** | `pdf-text-extractor` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 46 | **PDF Image Extractor** | `pdf-image-extractor` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 47 | **PDF Comparison** | `pdf-comparison` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 48 | **PDF Repair** | `pdf-repair` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 49 | **Flatten PDF Forms** | `flatten-pdf-forms` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 50 | **PDF to Grayscale** | `pdf-to-grayscale` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 100000 | **PDF Add Header Footer** | `pdf-add-header-footer` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 100001 | **PDF Permission Inspector** | `pdf-permission-inspector` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 100002 | **PDF Overlay** | `pdf-overlay` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 100003 | **PDF Stamp** | `pdf-stamp` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 100004 | **PDF Form Field Inspector** | `pdf-form-field-inspector` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 100005 | **PDF Bookmark Editor** | `pdf-bookmark-editor` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 100006 | **PDF Outline Extractor** | `pdf-outline-extractor` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 100007 | **PDF Page Size Checker** | `pdf-page-size-checker` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 100008 | **PDF Linearize** | `pdf-linearize` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 100009 | **PDF Redact** | `pdf-redact` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 100010 | **PDF Signature Inspector** | `pdf-signature-inspector` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 100244 | **Epub To Html** | `epub-to-html` | `document-engine` | ⏳ Untested | — |
| [ ] | 100245 | **Ebook Metadata Editor** | `ebook-metadata-editor` | `document-engine` | ⏳ Untested | — |
| [ ] | 100246 | **Ebook Cover Extractor** | `ebook-cover-extractor` | `document-engine` | ⏳ Untested | — |
| [ ] | 100247 | **Ebook Format Converter** | `ebook-format-converter` | `document-engine` | ⏳ Untested | — |
| [ ] | 100248 | **Epub Validator** | `epub-validator` | `document-engine` | ⏳ Untested | — |
| [ ] | 100274 | **Docx To Html** | `docx-to-html` | `document-engine` | ⏳ Untested | — |
| [ ] | 100275 | **Docx To Text** | `docx-to-text` | `document-engine` | ⏳ Untested | — |
| [ ] | 100276 | **Docx Metadata Viewer** | `docx-metadata-viewer` | `document-engine` | ⏳ Untested | — |
| [ ] | 100277 | **Docx Template Fill** | `docx-template-fill` | `document-engine` | ⏳ Untested | — |
| [ ] | 100278 | **Odt To Docx** | `odt-to-docx` | `document-engine` | ⏳ Untested | — |
| [ ] | 100279 | **RTF To Text** | `rtf-to-text` | `document-engine` | ⏳ Untested | — |
| [ ] | 100280 | **RTF To Html** | `rtf-to-html` | `document-engine` | ⏳ Untested | — |
| [ ] | 100281 | **Pptx To PDF** | `pptx-to-pdf` | `document-engine` | ⏳ Untested | — |
| [ ] | 100282 | **Pptx Metadata Viewer** | `pptx-metadata-viewer` | `document-engine` | ⏳ Untested | — |

### Productivity (39 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 391 | **To-do List** | `to-do-list` | — | 💤 Queued | — |
| [ ] | 392 | **Kanban Board** | `kanban-board` | — | 💤 Queued | — |
| [ ] | 393 | **Pomodoro Timer** | `pomodoro-timer` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 394 | **Stopwatch** | `stopwatch` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 395 | **Countdown Timer** | `countdown-timer` | — | 💤 Queued | — |
| [ ] | 396 | **Alarm Clock** | `alarm-clock` | — | 💤 Queued | — |
| [ ] | 397 | **World Clock** | `world-clock` | — | 💤 Queued | — |
| [ ] | 398 | **Calendar Generator** | `calendar-generator` | `datetime-engine` | ⏳ Untested | — |
| [ ] | 399 | **Event Planner** | `event-planner` | — | 💤 Queued | — |
| [ ] | 400 | **Daily Planner** | `daily-planner` | — | 💤 Queued | — |
| [ ] | 401 | **Habit Tracker** | `habit-tracker` | — | 💤 Queued | — |
| [ ] | 402 | **Notes App** | `notes-app` | — | 💤 Queued | — |
| [ ] | 403 | **Sticky Notes** | `sticky-notes` | — | 💤 Queued | — |
| [ ] | 404 | **Clipboard Manager** | `clipboard-manager` | — | 💤 Queued | — |
| [ ] | 405 | **Meeting Agenda Generator** | `meeting-agenda-generator` | — | 💤 Queued | — |
| [ ] | 406 | **Random Number Generator** | `random-number-generator` | `math-engine` | ⏳ Untested | — |
| [ ] | 407 | **Random Name Picker** | `random-name-picker` | — | 💤 Queued | — |
| [ ] | 408 | **Decision Wheel** | `decision-wheel` | — | 💤 Queued | — |
| [ ] | 409 | **Spin the Wheel** | `spin-the-wheel` | — | 💤 Queued | — |
| [ ] | 410 | **Coin Flip** | `coin-flip` | — | 💤 Queued | — |
| [ ] | 411 | **Dice Roller** | `dice-roller` | — | 💤 Queued | — |
| [ ] | 412 | **Team Randomizer** | `team-randomizer` | — | 💤 Queued | — |
| [ ] | 413 | **Group Maker** | `group-maker` | — | 💤 Queued | — |
| [ ] | 414 | **Raffle Picker** | `raffle-picker` | — | 💤 Queued | — |
| [ ] | 100283 | **Invoice Generator** | `invoice-generator` | `business-engine` | ⏳ Untested | — |
| [ ] | 100284 | **Quotation Generator** | `quotation-generator` | `business-engine` | ⏳ Untested | — |
| [ ] | 100285 | **Purchase Order Generator** | `purchase-order-generator` | `business-engine` | ⏳ Untested | — |
| [ ] | 100286 | **Profit Loss Calculator** | `profit-loss-calculator` | `business-engine` | ⏳ Untested | — |
| [ ] | 100287 | **Markup Margin Calculator** | `markup-margin-calculator` | `business-engine` | ⏳ Untested | — |
| [ ] | 100288 | **Inventory Turnover Calculator** | `inventory-turnover-calculator` | `business-engine` | ⏳ Untested | — |
| [ ] | 100289 | **Stock Reorder Calculator** | `stock-reorder-calculator` | `business-engine` | ⏳ Untested | — |
| [ ] | 100290 | **Discount Pricing Calculator** | `discount-pricing-calculator` | `business-engine` | ⏳ Untested | — |
| [ ] | 100291 | **Sales Tax Calculator** | `sales-tax-calculator` | `business-engine` | ⏳ Untested | — |
| [ ] | 100292 | **Commission Calculator** | `commission-calculator` | `business-engine` | ⏳ Untested | — |
| [ ] | 100293 | **Payroll Estimator** | `payroll-estimator` | `business-engine` | ⏳ Untested | — |
| [ ] | 100294 | **Cash Flow Forecast** | `cash-flow-forecast` | `business-engine` | ⏳ Untested | — |
| [ ] | 100295 | **Break Even Analysis** | `break-even-analysis` | `business-engine` | ⏳ Untested | — |
| [ ] | 100296 | **Business Valuation Estimator** | `business-valuation-estimator` | `business-engine` | ⏳ Untested | — |
| [ ] | 100297 | **Unit Economics Calculator** | `unit-economics-calculator` | `business-engine` | ⏳ Untested | — |

### SEO & Website (25 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 531 | **Meta Tag Generator** | `meta-tag-generator` | — | 💤 Queued | — |
| [ ] | 532 | **Meta Tag Analyzer** | `meta-tag-analyzer` | — | 💤 Queued | — |
| [ ] | 533 | **Open Graph Preview** | `open-graph-preview` | — | 💤 Queued | — |
| [ ] | 534 | **Twitter Card Preview** | `twitter-card-preview` | — | 💤 Queued | — |
| [ ] | 535 | **Sitemap Generator** | `sitemap-generator` | — | 💤 Queued | — |
| [ ] | 536 | **Robots.txt Tester** | `robots-txt-tester` | — | 💤 Queued | — |
| [ ] | 537 | **Schema Markup Generator** | `schema-markup-generator` | — | 💤 Queued | — |
| [ ] | 538 | **Structured Data Validator** | `structured-data-validator` | — | 💤 Queued | — |
| [ ] | 539 | **Canonical URL Checker** | `canonical-url-checker` | — | 💤 Queued | — |
| [ ] | 540 | **SERP Snippet Preview** | `serp-snippet-preview` | — | 💤 Queued | — |
| [ ] | 541 | **Keyword Density Checker** | `keyword-density-checker` | — | 💤 Queued | — |
| [ ] | 542 | **Word Count Analyzer** | `word-count-analyzer` | `text-engine` | ⏳ Untested | — |
| [ ] | 543 | **Heading Hierarchy Checker** | `heading-hierarchy-checker` | `developer-engine` | ⏳ Untested | — |
| [ ] | 544 | **Broken Link Checker** | `broken-link-checker` | — | 💤 Queued | — |
| [ ] | 545 | **Redirect Checker** | `redirect-checker` | — | 💤 Queued | — |
| [ ] | 546 | **URL Slug Generator** | `url-slug-generator` | `text-engine` | ⏳ Untested | — |
| [ ] | 547 | **UTM Builder** | `utm-builder` | — | 💤 Queued | — |
| [ ] | 548 | **QR Code Generator for URLs** | `qr-code-generator-for-urls` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 549 | **Page Size Analyzer** | `page-size-analyzer` | — | 💤 Queued | — |
| [ ] | 550 | **HTML Source Viewer** | `html-source-viewer` | `developer-engine` | ⏳ Untested | — |
| [ ] | 551 | **Website Screenshot Tool** | `website-screenshot-tool` | — | 💤 Queued | — |
| [ ] | 552 | **Favicon Checker** | `favicon-checker` | — | 💤 Queued | — |
| [ ] | 553 | **DNS Checker** | `dns-checker` | — | 💤 Queued | — |
| [ ] | 554 | **Website Uptime Monitor** | `website-uptime-monitor` | — | 💤 Queued | — |
| [ ] | 555 | **Lighthouse Report Viewer** | `lighthouse-report-viewer` | — | 💤 Queued | — |

### Security & Network (58 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 451 | **Password Generator** | `password-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 452 | **Password Strength Tester** | `password-strength-tester` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 453 | **Passphrase Generator** | `passphrase-generator` | — | 💤 Queued | — |
| [ ] | 454 | **Hash Calculator** | `hash-calculator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 455 | **SHA Checksum Verifier** | `sha-checksum-verifier` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 456 | **Text Encryption Tool** | `text-encryption-tool` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 457 | **Text Decryption Tool** | `text-decryption-tool` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 458 | **AES Encryption Tool** | `aes-encryption-tool` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 459 | **AES Decryption Tool** | `aes-decryption-tool` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 460 | **RSA Key Pair Generator** | `rsa-key-pair-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 461 | **Certificate Viewer** | `certificate-viewer` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 462 | **SSL Certificate Decoder** | `ssl-certificate-decoder` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 463 | **JWT Inspector** | `jwt-inspector` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 464 | **TOTP Authenticator** | `totp-authenticator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 465 | **QR-based 2FA Generator** | `qr-based-2fa-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 466 | **EXIF Data Remover** | `exif-data-remover` | `image-engine` | ⏳ Untested | — |
| [ ] | 467 | **Metadata Stripper** | `metadata-stripper` | `image-engine` | ⏳ Untested | — |
| [ ] | 468 | **URL Safety Checker** | `url-safety-checker` | — | 💤 Queued | — |
| [ ] | 469 | **HTTP Header Analyzer** | `http-header-analyzer` | — | 💤 Queued | — |
| [ ] | 470 | **CSP Header Generator** | `csp-header-generator` | — | 💤 Queued | — |
| [ ] | 471 | **Security Header Checker** | `security-header-checker` | — | 💤 Queued | — |
| [ ] | 472 | **Privacy Policy Generator** | `privacy-policy-generator` | — | 💤 Queued | — |
| [ ] | 473 | **Cookie Policy Generator** | `cookie-policy-generator` | — | 💤 Queued | — |
| [ ] | 474 | **Robots.txt Generator** | `robots-txt-generator` | — | 💤 Queued | — |
| [ ] | 475 | **Password Leak Checker** | `password-leak-checker` | — | 💤 Queued | — |
| [ ] | 476 | **IP Address Lookup** | `ip-address-lookup` | — | 💤 Queued | — |
| [ ] | 477 | **IPv4 Calculator** | `ipv4-calculator` | — | 💤 Queued | — |
| [ ] | 478 | **IPv6 Calculator** | `ipv6-calculator` | — | 💤 Queued | — |
| [ ] | 479 | **Subnet Calculator** | `subnet-calculator` | — | 💤 Queued | — |
| [ ] | 480 | **CIDR Calculator** | `cidr-calculator` | — | 💤 Queued | — |
| [ ] | 481 | **MAC Address Lookup** | `mac-address-lookup` | — | 💤 Queued | — |
| [ ] | 482 | **DNS Record Lookup** | `dns-record-lookup` | — | 💤 Queued | — |
| [ ] | 483 | **DNS Propagation Checker** | `dns-propagation-checker` | — | 💤 Queued | — |
| [ ] | 484 | **WHOIS Lookup** | `whois-lookup` | — | 💤 Queued | — |
| [ ] | 485 | **Domain Availability Checker** | `domain-availability-checker` | — | 💤 Queued | — |
| [ ] | 486 | **HTTP Status Checker** | `http-status-checker` | — | 💤 Queued | — |
| [ ] | 487 | **HTTP Header Viewer** | `http-header-viewer` | — | 💤 Queued | — |
| [ ] | 488 | **URL Redirect Checker** | `url-redirect-checker` | — | 💤 Queued | — |
| [ ] | 489 | **User Agent Parser** | `user-agent-parser` | `developer-engine` | ⏳ Untested | — |
| [ ] | 490 | **Port Reference Lookup** | `port-reference-lookup` | — | 💤 Queued | — |
| [ ] | 491 | **Ping Tester** | `ping-tester` | — | 💤 Queued | — |
| [ ] | 492 | **Traceroute** | `traceroute` | — | 💤 Queued | — |
| [ ] | 493 | **Download Speed Tester** | `download-speed-tester` | — | 💤 Queued | — |
| [ ] | 494 | **Upload Speed Tester** | `upload-speed-tester` | — | 💤 Queued | — |
| [ ] | 495 | **Latency Tester** | `latency-tester` | — | 💤 Queued | — |
| [ ] | 496 | **WebSocket Tester** | `websocket-tester` | — | 💤 Queued | — |
| [ ] | 497 | **API Request Builder** | `api-request-builder` | — | 💤 Queued | — |
| [ ] | 498 | **REST API Tester** | `rest-api-tester` | — | 💤 Queued | — |
| [ ] | 499 | **Webhook Tester** | `webhook-tester` | — | 💤 Queued | — |
| [ ] | 500 | **IP Subnet Range Calculator** | `ip-subnet-range-calculator` | — | 💤 Queued | — |
| [ ] | 100111 | **Sha3 Generator** | `sha3-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 100112 | **Blake3 Generator** | `blake3-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 100113 | **Hmac Generator** | `hmac-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 100114 | **Hmac Verifier** | `hmac-verifier` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 100115 | **Bcrypt Hash Generator** | `bcrypt-hash-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 100116 | **Argon2 Hash Generator** | `argon2-hash-generator` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 100119 | **RSA Sign Verify** | `rsa-sign-verify` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 100120 | **Pem Parser** | `pem-parser` | `crypto-engine` | ⏳ Untested | — |

### Social Media (20 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 611 | **YouTube Thumbnail Downloader** | `youtube-thumbnail-downloader` | — | 💤 Queued | — |
| [ ] | 612 | **YouTube Thumbnail Maker** | `youtube-thumbnail-maker` | `image-engine` | ⏳ Untested | — |
| [ ] | 613 | **YouTube Title Analyzer** | `youtube-title-analyzer` | — | 💤 Queued | — |
| [ ] | 614 | **YouTube Description Generator** | `youtube-description-generator` | — | 💤 Queued | — |
| [ ] | 615 | **YouTube Hashtag Generator** | `youtube-hashtag-generator` | — | 💤 Queued | — |
| [ ] | 616 | **YouTube Timestamp Generator** | `youtube-timestamp-generator` | — | 💤 Queued | — |
| [ ] | 617 | **YouTube Tag Formatter** | `youtube-tag-formatter` | — | 💤 Queued | — |
| [ ] | 618 | **Video Caption Formatter** | `video-caption-formatter` | `video-engine` | ⏳ Untested | — |
| [ ] | 619 | **Instagram Bio Generator** | `instagram-bio-generator` | — | 💤 Queued | — |
| [ ] | 620 | **Instagram Post Resizer** | `instagram-post-resizer` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 621 | **Instagram Hashtag Generator** | `instagram-hashtag-generator` | — | 💤 Queued | — |
| [ ] | 622 | **LinkedIn Headline Generator** | `linkedin-headline-generator` | — | 💤 Queued | — |
| [ ] | 623 | **LinkedIn Post Formatter** | `linkedin-post-formatter` | — | 💤 Queued | — |
| [ ] | 624 | **X/Twitter Character Counter** | `x-twitter-character-counter` | `text-engine` | ⏳ Untested | — |
| [ ] | 625 | **Social Media Content Calendar** | `social-media-content-calendar` | — | 💤 Queued | — |
| [ ] | 626 | **Social Media Image Resizer** | `social-media-image-resizer` | `colordesign-engine` | ⏳ Untested | — |
| [ ] | 627 | **Profile Picture Cropper** | `profile-picture-cropper` | `image-engine` | ⏳ Untested | — |
| [ ] | 628 | **Meme Maker** | `meme-maker` | `image-engine` | ⏳ Untested | — |
| [ ] | 629 | **Emoji Picker** | `emoji-picker` | — | 💤 Queued | — |
| [ ] | 630 | **Social Media Preview Generator** | `social-media-preview-generator` | `image-engine` | ⏳ Untested | — |

### Text Tools (59 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 201 | **Word Counter** | `word-counter` | `text-engine` | ⏳ Untested | — |
| [ ] | 202 | **Character Counter** | `character-counter` | `text-engine` | ⏳ Untested | — |
| [ ] | 203 | **Sentence Counter** | `sentence-counter` | `text-engine` | ⏳ Untested | — |
| [ ] | 204 | **Paragraph Counter** | `paragraph-counter` | `text-engine` | ⏳ Untested | — |
| [ ] | 205 | **Reading Time Calculator** | `reading-time-calculator` | `text-engine` | ⏳ Untested | — |
| [ ] | 206 | **Speaking Time Calculator** | `speaking-time-calculator` | `text-engine` | ⏳ Untested | — |
| [ ] | 207 | **Text Case Converter** | `text-case-converter` | `text-engine` | ⏳ Untested | — |
| [ ] | 208 | **Uppercase Converter** | `uppercase-converter` | `text-engine` | ⏳ Untested | — |
| [ ] | 209 | **Lowercase Converter** | `lowercase-converter` | `text-engine` | ⏳ Untested | — |
| [ ] | 210 | **Title Case Converter** | `title-case-converter` | `text-engine` | ⏳ Untested | — |
| [ ] | 211 | **Sentence Case Converter** | `sentence-case-converter` | `text-engine` | ⏳ Untested | — |
| [ ] | 212 | **Remove Extra Spaces** | `remove-extra-spaces` | `text-engine` | ⏳ Untested | — |
| [ ] | 213 | **Remove Duplicate Lines** | `remove-duplicate-lines` | `text-engine` | ⏳ Untested | — |
| [ ] | 214 | **Sort Lines Alphabetically** | `sort-lines-alphabetically` | `text-engine` | ⏳ Untested | — |
| [ ] | 215 | **Reverse Text** | `reverse-text` | `text-engine` | ⏳ Untested | — |
| [ ] | 216 | **Reverse Words** | `reverse-words` | `text-engine` | ⏳ Untested | — |
| [ ] | 217 | **Text to Binary** | `text-to-binary` | `text-engine` | ⏳ Untested | — |
| [ ] | 218 | **Binary to Text** | `binary-to-text` | `text-engine` | ⏳ Untested | — |
| [ ] | 219 | **Text to Morse Code** | `text-to-morse-code` | `text-engine` | ⏳ Untested | — |
| [ ] | 220 | **Morse Code to Text** | `morse-code-to-text` | `text-engine` | ⏳ Untested | — |
| [ ] | 221 | **Text to ASCII** | `text-to-ascii` | `text-engine` | ⏳ Untested | — |
| [ ] | 222 | **ASCII to Text** | `ascii-to-text` | `text-engine` | ⏳ Untested | — |
| [ ] | 223 | **Text to Unicode** | `text-to-unicode` | `text-engine` | ⏳ Untested | — |
| [ ] | 224 | **Unicode Inspector** | `unicode-inspector` | `text-engine` | ⏳ Untested | — |
| [ ] | 225 | **Find and Replace** | `find-and-replace` | `text-engine` | ⏳ Untested | — |
| [ ] | 226 | **Text Diff Checker** | `text-diff-checker` | `text-engine` | ⏳ Untested | — |
| [ ] | 227 | **Text Comparer** | `text-comparer` | `text-engine` | ⏳ Untested | — |
| [ ] | 228 | **Lorem Ipsum Generator** | `lorem-ipsum-generator` | `text-engine` | ⏳ Untested | — |
| [ ] | 229 | **Random Text Generator** | `random-text-generator` | `text-engine` | ⏳ Untested | — |
| [ ] | 230 | **Slug Generator** | `slug-generator` | `text-engine` | ⏳ Untested | — |
| [ ] | 231 | **Hashtag Generator** | `hashtag-generator` | — | 💤 Queued | — |
| [ ] | 233 | **Grammar Checker** | `grammar-checker` | — | 💤 Queued | — |
| [ ] | 234 | **Readability Checker** | `readability-checker` | `text-engine` | ⏳ Untested | — |
| [ ] | 235 | **Plagiarism Checker** | `plagiarism-checker` | — | 💤 Queued | — |
| [ ] | 236 | **Markdown Editor** | `markdown-editor` | — | 💤 Queued | — |
| [ ] | 237 | **Markdown Previewer** | `markdown-previewer` | `text-engine` | ⏳ Untested | — |
| [ ] | 238 | **Markdown to HTML** | `markdown-to-html` | `text-engine` | ⏳ Untested | — |
| [ ] | 239 | **HTML to Markdown** | `html-to-markdown` | `text-engine` | ⏳ Untested | — |
| [ ] | 240 | **Markdown to DOCX** | `markdown-to-docx` | `document-engine` | ⏳ Untested | — |
| [ ] | 241 | **Text to PDF** | `text-to-pdf` | `pdf-engine` | ⏳ Untested | — |
| [ ] | 242 | **Text to QR Code** | `text-to-qr-code` | `qrbarcode-engine` | ⏳ Untested | — |
| [ ] | 243 | **Text Encryptor** | `text-encryptor` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 244 | **Text Decryptor** | `text-decryptor` | `crypto-engine` | ⏳ Untested | — |
| [ ] | 245 | **Clipboard Formatter** | `clipboard-formatter` | — | 💤 Queued | — |
| [ ] | 100042 | **Line Counter** | `line-counter` | `text-engine` | ⏳ Untested | — |
| [ ] | 100043 | **Remove Empty Lines** | `remove-empty-lines` | `text-engine` | ⏳ Untested | — |
| [ ] | 100044 | **Text To Speech Prep** | `text-to-speech-prep` | `text-engine` | ⏳ Untested | — |
| [ ] | 100045 | **Text Formatter** | `text-formatter` | `text-engine` | ⏳ Untested | — |
| [ ] | 100046 | **Text Cleaner** | `text-cleaner` | `text-engine` | ⏳ Untested | — |
| [ ] | 100047 | **Unicode Normalizer** | `unicode-normalizer` | `text-engine` | ⏳ Untested | — |
| [ ] | 100048 | **Ascii Converter** | `ascii-converter` | `text-engine` | ⏳ Untested | — |
| [ ] | 100049 | **Rot13** | `rot13` | `text-engine` | ⏳ Untested | — |
| [ ] | 100050 | **Text To Hex** | `text-to-hex` | `text-engine` | ⏳ Untested | — |
| [ ] | 100051 | **Hex To Text** | `hex-to-text` | `text-engine` | ⏳ Untested | — |
| [ ] | 100052 | **Text Joiner** | `text-joiner` | `text-engine` | ⏳ Untested | — |
| [ ] | 100053 | **Text Splitter** | `text-splitter` | `text-engine` | ⏳ Untested | — |
| [ ] | 100054 | **Text Truncator** | `text-truncator` | `text-engine` | ⏳ Untested | — |
| [ ] | 100055 | **Text Extractor** | `text-extractor` | `text-engine` | ⏳ Untested | — |
| [ ] | 100056 | **Markdown Preview Parser** | `markdown-preview-parser` | `text-engine` | ⏳ Untested | — |

### Video Tools (56 tools)

| Check | ID | Tool Name | Slug | Engine | Status | Notes |
| :---: | :---: | :--- | :--- | :--- | :---: | :--- |
| [ ] | 111 | **Video Editor** | `video-editor` | — | 💤 Queued | — |
| [ ] | 112 | **Video Cutter** | `video-cutter` | `video-engine` | ⏳ Untested | — |
| [ ] | 113 | **Video Trimmer** | `video-trimmer` | `video-engine` | ⏳ Untested | — |
| [ ] | 114 | **Video Merger** | `video-merger` | `video-engine` | ⏳ Untested | — |
| [ ] | 115 | **Video Splitter** | `video-splitter` | `video-engine` | ⏳ Untested | — |
| [ ] | 116 | **Video Compressor** | `video-compressor` | `video-engine` | 💤 Queued | — |
| [ ] | 117 | **Video Converter** | `video-converter` | `video-engine` | 💤 Queued | — |
| [ ] | 118 | **MP4 to WebM** | `mp4-to-webm` | `video-engine` | 💤 Queued | — |
| [ ] | 119 | **WebM to MP4** | `webm-to-mp4` | `video-engine` | 💤 Queued | — |
| [ ] | 120 | **MOV to MP4** | `mov-to-mp4` | `video-engine` | 💤 Queued | — |
| [ ] | 121 | **AVI to MP4** | `avi-to-mp4` | `video-engine` | 💤 Queued | — |
| [ ] | 122 | **MKV to MP4** | `mkv-to-mp4` | `video-engine` | 💤 Queued | — |
| [ ] | 123 | **MP4 to GIF** | `mp4-to-gif` | `video-engine` | ⏳ Untested | — |
| [ ] | 124 | **Video to Images** | `video-to-images` | `video-engine` | ⏳ Untested | — |
| [ ] | 125 | **Video to Audio** | `video-to-audio` | `audio-engine` | 💤 Queued | — |
| [ ] | 126 | **Video Thumbnail Extractor** | `video-thumbnail-extractor` | `video-engine` | ⏳ Untested | — |
| [ ] | 127 | **Video Frame Extractor** | `video-frame-extractor` | `video-engine` | ⏳ Untested | — |
| [ ] | 128 | **Video Speed Changer** | `video-speed-changer` | `video-engine` | 💤 Queued | — |
| [ ] | 129 | **Slow Motion Creator** | `slow-motion-creator` | `video-engine` | 💤 Queued | — |
| [ ] | 130 | **Time-lapse Creator** | `time-lapse-creator` | `video-engine` | 💤 Queued | — |
| [ ] | 131 | **Reverse Video** | `reverse-video` | `video-engine` | 💤 Queued | — |
| [ ] | 132 | **Video Rotation** | `video-rotation` | `video-engine` | 💤 Queued | — |
| [ ] | 133 | **Video Cropper** | `video-cropper` | `video-engine` | 💤 Queued | — |
| [ ] | 134 | **Video Resizer** | `video-resizer` | `video-engine` | 💤 Queued | — |
| [ ] | 135 | **Video Aspect Ratio Converter** | `video-aspect-ratio-converter` | `video-engine` | ⏳ Untested | — |
| [ ] | 136 | **Video Watermark** | `video-watermark` | `video-engine` | 💤 Queued | — |
| [ ] | 137 | **Video Subtitle Editor** | `video-subtitle-editor` | — | 💤 Queued | — |
| [ ] | 138 | **Subtitle Extractor** | `subtitle-extractor` | `video-engine` | ⏳ Untested | — |
| [ ] | 139 | **Subtitle Merger** | `subtitle-merger` | — | 💤 Queued | — |
| [ ] | 140 | **SRT to VTT** | `srt-to-vtt` | `video-engine` | ⏳ Untested | — |
| [ ] | 141 | **VTT to SRT** | `vtt-to-srt` | `video-engine` | ⏳ Untested | — |
| [ ] | 142 | **Video Mute Tool** | `video-mute-tool` | `video-engine` | ⏳ Untested | — |
| [ ] | 143 | **Video Audio Replacement** | `video-audio-replacement` | — | 💤 Queued | — |
| [ ] | 144 | **Video Fade In/Out** | `video-fade-in-out` | — | 💤 Queued | — |
| [ ] | 145 | **Video Transition Editor** | `video-transition-editor` | — | 💤 Queued | — |
| [ ] | 146 | **Video Overlay Editor** | `video-overlay-editor` | — | 💤 Queued | — |
| [ ] | 147 | **Video Text Overlay** | `video-text-overlay` | — | 💤 Queued | — |
| [ ] | 148 | **Video Collage Maker** | `video-collage-maker` | — | 💤 Queued | — |
| [ ] | 149 | **Video Comparison Tool** | `video-comparison-tool` | — | 💤 Queued | — |
| [ ] | 150 | **Video Metadata Viewer** | `video-metadata-viewer` | `video-engine` | ⏳ Untested | — |
| [ ] | 151 | **Video Color Correction** | `video-color-correction` | — | 💤 Queued | — |
| [ ] | 152 | **Video Brightness Editor** | `video-brightness-editor` | — | 💤 Queued | — |
| [ ] | 153 | **Video Stabilization** | `video-stabilization` | — | 💤 Queued | — |
| [ ] | 154 | **Video Thumbnail Maker** | `video-thumbnail-maker` | `video-engine` | ⏳ Untested | — |
| [ ] | 155 | **Screen Recorder** | `screen-recorder` | — | 💤 Queued | — |
| [ ] | 156 | **Webcam Recorder** | `webcam-recorder` | — | 💤 Queued | — |
| [ ] | 157 | **Browser Tab Recorder** | `browser-tab-recorder` | — | 💤 Queued | — |
| [ ] | 158 | **GIF Maker** | `gif-maker` | `video-engine` | ⏳ Untested | — |
| [ ] | 159 | **GIF Compressor** | `gif-compressor` | `image-engine` | ⏳ Untested | — |
| [ ] | 160 | **GIF Resizer** | `gif-resizer` | `image-engine` | ⏳ Untested | — |
| [ ] | 100022 | **Video Concat** | `video-concat` | `video-engine` | ⏳ Untested | — |
| [ ] | 100023 | **Video Extract Audio** | `video-extract-audio` | `video-engine` | ⏳ Untested | — |
| [ ] | 100024 | **Video Resolution Check** | `video-resolution-check` | `video-engine` | ⏳ Untested | — |
| [ ] | 100025 | **Video Frame Rate Check** | `video-frame-rate-check` | `video-engine` | ⏳ Untested | — |
| [ ] | 100026 | **Video Remove Audio** | `video-remove-audio` | `video-engine` | ⏳ Untested | — |
| [ ] | 100027 | **Video Loop** | `video-loop` | `video-engine` | ⏳ Untested | — |
