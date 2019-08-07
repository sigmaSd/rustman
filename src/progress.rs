use std::io::Write;
use termcolor::WriteColor;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream};

pub struct Progress {
    width: usize,
    step: usize,
    current: usize,
    printer: StandardStream,
}

impl Progress {
    pub fn new(max: usize) -> Self {
        let mut printer = StandardStream::stdout(ColorChoice::Always);
        printer
            .set_color(ColorSpec::new().set_fg(Some(Color::Red)))
            .unwrap();

        let width = std::cmp::min(30, max);
        let step = max.checked_div(width).unwrap_or(width);
        let current = 0;

        Self {
            width,
            step,
            current,
            printer,
        }
    }

    pub fn advance(&mut self) {
        self.current += 1;
    }

    pub fn print(&mut self) {
        let progress = self.current.checked_div(self.step).unwrap_or(0);
        let remaining = match self.width.checked_sub(progress) {
            Some(n) => n,
            None => return,
        };
        let progress: String = std::iter::repeat('#').take(progress).collect();
        let remaining: String = std::iter::repeat(' ').take(remaining).collect();

        write!(&mut self.printer, "\r").unwrap();
        write!(&mut self.printer, "\t\t[{}{}]", progress, remaining).unwrap();
        self.printer.flush().unwrap();
    }
}
