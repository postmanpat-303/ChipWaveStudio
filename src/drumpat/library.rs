//! The built-in groove library. Lanes: K kick, S snare, C clap, H closed
//! hat, O open hat, L/M/T low/mid/high tom, P perc, Z perc 2 (see
//! `super::LANES`); 16 steps per bar, `|` is only for reading.
use super::Pat;

macro_rules! p {
    ($name:expr, $style:expr, $bars:expr, $lo:expr, $hi:expr, $kit:expr, $swing:expr, [$(($c:expr, $l:expr)),* $(,)?]) => {
        Pat {
            name: $name,
            style: $style,
            bars: $bars,
            bpm: ($lo, $hi),
            kit: $kit,
            swing: $swing,
            lanes: &[$(($c, $l)),*],
        }
    };
}

const FOUR: &str = "x...|x...|x...|x...";
const OFF: &str = "..x.|..x.|..x.|..x.";
const BACK: &str = "....|x...|....|x...";
const EIGHTS: &str = "x.x.|x.x.|x.x.|x.x.";
const SIXT: &str = "xoxo|xoxo|xoxo|xoxo";

pub static PATTERNS: &[Pat] = &[
    // ---------------------------------------------------- Psytrance / Dark psy
    p!(
        "Full-on Drive",
        0,
        1,
        140,
        148,
        "Psy Kit",
        0.0,
        [
            ('K', FOUR),
            ('O', OFF),
            ('C', BACK),
            ('H', "...x|...x|...x|...x")
        ]
    ),
    p!(
        "Dark Forest Roll",
        0,
        1,
        145,
        155,
        "Psy Kit",
        0.0,
        [
            ('K', FOUR),
            ('H', "oxXo|oxXo|oxXo|oxXo"),
            ('C', BACK),
            ('Z', "....|....|....|..x.")
        ]
    ),
    p!(
        "Night Gallop",
        0,
        2,
        142,
        150,
        "Psy Kit",
        0.0,
        [
            ('K', FOUR),
            ('O', OFF),
            ('P', "...x|..x.|....|.x..|...x|..x.|..x.|.x.x"),
            ('C', BACK)
        ]
    ),
    p!(
        "Zap Groove",
        0,
        2,
        140,
        148,
        "Psy Kit",
        0.0,
        [
            ('K', FOUR),
            ('O', OFF),
            ('C', BACK),
            ('Z', "....|....|....|....|....|....|..x.|x...")
        ]
    ),
    p!(
        "Hollow Pulse",
        0,
        1,
        138,
        146,
        "Psy Kit",
        0.0,
        [
            ('K', FOUR),
            ('H', "..xo|..xo|..xo|..xo"),
            ('P', ".x..|....|.x..|..x.")
        ]
    ),
    p!(
        "Psy Breakdown",
        0,
        2,
        140,
        150,
        "Psy Kit",
        0.0,
        [
            ('O', OFF),
            ('C', "....|x...|....|x...|....|x...|....|x.xx"),
            ('Z', "....|....|....|..x.|....|....|x...|....")
        ]
    ),
    // ---------------------------------------------------- Techno
    p!(
        "Warehouse Four",
        1,
        1,
        126,
        134,
        "909 Kit",
        0.0,
        [('K', FOUR), ('O', OFF), ('C', BACK), ('H', SIXT)]
    ),
    p!(
        "Berlin Rumble",
        1,
        1,
        128,
        136,
        "Techno Kit",
        0.0,
        [
            ('K', FOUR),
            ('L', "..o.|..o.|..o.|.oo."),
            ('O', OFF),
            ('P', ".x..|..x.|..x.|.x..")
        ]
    ),
    p!(
        "Minimal Tick",
        1,
        1,
        124,
        130,
        "Techno Kit",
        0.0,
        [
            ('K', FOUR),
            ('H', OFF),
            ('P', "x..x|..x.|.x..|x..x"),
            ('Z', "....|....|....|x...")
        ]
    ),
    p!(
        "Industrial Stomp",
        1,
        1,
        130,
        140,
        "Techno Kit",
        0.0,
        [
            ('K', "x...|x...|x...|x.x."),
            ('C', "....|X...|....|X..."),
            ('H', "xxXx|xxXx|xxXx|xxXx"),
            ('Z', "....|..x.|....|....")
        ]
    ),
    p!(
        "Detroit Clap",
        1,
        1,
        125,
        132,
        "909 Kit",
        0.0,
        [
            ('K', FOUR),
            ('C', "....|x...|....|x..x"),
            ('H', OFF),
            ('Z', "x.x.|x.x.|x.x.|x.x.")
        ]
    ),
    // ---------------------------------------------------- House
    p!(
        "Classic House",
        2,
        1,
        120,
        126,
        "909 Kit",
        0.0,
        [('K', FOUR), ('C', BACK), ('O', OFF), ('H', SIXT)]
    ),
    p!(
        "Deep Shuffle",
        2,
        1,
        118,
        124,
        "Disco Kit",
        0.18,
        [
            ('K', FOUR),
            ('C', BACK),
            ('H', SIXT),
            ('O', OFF),
            ('P', "....|..x.|....|..x.")
        ]
    ),
    p!(
        "Garage 2-Step",
        2,
        1,
        128,
        134,
        "909 Kit",
        0.25,
        [
            ('K', "x...|....|..x.|...."),
            ('S', BACK),
            ('H', "x.xx|.x.x|x.xx|.x.x"),
            ('P', "...x|....|...x|....")
        ]
    ),
    p!(
        "Jackin' Groove",
        2,
        1,
        122,
        128,
        "Disco Kit",
        0.1,
        [
            ('K', FOUR),
            ('C', "....|x..x|....|x..."),
            ('O', OFF),
            ('P', "..x.|.x..|..x.|.x..")
        ]
    ),
    p!(
        "Tech House Rolls",
        2,
        1,
        124,
        128,
        "Techno Kit",
        0.08,
        [
            ('K', FOUR),
            ('C', BACK),
            ('H', "xxxx|xxxx|xxxx|xxxx"),
            ('O', OFF),
            ('L', "....|...o|....|...o")
        ]
    ),
    // ---------------------------------------------------- Trance
    p!(
        "Uplifting Gallop",
        3,
        1,
        136,
        140,
        "909 Kit",
        0.0,
        [
            ('K', FOUR),
            ('O', OFF),
            ('H', "x.xx|x.xx|x.xx|x.xx"),
            ('C', BACK)
        ]
    ),
    p!(
        "Trance Offbeat",
        3,
        1,
        136,
        142,
        "909 Kit",
        0.0,
        [
            ('K', FOUR),
            ('O', OFF),
            ('C', BACK),
            ('Z', "x...|....|....|....")
        ]
    ),
    p!(
        "Snare Build",
        3,
        2,
        136,
        142,
        "909 Kit",
        0.0,
        [
            ('K', FOUR),
            ('S', "x.x.|x.x.|x.x.|x.x.|xxxx|xxxx|xxxx|XXXX")
        ]
    ),
    p!(
        "Hard Trance",
        3,
        1,
        145,
        155,
        "Techno Kit",
        0.0,
        [
            ('K', FOUR),
            ('O', OFF),
            ('H', "xxXx|xxXx|xxXx|xxXx"),
            ('C', BACK),
            ('P', ".x..|....|.x..|....")
        ]
    ),
    // ---------------------------------------------------- DnB / Jungle
    p!(
        "Two-Step DnB",
        4,
        1,
        170,
        176,
        "DnB Kit",
        0.0,
        [('K', "x...|....|..x.|...."), ('S', BACK), ('H', EIGHTS)]
    ),
    p!(
        "Amen-ish",
        4,
        2,
        165,
        175,
        "Acoustic-ish Kit",
        0.0,
        [
            ('K', "x.x.|....|..xx|....|..x.|....|..x.|...."),
            ('S', "....|x..o|.o..|x..o|.o..|x..o|.o..|x..."),
            ('Z', EIGHTS)
        ]
    ),
    p!(
        "Liquid Roll",
        4,
        1,
        172,
        176,
        "DnB Kit",
        0.0,
        [
            ('K', "x...|....|..x.|.x.."),
            ('S', BACK),
            ('H', SIXT),
            ('O', "....|...x|....|....")
        ]
    ),
    p!(
        "Jungle Chop",
        4,
        2,
        160,
        170,
        "Acoustic-ish Kit",
        0.0,
        [
            ('K', "x...|...x|..x.|....|x.x.|....|..x.|...."),
            ('S', "....|x...|.o.x|..o.|....|x..o|..x.|x.xx"),
            ('Z', EIGHTS),
            ('T', "....|....|....|....|....|....|....|..x.")
        ]
    ),
    p!(
        "Halftime",
        4,
        1,
        170,
        174,
        "DnB Kit",
        0.0,
        [
            ('K', "x...|..x.|..x.|...."),
            ('S', "....|....|x...|...."),
            ('H', EIGHTS)
        ]
    ),
    // ---------------------------------------------------- Breakbeat
    p!(
        "Funky Break",
        5,
        1,
        120,
        130,
        "Acoustic-ish Kit",
        0.08,
        [
            ('K', "x.x.|....|..x.|...."),
            ('S', "....|x..o|.o..|x..."),
            ('H', EIGHTS),
            ('O', "....|....|....|..x.")
        ]
    ),
    p!(
        "Big Beat",
        5,
        1,
        125,
        135,
        "909 Kit",
        0.0,
        [
            ('K', "x...|...x|x.x.|...."),
            ('S', BACK),
            ('C', BACK),
            ('H', SIXT)
        ]
    ),
    p!(
        "Nu-Skool Break",
        5,
        1,
        130,
        136,
        "Techno Kit",
        0.0,
        [
            ('K', "x...|..x.|..x.|...."),
            ('S', "....|x...|....|x..o"),
            ('H', OFF),
            ('Z', "....|....|....|..x.")
        ]
    ),
    p!(
        "Electro Break",
        5,
        1,
        120,
        128,
        "808 Kit",
        0.0,
        [
            ('K', "x...|..x.|..x.|..x."),
            ('C', BACK),
            ('H', "xxxx|xxxx|xxxx|xxxx"),
            ('P', "....|....|..x.|....")
        ]
    ),
    // ---------------------------------------------------- Hip-hop / Lo-fi
    p!(
        "Boom Bap",
        6,
        1,
        86,
        94,
        "Lo-fi Kit",
        0.2,
        [('K', "x...|...x|..x.|...."), ('S', BACK), ('H', EIGHTS)]
    ),
    p!(
        "Dusty Loop",
        6,
        1,
        80,
        90,
        "Lo-fi Kit",
        0.25,
        [
            ('K', "x...|..x.|..x.|.x.."),
            ('S', BACK),
            ('H', "xox.|xox.|xox.|xox.")
        ]
    ),
    p!(
        "Cozy Rain",
        6,
        2,
        76,
        86,
        "Lo-fi Kit",
        0.22,
        [
            ('K', "x...|....|x.x.|....|x...|....|x...|..x."),
            ('P', BACK),
            ('H', SIXT),
            ('Z', "....|....|....|....|....|....|....|x...")
        ]
    ),
    p!(
        "Late Bus Home",
        6,
        2,
        72,
        82,
        "Lo-fi Kit",
        0.2,
        [
            ('K', "x...|...x|....|....|x.x.|....|..x.|...."),
            ('S', "....|x...|....|x...|....|x...|....|x..o"),
            ('H', EIGHTS),
            ('O', "....|....|....|..x.")
        ]
    ),
    p!(
        "Jazz Hop",
        6,
        1,
        88,
        96,
        "Acoustic-ish Kit",
        0.3,
        [
            ('K', "x...|....|..x.|...."),
            ('S', BACK),
            ('Z', "x..x|x.x.|x..x|x.x."),
            ('H', "....|..x.|....|..x.")
        ]
    ),
    p!(
        "Half Asleep",
        6,
        2,
        70,
        80,
        "Lo-fi Kit",
        0.2,
        [
            ('K', "x...|....|....|..x.|x...|....|..x.|...."),
            ('S', "....|....|x...|....|....|....|x...|...."),
            ('H', EIGHTS),
            ('P', "....|..o.|....|....")
        ]
    ),
    // ---------------------------------------------------- Trap
    p!(
        "Trap Basic",
        7,
        1,
        130,
        150,
        "Trap Kit",
        0.0,
        [
            ('K', "x...|...x|..x.|...."),
            ('C', "....|....|x...|...."),
            ('H', "x.x.|x.x.|x.xx|x.x.")
        ]
    ),
    p!(
        "Hi-hat Rolls",
        7,
        2,
        135,
        150,
        "Trap Kit",
        0.0,
        [
            ('K', "x...|....|..x.|....|x..x|....|..x.|...."),
            ('C', "....|....|x...|...."),
            ('H', "x.x.|xxxx|x.x.|x.xx|x.x.|x.x.|xxxx|xXxX")
        ]
    ),
    p!(
        "Drill Slide",
        7,
        1,
        138,
        144,
        "Trap Kit",
        0.0,
        [
            ('K', "x...|....|.x..|..x."),
            ('S', "....|....|x...|...x"),
            ('H', "x..x|..x.|x..x|..x."),
            ('P', "....|..x.|....|....")
        ]
    ),
    p!(
        "808 Bounce",
        7,
        1,
        130,
        145,
        "Trap Kit",
        0.0,
        [
            ('K', "x..x|....|x...|.x.."),
            ('C', "....|....|x...|...."),
            ('H', EIGHTS),
            ('O', "....|....|....|..x."),
            ('Z', "...x|....|....|....")
        ]
    ),
    // ---------------------------------------------------- Disco / Funk
    p!(
        "Disco Four",
        8,
        1,
        115,
        124,
        "Disco Kit",
        0.0,
        [('K', FOUR), ('S', BACK), ('O', OFF), ('H', SIXT)]
    ),
    p!(
        "Funk Drummer",
        8,
        1,
        98,
        108,
        "Acoustic-ish Kit",
        0.1,
        [
            ('K', "x.x.|..x.|..x.|.x.."),
            ('S', "....|x..o|.o.o|x..o"),
            ('H', EIGHTS)
        ]
    ),
    p!(
        "Philly Groove",
        8,
        1,
        112,
        120,
        "Disco Kit",
        0.0,
        [
            ('K', FOUR),
            ('S', BACK),
            ('O', OFF),
            ('P', "..x.|....|..x.|..x."),
            ('M', "....|....|....|..xx")
        ]
    ),
    p!(
        "Nu-Disco",
        8,
        1,
        116,
        124,
        "Disco Kit",
        0.12,
        [
            ('K', FOUR),
            ('C', BACK),
            ('O', OFF),
            ('T', "...x|..x.|...x|.x.."),
            ('Z', "x..x|....|x..x|....")
        ]
    ),
    // ---------------------------------------------------- Synthwave / 80s
    p!(
        "Outrun Drive",
        9,
        1,
        100,
        118,
        "Synthwave Kit",
        0.0,
        [('K', FOUR), ('S', BACK), ('H', EIGHTS)]
    ),
    p!(
        "Gated Snare Pop",
        9,
        1,
        104,
        116,
        "Synthwave Kit",
        0.0,
        [
            ('K', "x...|..x.|x...|...."),
            ('S', "....|X...|....|X..."),
            ('C', BACK),
            ('H', SIXT)
        ]
    ),
    p!(
        "Midnight Groove",
        9,
        1,
        92,
        104,
        "Synthwave Kit",
        0.0,
        [
            ('K', "x...|...x|..x.|...."),
            ('S', BACK),
            ('H', EIGHTS),
            ('P', "....|....|....|.x..")
        ]
    ),
    p!(
        "Tom Cascade",
        9,
        2,
        100,
        118,
        "Synthwave Kit",
        0.0,
        [
            ('K', "x...|....|x...|....|x...|....|x...|...."),
            ('S', "....|x...|....|x...|....|x...|....|...."),
            ('H', "x.x.|x.x.|x.x.|x.x.|x.x.|x.x.|....|...."),
            ('T', "....|....|....|....|....|....|xx..|...."),
            ('M', "....|....|....|....|....|....|..xx|...."),
            ('L', "....|....|....|....|....|....|....|xxxx")
        ]
    ),
    // ---------------------------------------------------- Reggaeton / Dembow
    p!(
        "Dembow Classic",
        10,
        1,
        88,
        98,
        "Dembow Kit",
        0.0,
        [('K', FOUR), ('S', "...x|..x.|...x|..x."), ('H', EIGHTS)]
    ),
    p!(
        "Perreo",
        10,
        1,
        90,
        100,
        "Dembow Kit",
        0.0,
        [
            ('K', FOUR),
            ('S', "...x|..x.|...x|..x."),
            ('P', ".x..|.x..|.x..|.x.."),
            ('O', "....|....|....|..x.")
        ]
    ),
    p!(
        "Moombah Groove",
        10,
        1,
        105,
        112,
        "Dembow Kit",
        0.0,
        [
            ('K', FOUR),
            ('C', "...x|..x.|...x|..x."),
            ('H', OFF),
            ('T', "....|...x|....|.x..")
        ]
    ),
    p!(
        "Dancehall Riddim",
        10,
        1,
        92,
        104,
        "Dembow Kit",
        0.05,
        [
            ('K', "x...|..x.|x...|..x."),
            ('S', BACK),
            ('P', "..x.|..x.|..x.|x.x."),
            ('H', EIGHTS)
        ]
    ),
    // ---------------------------------------------------- Amiga / Chip
    p!(
        "Tracker Beat",
        11,
        1,
        125,
        135,
        "Chip Kit",
        0.0,
        [('K', "x...|....|x.x.|...."), ('S', BACK), ('H', EIGHTS)]
    ),
    p!(
        "Arcade Run",
        11,
        1,
        140,
        160,
        "Chip Kit",
        0.0,
        [
            ('K', FOUR),
            ('S', BACK),
            ('H', SIXT),
            ('P', "....|....|....|..x.")
        ]
    ),
    p!(
        "Chip Breaks",
        11,
        2,
        130,
        145,
        "Chip Kit",
        0.0,
        [
            ('K', "x...|..x.|..x.|....|x.x.|....|..x.|...x"),
            ('S', "....|x...|....|x..x|....|x...|.x..|x.xx"),
            ('H', EIGHTS)
        ]
    ),
    p!(
        "Demoscene 4/4",
        11,
        1,
        125,
        128,
        "Chip Kit",
        0.0,
        [
            ('K', FOUR),
            ('C', BACK),
            ('O', OFF),
            ('Z', "....|..x.|....|...."),
            ('M', "....|....|....|.xx.")
        ]
    ),
    // ---------------------------------------------------- Afro / Latin
    p!(
        "Afro House",
        12,
        1,
        118,
        124,
        "Latin Kit",
        0.05,
        [
            ('K', FOUR),
            ('P', "..x.|.x..|x..x|.x.."),
            ('T', "..x.|..xx|..x.|...x"),
            ('M', ".x..|....|.x..|...."),
            ('O', OFF)
        ]
    ),
    p!(
        "Son Clave",
        12,
        2,
        95,
        110,
        "Latin Kit",
        0.0,
        [
            ('Z', "x..x|..x.|....|....|..x.|x...|....|...."),
            ('K', FOUR),
            ('L', "...x|....|...x|...."),
            ('T', "..x.|..x.|..x.|..xx")
        ]
    ),
    p!(
        "Samba Batucada",
        12,
        1,
        96,
        104,
        "Latin Kit",
        0.0,
        [
            ('K', "x...|...x|x...|...x"),
            ('S', "xoox|xoox|xoox|xoox"),
            ('P', "x.x.|.x.x|.x.x|x.x."),
            ('H', SIXT)
        ]
    ),
    p!(
        "Afrobeat",
        12,
        1,
        100,
        112,
        "Acoustic-ish Kit",
        0.08,
        [
            ('K', "x...|..x.|x...|...."),
            ('S', BACK),
            ('H', EIGHTS),
            ('P', "x.x.|xx.x|.x.x|x.x.")
        ]
    ),
    p!(
        "Bossa Nova",
        12,
        2,
        120,
        140,
        "Latin Kit",
        0.0,
        [
            ('Z', "x..x|..x.|..x.|.x..|x..x|..x.|..x.|.x.."),
            ('K', "x..x|x..x|x..x|x..x"),
            ('H', EIGHTS)
        ]
    ),
    // ---------------------------------------------------- Fills
    p!(
        "Snare Roll",
        13,
        1,
        80,
        180,
        "909 Kit",
        0.0,
        [('S', "x.x.|x.x.|xxxx|XXXX")]
    ),
    p!(
        "Tom Run",
        13,
        1,
        80,
        180,
        "Acoustic-ish Kit",
        0.0,
        [
            ('T', "xxx.|....|....|...."),
            ('M', "....|xxx.|....|...."),
            ('L', "....|....|xxx.|...."),
            ('S', "....|....|....|xxXX")
        ]
    ),
    p!(
        "Crash Drop",
        13,
        1,
        80,
        180,
        "909 Kit",
        0.0,
        [('K', "X...|....|....|...."), ('Z', "X...|....|....|....")]
    ),
    p!(
        "Psy Riser Fill",
        13,
        1,
        140,
        150,
        "Psy Kit",
        0.0,
        [
            ('K', "x...|x...|x...|x..."),
            ('C', "x.x.|x.x.|xxxx|xxxx"),
            ('Z', "....|....|....|x.x.")
        ]
    ),
    p!(
        "Breakdown Stop",
        13,
        1,
        80,
        180,
        "Techno Kit",
        0.0,
        [('K', "x.x.|x.x.|....|...."), ('C', "....|....|X...|....")]
    ),
    p!(
        "DnB Fill",
        13,
        1,
        165,
        176,
        "DnB Kit",
        0.0,
        [
            ('S', "x..x|.x.x|xx.x|xxxx"),
            ('K', "x...|..x.|....|...."),
            ('T', "....|....|..x.|....")
        ]
    ),
    p!(
        "Trap Roll Fill",
        13,
        1,
        130,
        150,
        "Trap Kit",
        0.0,
        [
            ('H', "xxxx|xxxx|xxxx|xxxx"),
            ('C', "....|....|x...|.xxx"),
            ('K', "x...|....|....|....")
        ]
    ),
];
