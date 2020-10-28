use std::fmt;
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

pub trait Colors
where
    Self: fmt::Display,
{
    fn color_print(&self, color: Color) -> super::Result<()> {
        let mut stdout = StandardStream::stdout(ColorChoice::Auto);
        stdout.set_color(ColorSpec::new().set_fg(Some(color)))?;
        write!(&mut stdout, "{}", &self)?;
        stdout.reset()?;
        stdout.flush()?;
        Ok(())
    }
}

impl Colors for &str {}
impl Colors for String {}
