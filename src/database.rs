use crate::progress::Progress;

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use unchained::Unchained;

const CRATES_URL_TEMPLATE: &str = "https://crates.io/api/v1/crates?per_page=100&page=";
const MAX_NET_TRY: usize = 10;

#[derive(Default)]
pub struct Database {
    crates: Arc<Mutex<Vec<Crate>>>,
    pub blacklist: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Crates {
    crates: Vec<Crate>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Crate {
    pub name: String,
    pub version: String,
    pub description: String,
}

impl Database {
    pub fn new() -> Self {
        let cache_dir = dirs::cache_dir().unwrap().join("rustman");
        let _ = std::fs::create_dir_all(&cache_dir);

        let database_path = cache_dir.join("database.toml");

        match std::fs::File::open(&database_path) {
            Ok(mut database_file) => {
                let mut database = String::new();
                database_file.read_to_string(&mut database).unwrap();
                //let crates: Crates = toml::from_str(&database).unwrap();
                let crates = {
                    let mut all_crates = vec![];

                    let crates = database.split("\n\n");
                    for pkg in crates {
                        if pkg.is_empty() {
                            continue;
                        }

                        let mut pkg = pkg.lines().skip(1);
                        let mut name = pkg
                            .next()
                            .unwrap()
                            .split('=')
                            .last()
                            .unwrap()
                            .trim()
                            .to_lowercase();
                        let mut version = pkg
                            .next()
                            .unwrap()
                            .split('=')
                            .last()
                            .unwrap()
                            .trim()
                            .to_owned();
                        let mut description = pkg
                            .next()
                            .unwrap()
                            .split('=')
                            .last()
                            .unwrap()
                            .trim()
                            .to_lowercase();

                        name.remove(0);
                        version.remove(0);
                        description.remove(0);

                        name.pop();
                        version.pop();
                        description.pop();

                        all_crates.push(Crate {
                            name,
                            version,
                            description,
                        });
                    }
                    all_crates
                };

                let blacklist = Self::read_black_list();
                let crates = crates
                    .into_iter()
                    .filter(|c| !blacklist.contains(&c.name))
                    .collect();

                let mut database = Self {
                    crates: Arc::new(Mutex::new(crates)),
                    blacklist,
                };

                if SystemTime::now()
                    .duration_since(database_file.metadata().unwrap().modified().unwrap())
                    .unwrap()
                    > Duration::new(24 * 60 * 60, 0)
                {
                    database.update();
                }

                database
            }
            Err(_) => {
                let mut database = Database::default();
                database.update();
                database.save();

                database
            }
        }
    }

    pub fn read_black_list() -> Vec<String> {
        let cache_dir = dirs::cache_dir().unwrap().join("rustman");
        let _ = std::fs::create_dir_all(&cache_dir);

        let blacklist_path = cache_dir.join("blacklist");
        let blacklist = match std::fs::read_to_string(blacklist_path) {
            Ok(bl) => bl,
            Err(_) => String::new(),
        };

        blacklist.lines().map(ToOwned::to_owned).collect()
    }

    pub fn add_to_blaklist(mut blacklist: Vec<String>, s: &str) {
        blacklist.push(s.to_string());

        let cache_dir = dirs::cache_dir().unwrap().join("rustman");
        let _ = std::fs::create_dir_all(&cache_dir);

        let blacklist_path = cache_dir.join("blacklist");
        let mut blacklist_file = std::fs::File::create(blacklist_path).unwrap();

        writeln!(
            blacklist_file,
            "{}",
            blacklist
                .iter()
                .map(|p| {
                    let mut p = p.to_string();
                    p.push('\n');
                    p
                })
                .collect::<String>()
        )
        .unwrap();
    }
    pub fn update(&mut self) {
        self.crates.lock().unwrap().clear();

        let progress = Arc::new(Mutex::new(Progress::new(300)));
        let crates = self.crates.clone();

        (1..300).unchained_for_each(move |page_idx| {
            let mut crates_url = CRATES_URL_TEMPLATE.to_string();
            crates_url.push_str(&page_idx.to_string());

            let crates_url: http_req::uri::Uri = crates_url.parse().unwrap();
            let mut crate_metadata = Vec::new();

            let mut send_request = || {
                http_req::request::Request::new(&crates_url)
                    .header("User-Agent", "https://github.com/sigmaSd/rustman")
                    .send(&mut crate_metadata)
            };

            let mut counter = 0;
            while let Err(_) = send_request() {
                counter += 1;
                if counter == MAX_NET_TRY {
                    panic!("Network error");
                }
            }

            let crates_json = String::from_utf8(crate_metadata).unwrap();
            let crates_json = json::parse(&crates_json).unwrap();

            let crates = crates.clone();
            (0..100).unchained_for_each(move |i| {
                let name = crates_json["crates"][i]["name"].to_string().to_lowercase();
                let version = crates_json["crates"][i]["max_version"].to_string();
                let description = crates_json["crates"][i]["description"]
                    .to_string()
                    .to_lowercase();

                if version == "null" {
                    return;
                }

                let mut crates = loop {
                    if let Ok(crates) = crates.try_lock() {
                        break crates;
                    }
                };

                crates.push(Crate {
                    name,
                    version,
                    description,
                });
            });

            let mut progress = loop {
                if let Ok(progress) = progress.try_lock() {
                    break progress;
                }
            };

            progress.advance();
            progress.print();
        });
    }

    pub fn save(&self) {
        let cache_dir = dirs::cache_dir().unwrap().join("rustman");
        let _ = std::fs::create_dir_all(&cache_dir);

        let database_path = cache_dir.join("database.toml");
        let mut database_file = std::fs::File::create(database_path).unwrap();

        let database = self.crates.lock().unwrap().clone();
        let database = Crates { crates: database };
        let database_toml = toml::to_string(&database).unwrap();

        writeln!(database_file, "{}", database_toml).unwrap();
    }

    pub fn search(&self, needle: &str) -> Vec<Crate> {
        let needle = needle.to_lowercase();

        self.crates
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.name.contains(&needle) || c.description.contains(&needle))
            .cloned()
            .collect()
    }
}

#[test]
fn database_check() {
    let mut database = Database::new();
    database.update();
    database.save();
}
