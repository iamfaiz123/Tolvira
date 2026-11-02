//! Conversion factor tables. Every linear category lists units as
//! `(canonical id, display name, aliases, factor to the base unit)`.
//! Factors follow NIST SP 811 / ISO 80000 exact definitions where they exist.

/// Version of the factor tables (bump when definitions change).
pub const TABLE_VERSION: &str = "2024.1";

/// A unit within a category.
pub struct Unit {
    /// Canonical identifier.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Accepted aliases (case-insensitive).
    pub aliases: &'static [&'static str],
    /// Multiply a value in this unit by `factor` to get the base unit.
    pub factor: f64,
}

/// A group of mutually convertible units.
pub struct Category {
    /// Category identifier.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Id of the base unit the factors refer to.
    pub base: &'static str,
    /// Units in the category.
    pub units: &'static [Unit],
}

macro_rules! u {
    ($id:expr, $name:expr, [$($a:expr),*], $f:expr) => {
        Unit { id: $id, name: $name, aliases: &[$($a),*], factor: $f }
    };
}

/// Length units (base: metre).
pub static LENGTH: Category = Category {
    id: "length",
    name: "Length",
    base: "m",
    units: &[
        u!("m", "metre", ["meter", "meters", "metres"], 1.0),
        u!("km", "kilometre", ["kilometer", "kilometers", "kilometres"], 1000.0),
        u!("cm", "centimetre", ["centimeter", "centimeters"], 0.01),
        u!("mm", "millimetre", ["millimeter", "millimeters"], 0.001),
        u!("um", "micrometre", ["micrometer", "micron", "µm", "μm"], 1e-6),
        u!("nm", "nanometre", ["nanometer"], 1e-9),
        u!("in", "inch", ["inch", "inches", "\""], 0.0254),
        u!("ft", "foot", ["foot", "feet", "'"], 0.3048),
        u!("yd", "yard", ["yard", "yards"], 0.9144),
        u!("mi", "mile", ["mile", "miles"], 1609.344),
        u!("nmi", "nautical mile", ["nautical", "nauticalmile"], 1852.0),
        u!("ly", "light year", ["lightyear", "light-year"], 9.4607304725808e15),
        u!("au", "astronomical unit", ["astronomical"], 1.495978707e11),
        u!("pc", "parsec", ["parsec"], 3.085_677_581_491_367e16),
        u!("furlong", "furlong", [], 201.168),
        u!("fathom", "fathom", [], 1.8288),
        u!("px", "pixel (96 dpi)", ["pixel", "pixels"], 0.0254 / 96.0),
        u!("pt", "point (typographic)", ["point", "points"], 0.0254 / 72.0),
    ],
};

/// Mass units (base: kilogram).
pub static MASS: Category = Category {
    id: "mass",
    name: "Weight / Mass",
    base: "kg",
    units: &[
        u!("kg", "kilogram", ["kilogram", "kilograms", "kilo", "kilos"], 1.0),
        u!("g", "gram", ["gram", "grams", "gm"], 0.001),
        u!("mg", "milligram", ["milligram", "milligrams"], 1e-6),
        u!("ug", "microgram", ["microgram", "mcg", "µg", "μg"], 1e-9),
        u!("t", "tonne (metric)", ["tonne", "tonnes", "metricton", "metric ton"], 1000.0),
        u!("lb", "pound", ["pound", "pounds", "lbs"], 0.45359237),
        u!("oz", "ounce", ["ounce", "ounces"], 0.028349523125),
        u!("st", "stone", ["stone", "stones"], 6.35029318),
        u!("ton_us", "short ton (US)", ["shortton", "us ton", "ton"], 907.18474),
        u!("ton_uk", "long ton (UK)", ["longton", "uk ton"], 1016.0469088),
        u!("ct", "carat", ["carat", "carats"], 0.0002),
        u!("gr", "grain", ["grain", "grains"], 6.479891e-5),
    ],
};

/// Area units (base: square metre).
pub static AREA: Category = Category {
    id: "area",
    name: "Area",
    base: "m2",
    units: &[
        u!(
            "m2",
            "square metre",
            ["sqm", "m^2", "m²", "square meter", "square meters", "square metres"],
            1.0
        ),
        u!("km2", "square kilometre", ["sqkm", "km^2", "km²", "square kilometer"], 1e6),
        u!("cm2", "square centimetre", ["sqcm", "cm^2", "cm²"], 1e-4),
        u!("mm2", "square millimetre", ["sqmm", "mm^2", "mm²"], 1e-6),
        u!("ha", "hectare", ["hectare", "hectares"], 1e4),
        u!("are", "are", ["ares"], 100.0),
        u!(
            "in2",
            "square inch",
            ["sqin", "in^2", "in²", "square inch", "square inches"],
            0.00064516
        ),
        u!(
            "ft2",
            "square foot",
            ["sqft", "ft^2", "ft²", "square foot", "square feet"],
            0.09290304
        ),
        u!(
            "yd2",
            "square yard",
            ["sqyd", "yd^2", "yd²", "square yard", "square yards"],
            0.83612736
        ),
        u!(
            "mi2",
            "square mile",
            ["sqmi", "mi^2", "mi²", "square mile", "square miles"],
            2589988.110336
        ),
        u!("acre", "acre", ["acres", "ac"], 4046.8564224),
        u!("guntha", "guntha", ["gunta"], 101.1714),
        u!("bigha", "bigha (UP, 27225 sq ft)", [], 2529.285264),
        u!("cent", "cent (India)", ["cents"], 40.468564224),
        u!("ground", "ground (Chennai)", [], 222.967296),
        u!("marla", "marla (272.25 sq ft)", [], 25.29285264),
        u!("kanal", "kanal", [], 505.857052),
    ],
};

/// Volume units (base: litre).
pub static VOLUME: Category = Category {
    id: "volume",
    name: "Volume",
    base: "l",
    units: &[
        u!("l", "litre", ["liter", "liters", "litres", "ltr"], 1.0),
        u!("ml", "millilitre", ["milliliter", "milliliters", "cc"], 0.001),
        u!("m3", "cubic metre", ["cbm", "m^3", "m³", "cubic meter", "cubic meters"], 1000.0),
        u!("cm3", "cubic centimetre", ["cm^3", "cm³", "cubic centimeter"], 0.001),
        u!("mm3", "cubic millimetre", ["mm^3", "mm³"], 1e-6),
        u!("gal_us", "US gallon", ["gallon", "gallons", "gal", "us gallon"], 3.785411784),
        u!("gal_uk", "imperial gallon", ["imperial gallon", "uk gallon"], 4.54609),
        u!("qt_us", "US quart", ["quart", "quarts", "qt"], 0.946352946),
        u!("pt_us", "US pint", ["pint", "pints", "pt"], 0.473176473),
        u!("pt_uk", "imperial pint", ["uk pint"], 0.56826125),
        u!("cup_us", "US cup", ["cup", "cups"], 0.2365882365),
        u!("cup_metric", "metric cup", ["metric cup"], 0.25),
        u!(
            "floz_us",
            "US fluid ounce",
            ["floz", "fl oz", "fluid ounce", "fluid ounces"],
            0.0295735295625
        ),
        u!("floz_uk", "imperial fluid ounce", ["uk fl oz"], 0.0284130625),
        u!(
            "tbsp",
            "tablespoon (US)",
            ["tablespoon", "tablespoons", "tbs"],
            0.01478676478125
        ),
        u!("tsp", "teaspoon (US)", ["teaspoon", "teaspoons"], 0.00492892159375),
        u!("in3", "cubic inch", ["in^3", "in³", "cubic inch", "cubic inches"], 0.016387064),
        u!(
            "ft3",
            "cubic foot",
            ["ft^3", "ft³", "cubic foot", "cubic feet", "cft"],
            28.316846592
        ),
        u!("yd3", "cubic yard", ["yd^3", "yd³", "cubic yard", "cubic yards"], 764.554857984),
        u!("bbl", "oil barrel", ["barrel", "barrels"], 158.987294928),
    ],
};

/// Speed units (base: metre per second).
pub static SPEED: Category = Category {
    id: "speed",
    name: "Speed",
    base: "m/s",
    units: &[
        u!(
            "m/s",
            "metre per second",
            ["mps", "m/s", "meter per second", "meters per second"],
            1.0
        ),
        u!(
            "km/h",
            "kilometre per hour",
            ["kmh", "kph", "km/h", "kilometers per hour", "kmph"],
            1000.0 / 3600.0
        ),
        u!("mph", "mile per hour", ["mi/h", "miles per hour"], 1609.344 / 3600.0),
        u!("kn", "knot", ["knot", "knots", "kt", "kts"], 1852.0 / 3600.0),
        u!("ft/s", "foot per second", ["fps", "feet per second"], 0.3048),
        u!("mach", "Mach (sea level, 15 °C)", [], 340.29),
        u!("c", "speed of light", ["lightspeed"], 299_792_458.0),
        u!("min/km", "minutes per kilometre (pace)", ["pace km", "min per km"], f64::NAN),
        u!("min/mi", "minutes per mile (pace)", ["pace mile", "min per mile"], f64::NAN),
    ],
};

/// Time units (base: second).
pub static TIME: Category = Category {
    id: "time",
    name: "Time",
    base: "s",
    units: &[
        u!("s", "second", ["sec", "secs", "second", "seconds"], 1.0),
        u!("ms", "millisecond", ["millisecond", "milliseconds"], 1e-3),
        u!("us", "microsecond", ["microsecond", "microseconds", "µs", "μs"], 1e-6),
        u!("ns", "nanosecond", ["nanosecond", "nanoseconds"], 1e-9),
        u!("min", "minute", ["minute", "minutes", "mins"], 60.0),
        u!("h", "hour", ["hr", "hrs", "hour", "hours"], 3600.0),
        u!("d", "day", ["day", "days"], 86400.0),
        u!("wk", "week", ["week", "weeks", "w"], 604800.0),
        u!("mo", "month (30.436875 d)", ["month", "months"], 2629746.0),
        u!("yr", "year (365.2425 d)", ["year", "years", "y"], 31556952.0),
        u!("decade", "decade", ["decades"], 315569520.0),
        u!("century", "century", ["centuries"], 3155695200.0),
    ],
};

/// Pressure units (base: pascal).
pub static PRESSURE: Category = Category {
    id: "pressure",
    name: "Pressure",
    base: "pa",
    units: &[
        u!("pa", "pascal", ["pascal", "pascals"], 1.0),
        u!("kpa", "kilopascal", ["kilopascal"], 1000.0),
        u!("mpa", "megapascal", ["megapascal"], 1e6),
        u!("hpa", "hectopascal", ["hectopascal"], 100.0),
        u!("bar", "bar", ["bars"], 100000.0),
        u!("mbar", "millibar", ["millibar", "millibars"], 100.0),
        u!("atm", "standard atmosphere", ["atmosphere", "atmospheres"], 101325.0),
        u!("psi", "pound per square inch", ["lbf/in2", "psig"], 6894.757293168),
        u!("torr", "torr", ["mmhg", "mm hg"], 101325.0 / 760.0),
        u!("inhg", "inch of mercury", ["in hg"], 3386.389),
        u!("kgf/cm2", "kilogram-force per cm²", ["kg/cm2", "at"], 98066.5),
    ],
};

/// Energy units (base: joule).
pub static ENERGY: Category = Category {
    id: "energy",
    name: "Energy",
    base: "j",
    units: &[
        u!("j", "joule", ["joule", "joules"], 1.0),
        u!("kj", "kilojoule", ["kilojoule", "kilojoules"], 1000.0),
        u!("mj", "megajoule", ["megajoule"], 1e6),
        u!("cal", "calorie (thermochemical)", ["calorie", "calories"], 4.184),
        u!(
            "kcal",
            "kilocalorie (food Calorie)",
            ["kilocalorie", "kilocalories", "Calorie", "food calorie"],
            4184.0
        ),
        u!("wh", "watt hour", ["watt-hour", "watthour"], 3600.0),
        u!("kwh", "kilowatt hour", ["kilowatt hour", "kilowatt-hour", "unit"], 3.6e6),
        u!("mwh", "megawatt hour", ["megawatt hour"], 3.6e9),
        u!("ev", "electronvolt", ["electronvolt", "electron volt"], 1.602176634e-19),
        u!("btu", "British thermal unit", ["btus"], 1055.05585262),
        u!("therm", "therm (US)", ["therms"], 105480400.0),
        u!("ftlb", "foot-pound", ["ft-lb", "ft·lbf", "foot pound"], 1.3558179483314),
        u!("erg", "erg", ["ergs"], 1e-7),
        u!("toe", "tonne of oil equivalent", [], 41.868e9),
    ],
};

/// Power units (base: watt).
pub static POWER: Category = Category {
    id: "power",
    name: "Power",
    base: "w",
    units: &[
        u!("w", "watt", ["watt", "watts"], 1.0),
        u!("kw", "kilowatt", ["kilowatt", "kilowatts"], 1000.0),
        u!("mw", "megawatt", ["megawatt", "megawatts"], 1e6),
        u!("gw", "gigawatt", ["gigawatt"], 1e9),
        u!("mw_milli", "milliwatt", ["milliwatt", "milliwatts"], 1e-3),
        u!("hp", "horsepower (mechanical)", ["horsepower", "bhp"], 745.69987158227),
        u!("ps", "metric horsepower", ["metric horsepower", "cv"], 735.49875),
        u!("btu/h", "BTU per hour", ["btuh", "btu/hr"], 0.29307107),
        u!("ton_ref", "ton of refrigeration", ["tr", "ton refrigeration"], 3516.8528420667),
        u!("cal/s", "calorie per second", [], 4.184),
        u!("ftlb/s", "foot-pound per second", [], 1.3558179483314),
        u!("dbm", "dBm (logarithmic)", [], f64::NAN),
    ],
};

/// Data size units (base: byte).
pub static DATA: Category = Category {
    id: "data",
    name: "Data size",
    base: "b",
    units: &[
        u!("bit", "bit", ["bits"], 0.125),
        u!("b", "byte", ["byte", "bytes"], 1.0),
        u!("kb", "kilobyte (10³)", ["kilobyte", "kilobytes"], 1e3),
        u!("mb", "megabyte (10⁶)", ["megabyte", "megabytes"], 1e6),
        u!("gb", "gigabyte (10⁹)", ["gigabyte", "gigabytes"], 1e9),
        u!("tb", "terabyte (10¹²)", ["terabyte", "terabytes"], 1e12),
        u!("pb", "petabyte (10¹⁵)", ["petabyte"], 1e15),
        u!("kib", "kibibyte (2¹⁰)", ["kibibyte"], 1024.0),
        u!("mib", "mebibyte (2²⁰)", ["mebibyte"], 1048576.0),
        u!("gib", "gibibyte (2³⁰)", ["gibibyte"], 1073741824.0),
        u!("tib", "tebibyte (2⁴⁰)", ["tebibyte"], 1099511627776.0),
        u!("pib", "pebibyte (2⁵⁰)", ["pebibyte"], 1125899906842624.0),
        u!("kbit", "kilobit", ["kilobit", "kilobits"], 125.0),
        u!("mbit", "megabit", ["megabit", "megabits"], 125000.0),
        u!("gbit", "gigabit", ["gigabit", "gigabits"], 125000000.0),
        u!("nibble", "nibble", [], 0.5),
    ],
};

/// Data rate units (base: bit per second).
pub static DATA_RATE: Category = Category {
    id: "data_rate",
    name: "Data transfer rate",
    base: "bps",
    units: &[
        u!("bps", "bit per second", ["bit/s", "bits per second"], 1.0),
        u!("kbps", "kilobit per second", ["kbit/s"], 1e3),
        u!("mbps", "megabit per second", ["mbit/s"], 1e6),
        u!("gbps", "gigabit per second", ["gbit/s"], 1e9),
        u!("B/s", "byte per second", ["bytes/s", "bytes per second"], 8.0),
        u!("kB/s", "kilobyte per second", ["kbyte/s", "kbps_bytes", "kb/s"], 8e3),
        u!("MB/s", "megabyte per second", ["mbyte/s", "mb/s", "megabytes per second"], 8e6),
        u!("GB/s", "gigabyte per second", ["gbyte/s", "gb/s"], 8e9),
        u!("MiB/s", "mebibyte per second", ["MiB/s"], 8.0 * 1048576.0),
    ],
};

/// Fuel economy units (base: litres per 100 km, handled as reciprocal where needed).
pub static FUEL: Category = Category {
    id: "fuel_economy",
    name: "Fuel economy",
    base: "km/l",
    units: &[
        u!("km/l", "kilometres per litre", ["kmpl", "km per litre", "km/liter"], 1.0),
        u!(
            "mpg_us",
            "miles per US gallon",
            ["mpg", "mpg us", "miles per gallon"],
            1.609344 / 3.785411784
        ),
        u!(
            "mpg_uk",
            "miles per imperial gallon",
            ["mpg uk", "mpg imperial"],
            1.609344 / 4.54609
        ),
        u!(
            "l/100km",
            "litres per 100 km",
            ["l/100", "liters per 100km", "litres per 100km"],
            f64::NAN
        ),
        u!("mi/l", "miles per litre", [], 1.609344),
        u!("km/gal", "kilometres per US gallon", [], 1.0 / 3.785411784),
    ],
};

/// Angle units (base: radian).
pub static ANGLE: Category = Category {
    id: "angle",
    name: "Angle",
    base: "rad",
    units: &[
        u!("rad", "radian", ["radian", "radians"], 1.0),
        u!("deg", "degree", ["degree", "degrees", "°"], std::f64::consts::PI / 180.0),
        u!("grad", "gradian", ["gradian", "gradians", "gon"], std::f64::consts::PI / 200.0),
        u!(
            "turn",
            "turn (revolution)",
            ["rev", "revolution", "revolutions", "turns"],
            std::f64::consts::TAU
        ),
        u!("arcmin", "arcminute", ["arcminute", "moa", "'"], std::f64::consts::PI / 10800.0),
        u!("arcsec", "arcsecond", ["arcsecond", "\""], std::f64::consts::PI / 648000.0),
        u!("mil", "NATO mil (1/6400 turn)", ["mils"], std::f64::consts::TAU / 6400.0),
        u!("quadrant", "quadrant", [], std::f64::consts::FRAC_PI_2),
    ],
};

/// Force units (base: newton).
pub static FORCE: Category = Category {
    id: "force",
    name: "Force",
    base: "n",
    units: &[
        u!("n", "newton", ["newton", "newtons"], 1.0),
        u!("kn", "kilonewton", ["kilonewton", "kilonewtons"], 1000.0),
        u!("mn", "meganewton", [], 1e6),
        u!("dyn", "dyne", ["dyne", "dynes"], 1e-5),
        u!("kgf", "kilogram-force", ["kilopond", "kp"], 9.80665),
        u!("gf", "gram-force", [], 0.00980665),
        u!("lbf", "pound-force", ["pound force", "pounds force"], 4.4482216152605),
        u!("ozf", "ounce-force", [], 0.27801385095378),
        u!("tf", "tonne-force", ["tonne force"], 9806.65),
        u!("pdl", "poundal", ["poundal"], 0.138254954376),
    ],
};

/// Density units (base: kilogram per cubic metre).
pub static DENSITY: Category = Category {
    id: "density",
    name: "Density",
    base: "kg/m3",
    units: &[
        u!("kg/m3", "kilogram per cubic metre", ["kg/m^3", "kg/m³"], 1.0),
        u!(
            "g/cm3",
            "gram per cubic centimetre",
            ["g/cc", "g/cm^3", "g/cm³", "g/ml"],
            1000.0
        ),
        u!("kg/l", "kilogram per litre", ["kg/liter"], 1000.0),
        u!("g/l", "gram per litre", ["g/liter"], 1.0),
        u!("mg/l", "milligram per litre", ["ppm (water)"], 0.001),
        u!("lb/ft3", "pound per cubic foot", ["lb/ft^3", "pcf"], 16.01846337396),
        u!("lb/in3", "pound per cubic inch", ["lb/in^3"], 27679.904710191),
        u!("lb/gal", "pound per US gallon", ["lb/gal us"], 119.82642731689),
        u!("oz/in3", "ounce per cubic inch", [], 1729.99404),
        u!("t/m3", "tonne per cubic metre", [], 1000.0),
    ],
};

/// Frequency units (base: hertz).
pub static FREQUENCY: Category = Category {
    id: "frequency",
    name: "Frequency",
    base: "hz",
    units: &[
        u!("hz", "hertz", ["hertz", "1/s", "cps"], 1.0),
        u!("khz", "kilohertz", ["kilohertz"], 1e3),
        u!("mhz", "megahertz", ["megahertz"], 1e6),
        u!("ghz", "gigahertz", ["gigahertz"], 1e9),
        u!("thz", "terahertz", ["terahertz"], 1e12),
        u!("rpm", "revolutions per minute", ["rev/min"], 1.0 / 60.0),
        u!("rps", "revolutions per second", ["rev/s"], 1.0),
        u!("rad/s", "radians per second", [], 1.0 / std::f64::consts::TAU),
        u!("bpm", "beats per minute", [], 1.0 / 60.0),
    ],
};

/// Torque units (base: newton metre).
pub static TORQUE: Category = Category {
    id: "torque",
    name: "Torque",
    base: "nm",
    units: &[
        u!("nm", "newton metre", ["n·m", "n-m", "newton meter", "newton-metre"], 1.0),
        u!("knm", "kilonewton metre", ["kn·m"], 1000.0),
        u!("ncm", "newton centimetre", ["n·cm"], 0.01),
        u!("kgfm", "kilogram-force metre", ["kgf·m", "kgf-m", "kg-m", "kgm"], 9.80665),
        u!("kgfcm", "kilogram-force centimetre", ["kgf·cm", "kg-cm"], 0.0980665),
        u!(
            "lbft",
            "pound-foot",
            ["lb-ft", "lbf·ft", "ft-lb", "ft·lbf", "ftlb", "foot pound"],
            1.3558179483314
        ),
        u!(
            "lbin",
            "pound-inch",
            ["lb-in", "lbf·in", "in-lb", "inlb", "inch pound"],
            0.1129848290276
        ),
        u!("ozin", "ounce-inch", ["oz-in", "ozf·in"], 0.00706155181422),
        u!("dyncm", "dyne centimetre", ["dyn·cm"], 1e-7),
    ],
};

/// All linear categories.
pub static CATEGORIES: &[&Category] = &[
    &LENGTH, &MASS, &AREA, &VOLUME, &SPEED, &TIME, &PRESSURE, &ENERGY, &POWER, &DATA, &DATA_RATE,
    &FUEL, &ANGLE, &FORCE, &DENSITY, &FREQUENCY, &TORQUE,
];

/// Ingredient densities for cooking conversions (g per ml).
pub static INGREDIENTS: &[(&str, f64)] = &[
    ("water", 1.0),
    ("milk", 1.03),
    ("flour", 0.53),
    ("all-purpose flour", 0.53),
    ("bread flour", 0.55),
    ("whole wheat flour", 0.54),
    ("sugar", 0.85),
    ("granulated sugar", 0.85),
    ("brown sugar", 0.82),
    ("powdered sugar", 0.51),
    ("butter", 0.96),
    ("oil", 0.92),
    ("vegetable oil", 0.92),
    ("olive oil", 0.91),
    ("honey", 1.42),
    ("salt", 1.2),
    ("rice", 0.78),
    ("oats", 0.41),
    ("cocoa", 0.41),
    ("cornstarch", 0.54),
    ("yogurt", 1.03),
    ("cream", 1.01),
    ("peanut butter", 1.09),
    ("maple syrup", 1.33),
    ("baking powder", 0.9),
    ("baking soda", 0.9),
    ("ground coffee", 0.4),
    ("nuts (chopped)", 0.6),
    ("cheese (grated)", 0.45),
];

/// Paper sizes in millimetres (width × height, portrait).
pub static PAPER: &[(&str, f64, f64, &str)] = &[
    ("A0", 841.0, 1189.0, "ISO 216"),
    ("A1", 594.0, 841.0, "ISO 216"),
    ("A2", 420.0, 594.0, "ISO 216"),
    ("A3", 297.0, 420.0, "ISO 216"),
    ("A4", 210.0, 297.0, "ISO 216"),
    ("A5", 148.0, 210.0, "ISO 216"),
    ("A6", 105.0, 148.0, "ISO 216"),
    ("A7", 74.0, 105.0, "ISO 216"),
    ("B0", 1000.0, 1414.0, "ISO 216"),
    ("B1", 707.0, 1000.0, "ISO 216"),
    ("B2", 500.0, 707.0, "ISO 216"),
    ("B3", 353.0, 500.0, "ISO 216"),
    ("B4", 250.0, 353.0, "ISO 216"),
    ("B5", 176.0, 250.0, "ISO 216"),
    ("C4", 229.0, 324.0, "ISO 269 envelope"),
    ("C5", 162.0, 229.0, "ISO 269 envelope"),
    ("C6", 114.0, 162.0, "ISO 269 envelope"),
    ("DL", 110.0, 220.0, "ISO 269 envelope"),
    ("Letter", 215.9, 279.4, "ANSI / North America"),
    ("Legal", 215.9, 355.6, "North America"),
    ("Tabloid", 279.4, 431.8, "ANSI B"),
    ("Ledger", 431.8, 279.4, "ANSI B landscape"),
    ("Executive", 184.15, 266.7, "North America"),
    ("Half Letter", 139.7, 215.9, "North America"),
    ("ANSI C", 431.8, 558.8, "ANSI"),
    ("ANSI D", 558.8, 863.6, "ANSI"),
    ("ANSI E", 863.6, 1117.6, "ANSI"),
    ("Foolscap", 203.2, 330.2, "British"),
    ("Business card (ISO)", 85.6, 53.98, "ISO 7810 ID-1"),
    ("Business card (US)", 88.9, 50.8, "US"),
    ("Index card 3x5", 76.2, 127.0, "US"),
    ("Postcard (US)", 101.6, 152.4, "US"),
    ("Photo 4x6", 101.6, 152.4, "Photo"),
    ("Photo 5x7", 127.0, 177.8, "Photo"),
    ("Photo 8x10", 203.2, 254.0, "Photo"),
];

/// Shoe size tables: (US men, US women, UK, EU, foot length cm). Based on
/// common brand charts; always approximate.
pub static SHOES: &[(f64, f64, f64, f64, f64)] = &[
    (4.0, 5.5, 3.5, 36.0, 22.4),
    (4.5, 6.0, 4.0, 36.5, 22.9),
    (5.0, 6.5, 4.5, 37.5, 23.3),
    (5.5, 7.0, 5.0, 38.0, 23.7),
    (6.0, 7.5, 5.5, 38.5, 24.1),
    (6.5, 8.0, 6.0, 39.0, 24.6),
    (7.0, 8.5, 6.5, 40.0, 25.0),
    (7.5, 9.0, 7.0, 40.5, 25.4),
    (8.0, 9.5, 7.5, 41.0, 25.8),
    (8.5, 10.0, 8.0, 42.0, 26.3),
    (9.0, 10.5, 8.5, 42.5, 26.7),
    (9.5, 11.0, 9.0, 43.0, 27.1),
    (10.0, 11.5, 9.5, 44.0, 27.5),
    (10.5, 12.0, 10.0, 44.5, 27.9),
    (11.0, 12.5, 10.5, 45.0, 28.3),
    (11.5, 13.0, 11.0, 45.5, 28.8),
    (12.0, 13.5, 11.5, 46.0, 29.2),
    (12.5, 14.0, 12.0, 46.5, 29.6),
    (13.0, 14.5, 12.5, 47.5, 30.0),
    (14.0, 15.5, 13.5, 48.5, 30.8),
    (15.0, 16.5, 14.5, 49.5, 31.6),
];
