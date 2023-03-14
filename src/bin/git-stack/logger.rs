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
                log::Level::Error => {
                    writeln!(f, "{}: {}", palette.error(record.level()), record.args())
                }
                log::Level::Warn => {
                    writeln!(f, "{}: {}", palette.warn(record.level()), record.args())
                }
                log::Level::Info => writeln!(f, "{}", record.args()),
                log::Level::Debug => {
                    writeln!(f, "{}: {}", palette.debug(record.level()), record.args())
                }
                log::Level::Trace => {
                    writeln!(f, "{}: {}", palette.trace(record.level()), record.args())
                }
            });
        }

        builder.init();
    }
}

#[derive(Copy, Clone, Default, Debug)]
struct Palette {
    error: anstyle::Style,
    warn: anstyle::Style,
    debug: anstyle::Style,
    trace: anstyle::Style,
}

impl Palette {
    pub fn colored() -> Self {
        Self {
            error: anstyle::AnsiColor::Red | anstyle::Effects::BOLD,
            warn: anstyle::AnsiColor::Yellow.into(),
            debug: anstyle::AnsiColor::Blue.into(),
            trace: anstyle::AnsiColor::Cyan.into(),
        }
    }

    pub fn plain() -> Self {
        Self::default()
    }

    pub(crate) fn error<D: std::fmt::Display>(self, display: D) -> Styled<D> {
        Styled::new(display, self.error)
    }

    pub(crate) fn warn<D: std::fmt::Display>(self, display: D) -> Styled<D> {
        Styled::new(display, self.warn)
    }

    pub(crate) fn debug<D: std::fmt::Display>(self, display: D) -> Styled<D> {
        Styled::new(display, self.debug)
    }

    pub(crate) fn trace<D: std::fmt::Display>(self, display: D) -> Styled<D> {
        Styled::new(display, self.trace)
    }
}

#[derive(Debug)]
pub(crate) struct Styled<D> {
    display: D,
    style: anstyle::Style,
}

impl<D: std::fmt::Display> Styled<D> {
    pub(crate) fn new(display: D, style: anstyle::Style) -> Self {
        Self { display, style }
    }
}

impl<D: std::fmt::Display> std::fmt::Display for Styled<D> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "{}", self.style.render())?;
            self.display.fmt(f)?;
            write!(f, "{}", self.style.render_reset())?;
            Ok(())
        } else {
            self.display.fmt(f)
        }
    }
}
