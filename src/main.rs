use colored::*;
use rayon::prelude::*;
use std::fs;
use std::io;
use std::process::Command;
use std::sync::{Arc, Mutex};

fn main() {
    let s = std::env::args().nth(1).unwrap();
    let hit = Arc::new(Mutex::new(vec![]));
    let res = search(&s)
        .unwrap()
        .lines()
        .map(ToOwned::to_owned)
        .collect::<Vec<String>>();

    let hit_c = hit.clone();
    res.par_iter().for_each(move |line| {
        if line.starts_with("...") {
            return;
        }
        let mut line = line.split('=');
        let n = line.next().unwrap().trim().to_string();
        let (v, d) = parse_version_desc(line.next().unwrap());

        if is_bin(&n, &v) {
            hit_c.lock().unwrap().push((n, v, d));
        }
    });

    main_loop(Arc::try_unwrap(hit).unwrap().into_inner().unwrap());
}

#[cfg(not(test))]
fn search(s: &str) -> io::Result<String> {
    let out = std::process::Command::new("cargo")
        .args(&["search", "--limit", "100", s])
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
    let d = s.next().unwrap();
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

fn main_loop(r: Vec<(String, String, String)>) {
    let installed = look_for_installed(&r);
    let num = r.len();
    for (i, (n, v, d)) in r.iter().enumerate() {
        let suffix = if installed.contains(&n) {
            "(Installed)"
        } else {
            ""
        };

        let v = format!("\"{}\"", v).green();
        let d = format!("#{}", d).purple();

        println!(
            "{} {} = {} {} {}",
            (num - i).to_string().yellow(),
            n.to_string().blue(),
            v,
            suffix.red(),
            d,
        );
    }
    println!(
        "{}",
        "==> Packages to install (eg: 1 2 3, 1-3 or ^4)".cyan()
    );

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    let reqeusted = r.get(num - input.trim_end().parse::<usize>().unwrap());
    if let Some(req) = reqeusted {
        install(&req.0);
    }
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
            if r_names.contains(&&file_name) {
                Some(file_name)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn t() {
    main();
}
