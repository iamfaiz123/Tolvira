//! Rule data: GST state codes, slab presets, income tax presets, TDS
//! sections, HSN chapters. Every preset carries a `source` / `effective`
//! label and can be overridden by the caller.

use serde_json::{json, Value};

/// Version of the bundled rule tables (financial year + revision).
pub const RULES_VERSION: &str = "2024-25.1";

/// GST state codes (first two digits of a GSTIN).
pub const STATE_CODES: &[(&str, &str)] = &[
    ("01", "Jammu and Kashmir"),
    ("02", "Himachal Pradesh"),
    ("03", "Punjab"),
    ("04", "Chandigarh"),
    ("05", "Uttarakhand"),
    ("06", "Haryana"),
    ("07", "Delhi"),
    ("08", "Rajasthan"),
    ("09", "Uttar Pradesh"),
    ("10", "Bihar"),
    ("11", "Sikkim"),
    ("12", "Arunachal Pradesh"),
    ("13", "Nagaland"),
    ("14", "Manipur"),
    ("15", "Mizoram"),
    ("16", "Tripura"),
    ("17", "Meghalaya"),
    ("18", "Assam"),
    ("19", "West Bengal"),
    ("20", "Jharkhand"),
    ("21", "Odisha"),
    ("22", "Chhattisgarh"),
    ("23", "Madhya Pradesh"),
    ("24", "Gujarat"),
    ("26", "Dadra and Nagar Haveli and Daman and Diu"),
    ("27", "Maharashtra"),
    ("29", "Karnataka"),
    ("30", "Goa"),
    ("31", "Lakshadweep"),
    ("32", "Kerala"),
    ("33", "Tamil Nadu"),
    ("34", "Puducherry"),
    ("35", "Andaman and Nicobar Islands"),
    ("36", "Telangana"),
    ("37", "Andhra Pradesh"),
    ("38", "Ladakh"),
    ("97", "Other Territory"),
    ("99", "Centre Jurisdiction"),
];

/// Standard GST slabs (percent). Stable since 2017; composite cess not included.
pub const GST_SLABS: &[f64] = &[0.0, 0.25, 3.0, 5.0, 12.0, 18.0, 28.0];

/// Income tax presets. Slabs are (from, to, rate%). `rebate_limit` is the
/// taxable income up to which section 87A makes tax nil.
pub fn income_tax_presets() -> Value {
    json!({
        "FY2024-25-new": {
            "label": "FY 2024-25 (AY 2025-26) – new regime (default)",
            "source": "Finance (No. 2) Act 2024 – verify against the Income-tax Department before \
                       filing",
            "slabs": [
                [0, 300000, 0],
                [300000, 700000, 5],
                [700000, 1000000, 10],
                [1000000, 1200000, 15],
                [1200000, 1500000, 20],
                [1500000, null, 30]
            ],
            "standard_deduction_salary": 75000,
            "rebate_limit": 700000,
            "rebate_max": 25000,
            "cess_percent": 4,
            "surcharge": [[5000000, 10], [10000000, 15], [20000000, 25]],
            "allows_chapter_via": false
        },
        "FY2024-25-old": {
            "label": "FY 2024-25 (AY 2025-26) – old regime",
            "source": "Income-tax Act 1961 slabs (unchanged) – verify before filing",
            "slabs": [
                [0, 250000, 0],
                [250000, 500000, 5],
                [500000, 1000000, 20],
                [1000000, null, 30]
            ],
            "standard_deduction_salary": 50000,
            "rebate_limit": 500000,
            "rebate_max": 12500,
            "cess_percent": 4,
            "surcharge": [[5000000, 10], [10000000, 15], [20000000, 25], [50000000, 37]],
            "allows_chapter_via": true
        },
        "FY2023-24-new": {
            "label": "FY 2023-24 (AY 2024-25) – new regime",
            "source": "Finance Act 2023",
            "slabs": [
                [0, 300000, 0],
                [300000, 600000, 5],
                [600000, 900000, 10],
                [900000, 1200000, 15],
                [1200000, 1500000, 20],
                [1500000, null, 30]
            ],
            "standard_deduction_salary": 50000,
            "rebate_limit": 700000,
            "rebate_max": 25000,
            "cess_percent": 4,
            "surcharge": [[5000000, 10], [10000000, 15], [20000000, 25]],
            "allows_chapter_via": false
        }
    })
}

/// Common TDS sections (rate % for resident payees, threshold per year).
pub fn tds_sections() -> Value {
    json!({
        "version": RULES_VERSION,
        "source": "Income-tax Act 1961, rates applicable FY 2024-25 for residents (with PAN); \
                   verify for your case",
        "sections": {
            "192": {
                "name": "Salary",
                "rate": null,
                "threshold": 0,
                "note": "Per slab rates of the employee"
            },
            "194A": {
                "name": "Interest other than securities (banks)",
                "rate": 10,
                "threshold": 40000,
                "threshold_senior": 50000
            },
            "194C": {
                "name": "Contractor payments (individual/HUF)",
                "rate": 1,
                "threshold": 30000,
                "aggregate_threshold": 100000
            },
            "194C-other": {
                "name": "Contractor payments (others)",
                "rate": 2,
                "threshold": 30000,
                "aggregate_threshold": 100000
            },
            "194H": {
                "name": "Commission or brokerage",
                "rate": 2,
                "threshold": 15000,
                "note": "5% before 1 Oct 2024"
            },
            "194I-land": { "name": "Rent of land/building", "rate": 10, "threshold": 240000 },
            "194I-machinery": {
                "name": "Rent of plant & machinery",
                "rate": 2,
                "threshold": 240000
            },
            "194J-professional": { "name": "Professional fees", "rate": 10, "threshold": 30000 },
            "194J-technical": {
                "name": "Technical services / royalty for films",
                "rate": 2,
                "threshold": 30000
            },
            "194Q": { "name": "Purchase of goods > 50 lakh", "rate": 0.1, "threshold": 5000000 },
            "194IA": { "name": "Transfer of immovable property", "rate": 1, "threshold": 5000000 },
            "194IB": {
                "name": "Rent by individual/HUF > 50k per month",
                "rate": 2,
                "threshold": 50000,
                "note": "5% before 1 Oct 2024"
            },
            "194D": { "name": "Insurance commission", "rate": 2, "threshold": 15000 },
            "194DA": { "name": "Life insurance policy payout", "rate": 2, "threshold": 100000 },
            "194B": { "name": "Lottery / game winnings", "rate": 30, "threshold": 10000 },
            "194K": { "name": "Income from mutual fund units", "rate": 10, "threshold": 5000 },
            "194O": { "name": "E-commerce participants", "rate": 0.1, "threshold": 500000 },
            "194S": { "name": "Virtual digital assets", "rate": 1, "threshold": 10000 },
            "206AA": {
                "name": "No PAN furnished",
                "rate": 20,
                "threshold": 0,
                "note": "Higher of 20% or the section rate"
            }
        }
    })
}

/// HSN chapter headings (2-digit) – partial dataset for guidance.
pub const HSN_CHAPTERS: &[(&str, &str)] = &[
    ("01", "Live animals"),
    ("02", "Meat and edible meat offal"),
    ("03", "Fish and crustaceans"),
    ("04", "Dairy produce; eggs; honey"),
    ("07", "Edible vegetables"),
    ("08", "Edible fruit and nuts"),
    ("09", "Coffee, tea, mate and spices"),
    ("10", "Cereals"),
    ("11", "Milling products; malt; starches"),
    ("12", "Oil seeds and oleaginous fruits"),
    ("15", "Animal or vegetable fats and oils"),
    ("17", "Sugars and sugar confectionery"),
    ("18", "Cocoa and cocoa preparations"),
    ("19", "Preparations of cereals, flour, starch or milk; bakers' wares"),
    ("20", "Preparations of vegetables, fruit, nuts"),
    ("21", "Miscellaneous edible preparations"),
    ("22", "Beverages, spirits and vinegar"),
    ("24", "Tobacco"),
    ("25", "Salt; sulphur; earths and stone; cement"),
    ("27", "Mineral fuels, oils"),
    ("28", "Inorganic chemicals"),
    ("29", "Organic chemicals"),
    ("30", "Pharmaceutical products"),
    ("31", "Fertilisers"),
    ("32", "Tanning or dyeing extracts; paints"),
    ("33", "Essential oils; perfumery, cosmetics"),
    ("34", "Soap, washing preparations, candles"),
    ("38", "Miscellaneous chemical products"),
    ("39", "Plastics and articles thereof"),
    ("40", "Rubber and articles thereof"),
    ("42", "Articles of leather; handbags"),
    ("44", "Wood and articles of wood"),
    ("48", "Paper and paperboard"),
    ("49", "Printed books, newspapers"),
    ("52", "Cotton"),
    ("54", "Man-made filaments"),
    ("61", "Knitted or crocheted apparel"),
    ("62", "Apparel, not knitted"),
    ("63", "Other made-up textile articles"),
    ("64", "Footwear"),
    ("68", "Articles of stone, plaster, cement"),
    ("69", "Ceramic products"),
    ("70", "Glass and glassware"),
    ("71", "Precious stones, metals, jewellery"),
    ("72", "Iron and steel"),
    ("73", "Articles of iron or steel"),
    ("74", "Copper and articles thereof"),
    ("76", "Aluminium and articles thereof"),
    ("82", "Tools, implements, cutlery"),
    ("83", "Miscellaneous articles of base metal"),
    ("84", "Machinery and mechanical appliances; computers"),
    ("85", "Electrical machinery; electronics; telephones"),
    ("87", "Vehicles other than railway"),
    ("90", "Optical, photographic, medical instruments"),
    ("91", "Clocks and watches"),
    ("94", "Furniture; bedding; lamps"),
    ("95", "Toys, games and sports requisites"),
    ("96", "Miscellaneous manufactured articles"),
];

/// SAC (services) headings – partial dataset.
pub const SAC_HEADINGS: &[(&str, &str, f64)] = &[
    ("9954", "Construction services", 18.0),
    ("9961", "Services in wholesale trade", 18.0),
    ("9962", "Services in retail trade", 18.0),
    ("9963", "Accommodation, food and beverage services", 5.0),
    ("9964", "Passenger transport services", 5.0),
    ("9965", "Goods transport services", 5.0),
    ("9966", "Rental services of transport vehicles", 18.0),
    ("9971", "Financial and related services", 18.0),
    ("9972", "Real estate services", 18.0),
    ("9973", "Leasing or rental services without operator", 18.0),
    ("9981", "Research and development services", 18.0),
    ("9982", "Legal and accounting services", 18.0),
    ("9983", "Other professional, technical and business services", 18.0),
    ("9984", "Telecommunications, broadcasting and information supply", 18.0),
    ("9985", "Support services", 18.0),
    ("9986", "Support services to agriculture, mining, utilities", 18.0),
    ("9987", "Maintenance, repair and installation services", 18.0),
    ("9988", "Manufacturing services on physical inputs (job work)", 12.0),
    ("9991", "Public administration services", 18.0),
    ("9992", "Education services", 18.0),
    ("9993", "Human health and social care services", 18.0),
    ("9994", "Sewage, waste collection and treatment", 18.0),
    ("9995", "Services of membership organisations", 18.0),
    ("9996", "Recreational, cultural and sporting services", 18.0),
    ("9997", "Other services (beauty, laundry, etc.)", 18.0),
    ("998313", "Information technology (IT) consulting and support services", 18.0),
    ("998314", "IT / software design and development services", 18.0),
    ("998315", "Hosting and IT infrastructure provisioning services", 18.0),
    ("998316", "IT infrastructure and network management services", 18.0),
    ("997331", "Licensing services for software (SaaS)", 18.0),
    ("997212", "Rental or leasing of commercial property", 18.0),
    ("996331", "Restaurant services", 5.0),
    ("998599", "Other support services", 18.0),
];

/// Professional tax presets (monthly salary slabs → monthly tax).
pub fn professional_tax_presets() -> Value {
    json!({
        "source": "State professional tax schedules – verify with the state commercial tax \
                   department",
        "states": {
            "maharashtra": {
                "slabs": [[0, 7500, 0], [7500, 10000, 175], [10000, null, 200]],
                "february_amount": 300,
                "note": "Women earning up to ₹25,000/month are exempt"
            },
            "karnataka": { "slabs": [[0, 25000, 0], [25000, null, 200]], "february_amount": 200 },
            "west-bengal": {
                "slabs": [
                    [0, 10000, 0],
                    [10000, 15000, 110],
                    [15000, 25000, 130],
                    [25000, 40000, 150],
                    [40000, null, 200]
                ],
                "february_amount": 200
            },
            "tamil-nadu": {
                "slabs": [
                    [0, 21000, 0],
                    [21000, 30000, 135],
                    [30000, 45000, 315],
                    [45000, 60000, 690],
                    [60000, 75000, 1025],
                    [75000, null, 1250]
                ],
                "note": "Half-yearly amounts shown as monthly equivalent of the half-yearly slab \
                         / 6",
                "february_amount": 1250
            },
            "gujarat": { "slabs": [[0, 12000, 0], [12000, null, 200]], "february_amount": 200 },
            "telangana": {
                "slabs": [[0, 15000, 0], [15000, 20000, 150], [20000, null, 200]],
                "february_amount": 200
            },
            "andhra-pradesh": {
                "slabs": [[0, 15000, 0], [15000, 20000, 150], [20000, null, 200]],
                "february_amount": 200
            },
            "madhya-pradesh": {
                "slabs": [
                    [0, 18750, 0],
                    [18750, 25000, 125],
                    [25000, 33333, 167],
                    [33333, null, 208]
                ],
                "february_amount": 212
            },
            "delhi": { "slabs": [[0, null, 0]], "note": "No professional tax" },
            "uttar-pradesh": { "slabs": [[0, null, 0]], "note": "No professional tax" },
            "haryana": { "slabs": [[0, null, 0]], "note": "No professional tax" },
            "rajasthan": { "slabs": [[0, null, 0]], "note": "No professional tax" }
        }
    })
}

/// Capital gains defaults (post Budget 2024, transfers on/after 23 July 2024).
pub fn capital_gains_presets() -> Value {
    json!({
        "source": "Finance (No. 2) Act 2024 – verify; rates differ for transfers before 23 July \
                   2024",
        "assets": {
            "equity": {
                "label": "Listed equity shares / equity mutual funds (STT paid)",
                "long_term_months": 12,
                "ltcg_rate": 12.5,
                "ltcg_exemption": 125000,
                "stcg_rate": 20,
                "indexation": false
            },
            "debt-fund": {
                "label": "Debt mutual funds bought after 1 Apr 2023",
                "long_term_months": 24,
                "ltcg_rate": null,
                "stcg_rate": null,
                "note": "Taxed at slab rates regardless of holding period",
                "indexation": false
            },
            "property": {
                "label": "Immovable property",
                "long_term_months": 24,
                "ltcg_rate": 12.5,
                "ltcg_exemption": 0,
                "stcg_rate": null,
                "indexation": false,
                "note": "Slab rates for short-term; 20% with indexation optional for property \
                         acquired before 23 July 2024"
            },
            "gold": {
                "label": "Gold / physical assets",
                "long_term_months": 24,
                "ltcg_rate": 12.5,
                "ltcg_exemption": 0,
                "stcg_rate": null,
                "indexation": false
            },
            "unlisted": {
                "label": "Unlisted shares",
                "long_term_months": 24,
                "ltcg_rate": 12.5,
                "ltcg_exemption": 0,
                "stcg_rate": null,
                "indexation": false
            },
            "crypto": {
                "label": "Virtual digital assets",
                "long_term_months": 0,
                "ltcg_rate": 30,
                "stcg_rate": 30,
                "ltcg_exemption": 0,
                "note": "Flat 30% (section 115BBH), no loss set-off"
            }
        }
    })
}
