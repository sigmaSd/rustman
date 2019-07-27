use std::fs;
use std::io::{self, Write};
use std::process::Command;
use std::sync::{Arc, Mutex};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
mod colors;
use colors::Colors;
mod unchained;
use unchained::{Sugar, Unchained};

type Name = String;
type Version = String;
type OnlineVersion = String;
type Description = String;

enum Action {
    FullUpdate,
    GetFromGit(String),
    GetFromName(Vec<String>),
}

#[derive(Debug)]
enum Errors {
    IoError(io::Error),
    Utf8Error(std::string::FromUtf8Error),
    Custom(&'static str),
}

impl From<io::Error> for Errors {
    fn from(e: io::Error) -> Errors {
        Errors::IoError(e)
    }
}

impl From<std::string::FromUtf8Error> for Errors {
    fn from(e: std::string::FromUtf8Error) -> Errors {
        Errors::Utf8Error(e)
    }
}

impl std::fmt::Display for Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let error = match self {
            Errors::IoError(e) => e.to_string(),
            Errors::Utf8Error(e) => e.to_string(),
            Errors::Custom(e) => e.to_string(),
        };

        write!(f, "Something happened! {}\n Rustman Out", error)
    }
}

fn main() {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);

    match parse_args() {
        Action::GetFromName(packages) => match get_from_name(packages) {
            Ok(_) => (),
            Err(e) => {
                writeln!(&mut stdout, "{}", e).unwrap();
                stdout.reset().unwrap();
                stdout.flush().unwrap();
            }
        },
        Action::GetFromGit(link) => get_from_link(&link),
        Action::FullUpdate => full_update().unwrap_or_default(),
    }
}

fn parse_args() -> Action {
    let envs: Vec<String> = std::env::args().skip(1).collect();
    if envs.is_empty() {
        return Action::FullUpdate;
    }
    if envs[0].starts_with("https") {
        Action::GetFromGit(envs[0].clone())
    } else {
        Action::GetFromName(envs)
    }
}

fn get_from_name(packages: Vec<String>) -> Result<(), Errors> {
    let raw_hits = search(&packages)?;

    if raw_hits.is_empty() {
        return Err(Errors::Custom("No matches found!"));
    }

    let hit = Arc::new(Mutex::new(vec![]));
    let progress = Arc::new(Mutex::new(Progress::new(raw_hits.len())));

    let hit_c = hit.clone();

    raw_hits
        .into_iter()
        .unchained_for_each(move |(name, version, description)| {
            let hit_cc = hit_c.clone();
            let progress_c = progress.clone();
            if is_bin(&name, &version) {
                hit_cc.lock().unwrap().push((name, version, description));
                progress_c.lock().unwrap().advance();
                progress_c.lock().unwrap().print();
            }
        })
        .join();

    //new line
    println!();

    main_loop(Arc::try_unwrap(hit).unwrap().into_inner().unwrap())
}

fn get_from_link(link: &str) {
    let tmp_dir = std::env::temp_dir()
        .join("rustman")
        .join(link.split('/').last().unwrap());
    let _ = fs::create_dir_all(&tmp_dir);

    Command::new("git")
        .current_dir(&tmp_dir)
        .arg("clone")
        .arg(link)
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}

fn full_update() -> Result<(), Errors> {
    let installed = look_for_installed();

    let online_versions = Arc::new(Mutex::new(vec![]));
    let progress = Arc::new(Mutex::new(Progress::new(installed.len())));

    let online_versions_c = online_versions.clone();

    installed
        .clone()
        .into_iter()
        .unchained_for_each(move |p| {
            let online_versions_cc = online_versions_c.clone();
            let progress_c = progress.clone();

            let p = search_one_pkg(&p.0);
            if let Ok(Some(p)) = p {
                online_versions_cc.lock().unwrap().push(p);
            }
            progress_c.lock().unwrap().advance();
            progress_c.lock().unwrap().print();
        })
        .join();

    //new line
    println!();

    // clear color
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let _ = stdout.reset();
    let _ = stdout.flush();

    let online_versions = Arc::try_unwrap(online_versions)
        .unwrap()
        .into_inner()
        .unwrap();

    let needs_update_pkgs = diff(&installed, &online_versions);

    let widths: (usize, usize) = needs_update_pkgs
        .iter()
        .map(|p| (p.0.len(), p.1.len()))
        .max()
        .unwrap_or_default();

    let offset1 = widths.0.saturating_sub(4);
    let offset2 = widths.1.saturating_sub(9);
    let installed_col_offset = offset1 + 4;
    let available_col_offset = installed_col_offset + offset2;

    format!(
        "Name{}\tInstalled{}\tAvailable",
        std::iter::repeat(" ").take(offset1).collect::<String>(),
        std::iter::repeat(" ").take(offset2).collect::<String>(),
    )
    .color_print(Color::Cyan);
    println!();

    for package in &needs_update_pkgs {
        let offsets = (
            std::iter::repeat(" ")
                .take(installed_col_offset.saturating_sub(package.0.len()))
                .collect::<String>(),
            std::iter::repeat(" ")
                .take(available_col_offset)
                .collect::<String>(),
        );
        package.0.color_print(Color::Blue);
        format!("{}\t", offsets.0).color_print(Color::White);
        package.1.color_print(Color::Green);
        format!("{}\t", offsets.1).color_print(Color::White);
        package.2.color_print(Color::Yellow);
        println!();
    }

    let update_all = |v: &Vec<(&Name, &Version, &Description)>| {
        v.iter().for_each(|p| install(p.0));
    };

    ":: Proceed with installation? [Y/n]".color_print(Color::Yellow);
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if answer.trim().is_empty() || answer.trim().to_lowercase() == "y" {
        update_all(&needs_update_pkgs);
    }

    Ok(())
}

fn diff<'a>(
    installed: &'a [(Name, Version)],
    online_versions: &'a [(Name, Version, Description)],
) -> Vec<(&'a Name, &'a Version, &'a OnlineVersion)> {
    let mut res = vec![];
    for ins_pkg in installed.iter() {
        if let Some(online_pkg) = online_versions
            .iter()
            .find(|(n, v, _)| n == &ins_pkg.0 && v != &ins_pkg.1)
        {
            res.push((&ins_pkg.0, &ins_pkg.1, &online_pkg.1));
        }
    }
    res
}

fn main_loop(r: Vec<(String, String, String)>) -> Result<(), Errors> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let installed = look_for_installed();
    let num = r.len();
    if num == 0 {
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)))?;
        writeln!(&mut stdout, "No matches found!")?;
        return Ok(());
    }

    for (i, (n, v, d)) in r.iter().enumerate() {
        let current_bin = installed.iter().find(|(name, _version)| name == n);
        let suffix = if let Some(hit) = current_bin {
            format!("Installed [v{}]", hit.1)
        } else {
            "".into()
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

fn search_one_pkg(s: &str) -> Result<Option<(Name, Version, Description)>, Errors> {
    let hit = search(vec![s.to_owned()].as_slice())?;
    let hit = hit.get(0);
    if let Some(h) = hit.as_ref() {
        if h.0 == s {
            return Ok(hit.cloned());
        }
    }
    Ok(None)
}

#[cfg(not(test))]
fn search(s: &[String]) -> Result<Vec<(Name, Version, Description)>, Errors> {
    let out = std::process::Command::new("cargo")
        .args(&["search", "--limit", "100"])
        .args(s)
        .output()?
        .stdout;

    let mut results = vec![];

    for line in String::from_utf8(out)?.lines() {
        if line.starts_with("...") {
            continue;
        }
        let mut line = line.split('=');
        let name = line.next().unwrap().trim().to_string();
        let (version, description) = parse_version_desc(line.next().unwrap());
        results.push((name, version, description));
    }

    Ok(results)
}
#[cfg(test)]
fn search(_s: &[String]) -> Result<Vec<(Name, Version, Description)>, Errors> {
    Ok(vec![(
        "irust".to_string(),
        "0.6.1".to_string(),
        "".to_string(),
    )])
}

fn parse_version_desc(s: &str) -> (Name, Version) {
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

fn look_for_installed() -> Vec<(Name, Version)> {
    let installed_bins_toml = fs::read_to_string(
        dirs::home_dir()
            .unwrap()
            .join(".cargo")
            .join(".crates.toml"),
    )
    .unwrap();

    let table: toml::map::Map<String, toml::Value> =
        toml::from_str(&installed_bins_toml.trim()).unwrap();

    if table.is_empty() {
        return vec![];
    }

    if let toml::Value::Table(table) = &table["v1"] {
        table
            .keys()
            .map(|k| {
                let mut key = k.split_whitespace();
                (
                    key.next().unwrap().to_string(),
                    key.next().unwrap().to_string(),
                )
            })
            .collect()
    } else {
        vec![]
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
        let step = max.checked_div(width).unwrap_or(width);
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
