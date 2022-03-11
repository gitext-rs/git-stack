use std::io::Write;

pub fn init_logging(
    level: clap_verbosity_flag::Verbosity<clap_verbosity_flag::InfoLevel>,
    colored: bool,
) {
    if let Some(level) = level.log_level() {
        let palette = if colored {
            Palette::colored()
        } else {
            Palette::plain()
        };

        let mut builder = env_logger::Builder::new();
        builder.write_style(if colored {
            env_logger::WriteStyle::Always
        } else {
            env_logger::WriteStyle::Never
        });

        builder.filter(None, level.to_level_filter());

        if level == log::LevelFilter::Trace || level == log::LevelFilter::Debug {
            builder.format_timestamp_secs();
        } else {
            builder.format(move |f, record| match record.level() {
                log::Level::Error => writeln!(
                    f,
                    "{}: {}",
                    palette.error.paint(record.level()),
                    record.args()
                ),
                log::Level::Warn => writeln!(
                    f,
                    "{}: {}",
                    palette.warn.paint(record.level()),
                    record.args()
                ),
                log::Level::Info => writeln!(f, "{}", record.args()),
                log::Level::Debug => writeln!(
                    f,
                    "{}: {}",
                    palette.debug.paint(record.level()),
                    record.args()
                ),
                log::Level::Trace => writeln!(
                    f,
                    "{}: {}",
                    palette.trace.paint(record.level()),
                    record.args()
                ),
            });
        }

        builder.init();
    }
}

#[derive(Copy, Clone, Debug)]
struct Palette {
    error: yansi::Style,
    warn: yansi::Style,
    debug: yansi::Style,
    trace: yansi::Style,
}

impl Palette {
    pub fn colored() -> Self {
        Self {
            error: yansi::Style::new(yansi::Color::Red).bold(),
            warn: yansi::Style::new(yansi::Color::Yellow),
            debug: yansi::Style::new(yansi::Color::Blue),
            trace: yansi::Style::new(yansi::Color::Cyan),
        }
    }

    pub fn plain() -> Self {
        Self {
            error: yansi::Style::default(),
            warn: yansi::Style::default(),
            debug: yansi::Style::default(),
            trace: yansi::Style::default(),
        }
    }
}
