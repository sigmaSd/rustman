use std::fmt;
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

pub trait Colors
where
    Self: fmt::Display,
{
    fn color_print(&self, color: Color) {
        let mut stdout = StandardStream::stdout(ColorChoice::Always);
        let _ = stdout.set_color(ColorSpec::new().set_fg(Some(color)));
        let _ = write!(&mut stdout, "{}", &self);
        let _ = stdout.reset();
        let _ = stdout.flush();
    }
}

impl Colors for &str {}
impl Colors for String {}
