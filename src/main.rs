use dirs_next as dirs;
use serde::Deserialize;
use std::fs;
use std::io::{self, Write};
use std::process::Command;
use std::sync::{Arc, Mutex};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

mod colors;
use colors::Colors;

type Name = String;
type Version = String;
type OnlineVersion = String;
type Description = String;

const PROGRESS_PRINTING_ERROR: &str = "Error printing progress";
const MUTEX_LOCK_ERROR: &str = "Error locking mutex";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

enum Action {
    FullUpdate,
    SearchByName(Vec<String>),
    InstallPackage(Vec<String>),
    RemovePackage(Vec<String>),
    ShowInstalled,
}

fn main() {
    tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .expect("Error building tokio runtime")
        .block_on(async {
            if let Err(e) = async_main().await {
                eprintln!("Something happened: {}", e);
            }
        })
}

async fn async_main() -> Result<()> {
    let mut stdout = StandardStream::stdout(ColorChoice::Auto);

    match parse_args() {
        Action::SearchByName(packages) => {
            if let Err(e) = get_from_name(packages).await {
                writeln!(&mut stdout, "{}", e)?;
                stdout.reset()?;
                stdout.flush()?;
            }
        }
        Action::FullUpdate => full_update().await?,
        Action::InstallPackage(packages) => install_packages(packages)?,
        Action::RemovePackage(packages) => remove_packages(packages)?,
        Action::ShowInstalled => show_installed()?,
    }
    Ok(())
}

fn parse_args() -> Action {
    let envs: Vec<String> = std::env::args().skip(1).collect();

    match envs.get(0).map(|s| s.as_str()) {
        Some("-S") => Action::InstallPackage(envs[1..].to_vec()),
        Some("-R") => Action::RemovePackage(envs[1..].to_vec()),
        Some("--list") => Action::ShowInstalled,
        Some(_) => Action::SearchByName(envs),
        None => Action::FullUpdate,
    }
}

fn show_installed() -> Result<()> {
    let installed = look_for_installed()?;
    let max_width = installed
        .iter()
        .map(|p| p.0.len())
        .max()
        .unwrap_or_default();

    for p in installed {
        format!("{:width$}\t", p.0, width = max_width).color_print(Color::Yellow)?;
        format!("{}\n", p.1).color_print(Color::Red)?;
    }
    Ok(())
}

fn install_packages(packages: Vec<String>) -> Result<()> {
    let actual_pkgs =
        packages
            .iter()
            .filter(|p| !p.starts_with('-'))
            .fold(String::new(), |acc, x| {
                if acc.is_empty() {
                    acc + x
                } else {
                    acc + " " + x
                }
            });
    format!("Installing pacakges: {}\n", &actual_pkgs).color_print(Color::Blue)?;
    install(&packages.iter().map(|s| s.as_str()).collect::<Vec<&str>>())
        .unwrap_or_else(|_| panic!("Error installing {:?}", packages));
    "Done!".color_print(Color::Blue)?;
    Ok(())
}

fn remove_packages(packages: Vec<String>) -> Result<()> {
    format!("Removing pacakges: {:?}\n", &packages).color_print(Color::Blue)?;
    packages.iter().for_each(|p| {
        remove(p).unwrap_or_else(|_| panic!("Error removing {}", p));
    });

    "Done!".color_print(Color::Blue)?;
    Ok(())
}

async fn get_from_name(packages: Vec<String>) -> Result<()> {
    let raw_hits = search(&packages)?;

    if raw_hits.is_empty() {
        return Err("No matches found!".into());
    }

    let hit = Arc::new(Mutex::new(vec![]));
    let progress = Arc::new(Mutex::new(Progress::new(raw_hits.len())?));

    let hit_c = hit.clone();

    let client = reqwest::Client::new();
    let f = raw_hits
        .into_iter()
        .map(move |(name, version, description)| {
            let hit_cc = hit_c.clone();
            let progress_c = progress.clone();
            let client = client.clone();

            tokio::spawn(async move {
                match is_bin(client, &name, &version).await {
                    Ok(_) => {
                        hit_cc
                            .lock()
                            .expect(MUTEX_LOCK_ERROR)
                            .push((name, version, description));
                    }
                    Err(e) => {
                        eprintln!("Error while checking crate {} type. Error: {}", name, e);
                    }
                };
                let mut progress = progress_c.lock().expect(MUTEX_LOCK_ERROR);
                progress.advance();
                progress.print().expect(PROGRESS_PRINTING_ERROR);
            })
        });

    futures::future::join_all(f).await;

    //new line
    println!();

    // safe unwrap since we awaited all futures
    main_loop(Arc::try_unwrap(hit).unwrap().into_inner()?)
}

async fn full_update() -> Result<()> {
    let installed = look_for_installed()?;

    let online_versions = Arc::new(Mutex::new(vec![]));
    let progress = Arc::new(Mutex::new(Progress::new(installed.len())?));

    //let online_versions_c = online_versions.clone();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_str("rustman")?,
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    let f = installed.clone().into_iter().map(|p| {
        let client = client.clone();
        let online_versions_c = online_versions.clone();
        let progress_c = progress.clone();
        tokio::spawn(async move {
            let p = search_one_pkg(client, &p.0).await;
            if let Ok(p) = p {
                online_versions_c.lock().expect(MUTEX_LOCK_ERROR).push(p);
            }
            let mut progress = progress_c.lock().expect(MUTEX_LOCK_ERROR);
            progress.advance();
            progress.print().expect(PROGRESS_PRINTING_ERROR);
        })
    });

    futures::future::join_all(f).await;

    //new line
    println!();

    // clear color
    let mut stdout = StandardStream::stdout(ColorChoice::Auto);
    stdout.reset()?;
    stdout.flush()?;

    // safe unwrap since we awaited all futures
    let online_versions = Arc::try_unwrap(online_versions).unwrap().into_inner()?;

    let needs_update_pkgs = diff(&installed, &online_versions);

    if needs_update_pkgs.is_empty() {
        "Everything is uptodate!".color_print(Color::Blue)?;
        println!();
        return Ok(());
    }

    let w1 = std::cmp::max(
        needs_update_pkgs
            .iter()
            .map(|p| p.0.len())
            .max()
            .unwrap_or_default(),
        "Name".len(),
    );
    let w2 = std::cmp::max(
        needs_update_pkgs
            .iter()
            .map(|p| p.1.len())
            .max()
            .unwrap_or_default(),
        "Installed".len(),
    );
    let w3 = std::cmp::max(
        needs_update_pkgs
            .iter()
            .map(|p| p.2.len())
            .max()
            .unwrap_or_default(),
        "Available".len(),
    );

    format!(
        "{:w1$}\t{:w2$}\t{:w3$}",
        "Name",
        "Installed",
        "Available",
        w1 = w1,
        w2 = w2,
        w3 = w3,
    )
    .color_print(Color::Cyan)?;
    println!();

    for package in &needs_update_pkgs {
        format!("{:w1$}\t", package.0, w1 = w1).color_print(Color::Blue)?;
        format!("{:w2$}\t", package.1, w2 = w2).color_print(Color::Green)?;
        format!("{:w3$}\t", package.2, w3 = w3).color_print(Color::Yellow)?;
        println!();
    }

    let update_all = |v: &Vec<(&Name, &Version, &Description)>| {
        v.iter().for_each(|p| {
            install(&[p.0]).unwrap_or_else(|_| panic!("Error installing {}", p.0));
        });
    };

    ":: Proceed with installation? [Y/n]".color_print(Color::Yellow)?;
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

fn main_loop(r: Vec<(String, String, String)>) -> Result<()> {
    let mut stdout = StandardStream::stdout(ColorChoice::Auto);
    let installed = look_for_installed()?;
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

    let input = input.trim_end().parse::<usize>()?;

    if num < input {
        return Err("Input is incorrect".into());
    }

    let reqeusted = r.get(num - input);

    if let Some(req) = reqeusted {
        install(&[&req.0]).unwrap_or_else(|_| panic!("Error installing {}", req.0));
    } else {
        return Err("0 is not a valid input".into());
    }

    Ok(())
}

#[derive(Deserialize, Debug)]
struct Resp {
    #[serde(rename = "crate")]
    r_crate: Crate,
}

#[derive(Deserialize, Debug)]
struct Crate {
    description: String,
    max_version: String,
    name: String,
}

async fn search_one_pkg(client: reqwest::Client, s: &str) -> Result<(Name, Version, Description)> {
    const URL: &str = "https://crates.io/api/v1/crates";
    let resp: Resp = client
        .get(&format!("{}/{}", URL, s))
        .send()
        .await?
        .json()
        .await?;

    Ok((
        resp.r_crate.name,
        resp.r_crate.max_version,
        resp.r_crate.description,
    ))
}

fn search(s: &[String]) -> Result<Vec<(Name, Version, Description)>> {
    let out = std::process::Command::new("cargo")
        .args(&["search", "--limit", "100"])
        .args(s)
        .output()?
        .stdout;

    let mut results = vec![];

    let try_parse = |line: &str| {
        if line.starts_with("...") {
            return None;
        }
        let mut line = line.split('=');
        let name = line.next()?.trim().to_string();
        let (version, description) = parse_version_desc(line.next()?)?;
        Some((name, version, description))
    };
    for line in String::from_utf8(out)?.lines() {
        if let Some((name, version, description)) = try_parse(line) {
            results.push((name, version, description));
        }
    }

    Ok(results)
}

fn parse_version_desc(s: &str) -> Option<(Name, Version)> {
    let mut s = s.split('#');
    let v = s.next()?;
    // description is optional
    let d = s.next().unwrap_or("");
    let mut v = v.trim()[1..].to_string();
    v.pop();
    let d = d.trim().to_string();

    Some((v, d))
}

async fn is_bin(client: reqwest::Client, n: &str, v: &str) -> Result<bool> {
    let doc = format!("https://docs.rs/crate/{}/{}", n, v);

    let resp = client.get(&doc).send().await?.text().await?;

    Ok(resp.contains("is not a library"))
}

fn install(s: &[&str]) -> Result<()> {
    Command::new("cargo")
        .args(&["install", "--force"])
        .args(s)
        .spawn()?
        .wait()?;
    Ok(())
}

fn remove(s: &str) -> Result<()> {
    Command::new("cargo")
        .arg("uninstall")
        .arg(s)
        .spawn()?
        .wait()?;
    Ok(())
}

fn look_for_installed() -> Result<Vec<(Name, Version)>> {
    let installed_bins_toml = fs::read_to_string(
        dirs::home_dir()
            .ok_or("Cannot read home dir location")?
            .join(".cargo")
            .join(".crates.toml"),
    )?;

    let table: toml::map::Map<String, toml::Value> = toml::from_str(&installed_bins_toml.trim())?;

    if table.is_empty() {
        return Ok(vec![]);
    }

    if let toml::Value::Table(table) = &table["v1"] {
        Ok(table
            .keys()
            .filter_map(|k| {
                let mut key = k.split_whitespace();
                Some((key.next()?.to_string(), key.next()?.to_string()))
            })
            .collect())
    } else {
        Ok(vec![])
    }
}

struct Progress {
    width: usize,
    current: usize,
    step: usize,
    printer: StandardStream,
}

impl Progress {
    fn new(max: usize) -> Result<Self> {
        let mut printer = StandardStream::stdout(ColorChoice::Auto);
        printer.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;

        let width = max / 2;
        let step = max.checked_div(width).unwrap_or(width);
        let current = 0;

        Ok(Self {
            width,
            step,
            current,
            printer,
        })
    }

    fn advance(&mut self) {
        self.current += 1;
    }

    fn print(&mut self) -> Result<()> {
        let progress = self.current.checked_div(self.step).unwrap_or(0);
        let remaining = match self.width.checked_sub(progress) {
            Some(n) => n,
            None => return Ok(()),
        };
        let progress: String = std::iter::repeat('#').take(progress).collect();
        let remaining: String = std::iter::repeat(' ').take(remaining).collect();

        write!(&mut self.printer, "\r")?;
        write!(&mut self.printer, "\t\t[{}{}]", progress, remaining)?;
        self.printer.flush()?;
        Ok(())
    }
}
