use log::*;

pub fn init() {
    static LOGGER: SimpleLogger = SimpleLogger;
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(match option_env!("LOG") {
        Some("error") => LevelFilter::Error,
        Some("warn") => LevelFilter::Warn,
        Some("info") => LevelFilter::Info,
        Some("debug") => LevelFilter::Debug,
        Some("trace") => LevelFilter::Trace,
        _ => LevelFilter::Off,
    });
}

struct SimpleLogger;

impl Log for SimpleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let color = Color::from(record.level());
            let (show, code) = color.to_console_code();
            println!(
                "\u{1B}[{};{}m[{:>5}] {}\u{1B}[0m",
                show,
                code + 30,
                record.level(),
                record.args()
            );
        }
    }
    fn flush(&self) {}
}

impl From<Level> for Color {
    fn from(level: Level) -> Self {
        match level {
            Level::Error => Color::Red,
            Level::Warn => Color::Yellow,
            Level::Info => Color::Blue,
            Level::Debug => Color::Green,
            Level::Trace => Color::DarkGray,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

impl Color {
    fn to_console_code(&self) -> (u8, u8) {
        match self {
            Color::Black => (0, 0),
            Color::Blue => (0, 4),
            Color::Green => (0, 2),
            Color::Cyan => (0, 6),
            Color::Red => (0, 1),
            Color::Magenta => (0, 5),
            Color::Brown => (0, 3),
            Color::LightGray => (1, 7),
            Color::DarkGray => (0, 7),
            Color::LightBlue => (1, 4),
            Color::LightGreen => (1, 2),
            Color::LightCyan => (1, 6),
            Color::LightRed => (1, 1),
            Color::Pink => (1, 5),
            Color::Yellow => (1, 3),
            Color::White => (1, 0),
        }
    }
}
