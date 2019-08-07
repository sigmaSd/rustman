use crate::progress::Progress;

use serde::{Deserialize, Serialize};
use std::sync::{mpsc::channel, Arc, Mutex};
use std::time::{Duration, SystemTime};
use unchained::Unchained;

const crates_url_template: &str = "https://crates.io/api/v1/crates?per_page=100&page=";

#[derive(Default)]
pub struct Database {
    crates: Arc<Mutex<Vec<Crate>>>,
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
        use std::io::{Read, Write};

        let cache_dir = dirs::cache_dir().unwrap().join("rustman");
        std::fs::create_dir_all(&cache_dir);

        let database_path = cache_dir.join("database.toml");

        match std::fs::File::open(&database_path) {
            Ok(mut database_file) => {
                let mut database = String::new();
                database_file.read_to_string(&mut database);

                let crates: Crates = toml::from_str(&database).unwrap();
                let crates: Vec<Crate> = crates.crates;

                let mut database = Self {
                    crates: Arc::new(Mutex::new(crates)),
                };

                if SystemTime::now()
                    .duration_since(database_file.metadata().unwrap().modified().unwrap())
                    .unwrap()
                    > Duration::new(24 * 60 * 3600, 0)
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

    pub fn update(&mut self) {
        let progress_c = Arc::new(Mutex::new(Progress::new(300)));
        let crates = self.crates.clone();

        (0..300).unchained_for_each(move |page_idx| {
            let mut crates_url = crates_url_template.to_string();
            crates_url.push_str(&page_idx.to_string());

            let crates_url: http_req::uri::Uri = crates_url.parse().unwrap();
            let mut crate_metadata = Vec::new();

            http_req::request::Request::new(&crates_url)
                .header("User-Agent", "https://github.com/sigmaSd/rustman")
                .send(&mut crate_metadata)
                .unwrap();

            let crates_json = String::from_utf8(crate_metadata).unwrap();
            let crates_json = json::parse(&crates_json).unwrap();

            let crates = crates.clone();
            (0..100).unchained_for_each(move |i| {
                let name = crates_json["crates"][i]["name"].to_string();
                let version = crates_json["crates"][i]["max_version"].to_string();
                let description = crates_json["crates"][i]["description"].to_string();

                if !crate::is_bin(&name, &version) {
                    return;
                }

                crates.try_lock().unwrap().push(Crate {
                    name,
                    version,
                    description,
                });
            });

            let mut progress_c = progress_c.try_lock().unwrap();
            progress_c.advance();
            progress_c.print();
        });
    }

    pub fn save(&self) {
        use std::io::Write;

        let cache_dir = dirs::cache_dir().unwrap().join("rustman");
        std::fs::create_dir_all(&cache_dir);

        let database_path = cache_dir.join("database.toml");
        let mut database_file = std::fs::File::create(database_path).unwrap();

        let database = self.crates.lock().unwrap().clone();
        let database = Crates { crates: database };
        let database_toml = toml::to_string(&database).unwrap();
        writeln!(database_file, "{}", database_toml);
    }

    pub fn search(&self, name: &str) -> Vec<Crate> {
        self.crates
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.name.contains(name))
            .map(|c| c.clone())
            .collect()
    }
}

#[test]
fn database_check() {
    let mut database = Database::new();
    database.update();
    database.save();
}
