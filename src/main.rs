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
        .unwrap()
        .block_on(async {
            async_main().await;
        })
}

async fn async_main() {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);

    match parse_args() {
        Action::SearchByName(packages) => match get_from_name(packages).await {
            Ok(_) => (),
            Err(e) => {
                writeln!(&mut stdout, "{}", e).unwrap();
                stdout.reset().unwrap();
                stdout.flush().unwrap();
            }
        },
        Action::FullUpdate => full_update().await.unwrap_or_default(),
        Action::InstallPackage(packages) => install_packages(packages),
        Action::RemovePackage(packages) => remove_packages(packages),
        Action::ShowInstalled => show_installed(),
    }
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

fn show_installed() {
    let installed = look_for_installed();
    let max_width = installed.iter().map(|p| p.0.len()).max().unwrap();

    installed.into_iter().for_each(|p| {
        format!("{:width$}\t", p.0, width = max_width).color_print(Color::Yellow);
        format!("{}\n", p.1).color_print(Color::Red);
    });
}

fn install_packages(packages: Vec<String>) {
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
    format!("Installing pacakges: {}\n", &actual_pkgs).color_print(Color::Blue);
    install(&packages.iter().map(|s| s.as_str()).collect::<Vec<&str>>());
    "Done!".color_print(Color::Blue);
}

fn remove_packages(packages: Vec<String>) {
    format!("Removing pacakges: {:?}\n", &packages).color_print(Color::Blue);
    packages.iter().for_each(|p| remove(p));
    "Done!".color_print(Color::Blue);
}

async fn get_from_name(packages: Vec<String>) -> Result<()> {
    let raw_hits = search(&packages)?;

    if raw_hits.is_empty() {
        return Err("No matches found!".into());
    }

    let hit = Arc::new(Mutex::new(vec![]));
    let progress = Arc::new(Mutex::new(Progress::new(raw_hits.len())));

    let hit_c = hit.clone();

    let client = reqwest::Client::new();
    let f = raw_hits
        .into_iter()
        .map(move |(name, version, description)| {
            let hit_cc = hit_c.clone();
            let progress_c = progress.clone();
            let client = client.clone();

            tokio::spawn(async move {
                if is_bin(client, &name, &version).await.unwrap() {
                    hit_cc.lock().unwrap().push((name, version, description));
                    progress_c.lock().unwrap().advance();
                    progress_c.lock().unwrap().print();
                }
            })
        });

    futures::future::join_all(f).await;

    //new line
    println!();

    main_loop(Arc::try_unwrap(hit).unwrap().into_inner().unwrap())
}

async fn full_update() -> Result<()> {
    let installed = look_for_installed();

    let online_versions = Arc::new(Mutex::new(vec![]));
    let progress = Arc::new(Mutex::new(Progress::new(installed.len())));

    //let online_versions_c = online_versions.clone();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_str("rustman")?,
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();

    let f = installed.clone().into_iter().map(|p| {
        let client = client.clone();
        let online_versions_c = online_versions.clone();
        let progress_c = progress.clone();
        tokio::spawn(async move {
            let p = search_one_pkg(client, &p.0).await;
            if let Ok(Some(p)) = p {
                online_versions_c.lock().unwrap().push(p);
            }
            progress_c.lock().unwrap().advance();
            progress_c.lock().unwrap().print();
        })
    });

    futures::future::join_all(f).await;

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

    if needs_update_pkgs.is_empty() {
        "Everything is uptodate!".color_print(Color::Blue);
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
    .color_print(Color::Cyan);
    println!();

    for package in &needs_update_pkgs {
        format!("{:w1$}\t", package.0, w1 = w1).color_print(Color::Blue);
        format!("{:w2$}\t", package.1, w2 = w2).color_print(Color::Green);
        format!("{:w3$}\t", package.2, w3 = w3).color_print(Color::Yellow);
        println!();
    }

    let update_all = |v: &Vec<(&Name, &Version, &Description)>| {
        v.iter().for_each(|p| install(&[p.0]));
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

fn main_loop(r: Vec<(String, String, String)>) -> Result<()> {
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

    let input = input.trim_end().parse::<usize>()?;

    if num < input {
        return Err("Input is incorrect".into());
    }

    let reqeusted = r.get(num - input);

    if let Some(req) = reqeusted {
        install(&[&req.0]);
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

async fn search_one_pkg(
    client: reqwest::Client,
    s: &str,
) -> Result<Option<(Name, Version, Description)>> {
    const URL: &str = "https://crates.io/api/v1/crates";
    let resp: Resp = client
        .get(&format!("{}/{}", URL, s))
        .send()
        .await?
        .json()
        .await?;

    Ok(Some((
        resp.r_crate.name,
        resp.r_crate.max_version,
        resp.r_crate.description,
    )))
}

fn search(s: &[String]) -> Result<Vec<(Name, Version, Description)>> {
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

async fn is_bin(client: reqwest::Client, n: &str, v: &str) -> Result<bool> {
    let doc = format!("https://docs.rs/crate/{}/{}", n, v);

    let resp = client.get(&doc).send().await?.text().await?;

    Ok(resp.contains("is not a library"))
}

fn install(s: &[&str]) {
    Command::new("cargo")
        .args(&["install", "--force"])
        .args(s)
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}

fn remove(s: &str) {
    Command::new("cargo")
        .arg("uninstall")
        .arg(s)
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
