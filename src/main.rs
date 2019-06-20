use std::fs;
use std::io::{self, Write};
use std::process::Command;
use std::sync::{Arc, Mutex};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

enum Errors {
    IoError(io::Error),
    Custom(&'static str),
}

impl From<io::Error> for Errors {
    fn from(e: io::Error) -> Errors {
        Errors::IoError(e)
    }
}

impl std::fmt::Display for Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let error = match self {
            Errors::IoError(e) => e.to_string(),
            Errors::Custom(e) => e.to_string(),
        };

        write!(f, "Something happened! {}\n Rustman Out", error)
    }
}

fn main() {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);

    match run() {
        Ok(_) => (),
        Err(e) => {
            writeln!(&mut stdout, "{}", e).unwrap();
            stdout.reset().unwrap();
            stdout.flush().unwrap();
        }
    }
}

fn run() -> Result<(), Errors> {
    let packages: Vec<String> = std::env::args().skip(1).collect();

    if packages.is_empty() {
        return Err(Errors::Custom("No package specified!"));
    }

    let res = search(&packages)
        .unwrap()
        .lines()
        .map(ToOwned::to_owned)
        .collect::<Vec<String>>();

    if res.is_empty() {
        return Err(Errors::Custom("No matches found!"));
    }

    let hit = Arc::new(Mutex::new(vec![]));
    let progress = Arc::new(Mutex::new(Progress::new(res.len())));

    let hit_c = hit.clone();
    let mut threads = vec![];
    for line in res {
        let hit_cc = hit_c.clone();
        let progress_c = progress.clone();
        threads.push(std::thread::spawn(move || {
            if line.starts_with("...") {
                return;
            }
            let mut line = line.split('=');
            let n = line.next().unwrap().trim().to_string();
            let (v, d) = parse_version_desc(line.next().unwrap());

            if is_bin(&n, &v) {
                hit_cc.lock().unwrap().push((n, v, d));
                progress_c.lock().unwrap().advance();
                progress_c.lock().unwrap().print();
            }
        }));
    }
    for t in threads {
        t.join().unwrap();
    }

    // keep only one strong refrence to hit so we can unwrap safely
    drop(hit_c);

    //new line
    println!();

    main_loop(Arc::try_unwrap(hit).unwrap().into_inner().unwrap())
}

fn main_loop(r: Vec<(String, String, String)>) -> Result<(), Errors> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let installed = look_for_installed(&r);
    let num = r.len();
    if num == 0 {
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)))?;
        writeln!(&mut stdout, "No matches found!")?;
        return Ok(());
    }

    for (i, (n, v, d)) in r.iter().enumerate() {
        let suffix = if installed.contains(&n) {
            "(Installed)"
        } else {
            ""
        };

        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
        write!(&mut stdout, "{}", num - i)?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)))?;
        write!(&mut stdout, " {}", n)?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Black)))?;
        write!(&mut stdout, " = ")?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        write!(&mut stdout, "\"{}\"", v)?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
        write!(&mut stdout, " {}", suffix)?;
        // description is optional
        if !d.is_empty() {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Magenta)))?;
            write!(&mut stdout, " #{}", d)?;
        }
        writeln!(&mut stdout)?;
    }

    stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
    writeln!(
        &mut stdout,
        "==> Packages to install (eg: 1 2 3, 1-3 or ^4)"
    )?;
    write!(&mut stdout, "==> ")?;
    stdout.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input
        .trim_end()
        .parse::<usize>()
        .map_err(|_| Errors::Custom("error while parsing input"))?;

    if num < input {
        return Err(Errors::Custom("Input is incorrect"));
    }

    let reqeusted = r.get(num - input);

    if let Some(req) = reqeusted {
        install(&req.0);
    } else {
        return Err(Errors::Custom("0 is not a valid input"));
    }

    Ok(())
}

#[cfg(not(test))]
fn search(s: &[String]) -> io::Result<String> {
    let out = std::process::Command::new("cargo")
        .args(&["search", "--limit", "100"])
        .args(s)
        .output()?
        .stdout;
    Ok(String::from_utf8(out).unwrap())
}
#[cfg(test)]
fn search(_s: &str) -> io::Result<String> {
    Ok("irust = \"0.6.1\" ".to_string())
}

fn parse_version_desc(s: &str) -> (String, String) {
    let mut s = s.split('#');
    let v = s.next().unwrap();
    // description is optional
    let d = s.next().unwrap_or("");
    let mut v = v.trim()[1..].to_string();
    v.pop();
    let d = d.trim().to_string();

    (v, d)
}

#[cfg(not(test))]
fn is_bin(n: &str, v: &str) -> bool {
    let doc = format!("https://docs.rs/crate/{}/{}", n, v);
    let mut writer = Vec::new();
    http_req::request::get(doc, &mut writer).unwrap();

    let writer = String::from_utf8(writer).unwrap();
    writer.contains("is not a library")
}
#[cfg(test)]
fn is_bin(_n: &str, _v: &str) -> bool {
    true
}

fn install(s: &str) {
    Command::new("cargo")
        .args(&["install", "--force", s])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}

fn look_for_installed(r: &[(String, String, String)]) -> Vec<String> {
    let r_names: Vec<&String> = r.iter().map(|(n, _v, _d)| n).collect();

    fs::read_dir(dirs::home_dir().unwrap().join(".cargo/bin"))
        .unwrap()
        .filter_map(|e| {
            let file_name = e
                .as_ref()
                .unwrap()
                .file_name()
                .to_str()
                .unwrap()
                .to_string();
            let file_name = remove_extention(file_name);

            if r_names.contains(&&file_name) {
                Some(file_name)
            } else {
                None
            }
        })
        .collect()
}

fn remove_extention(s: String) -> String {
    if s.contains('.') {
        s.rsplit('.').nth(1).unwrap().to_owned()
    } else {
        s
    }
}

struct Progress {
    width: usize,
    current: usize,
    step: usize,
    printer: StandardStream,
}

impl Progress {
    fn new(max: usize) -> Self {
        let mut printer = StandardStream::stdout(ColorChoice::Always);
        printer
            .set_color(ColorSpec::new().set_fg(Some(Color::Red)))
            .unwrap();

        let width = max / 2;
        let step = max / width;
        let current = 0;

        Self {
            width,
            step,
            current,
            printer,
        }
    }

    fn advance(&mut self) {
        self.current += 1;
    }

    fn print(&mut self) {
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

#[test]
fn t() {
    main();
}
