use std::sync::{Arc, Mutex, mpsc::channel};
use serde::{Serialize, Deserialize};

const crates_url_template: &str = "https://crates.io/api/v1/crates?per_page=100&page=";

struct Database {
    crates: Arc<Mutex<Vec<Crate>>>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Crate {
    name: String,
    version: String,
    description: String,
}

impl Database {
    fn new() -> Self {
    			use std::io::{Read, Write};
    			
    			let cache_dir = dirs::cache_dir().unwrap().join("rustman");
    			std::fs::create_dir_all(&cache_dir);
    	
    			let database_path = cache_dir.join("database.toml");
    			let mut database_file = if std::path::Path::exists(&database_path) {
    				std::fs::File::open(&database_path).unwrap()
    			} else {
    				std::fs::File::create(database_path).unwrap()
    			};
    			let mut database = String::new();
    			database_file.read_to_string(&mut database);

    	
		let crates: Vec<Crate> = toml::from_str(&database).unwrap();
		Self {
			crates: Arc::new(Mutex::new(crates))
		}
    }

    /*fn get_crates(crates: Arc<Mutex<Vec<Crate>>>) -> std::sync::MutexGuard<'static, Vec<Crate>> {
    	match crates.try_lock() {
			Ok(crates) => crates,
			Err(_) => Self::get_crates(crates.clone()),
		}
    }*/

    fn update(&mut self) {
        let mut page_idx = 1;
        let (sender, receiver) = channel();
        let mut threads = vec!();
        
        loop {
            let crates = self.crates.clone();
            let sender = sender.clone();

			if page_idx < 300  {
			threads.push(std::thread::spawn(move||{
								
								                let mut crates_url = crates_url_template.to_string();
								                crates_url.push_str(&page_idx.to_string());
								
								                let crates_url: http_req::uri::Uri = crates_url.parse().unwrap();
								                let mut crate_metadata = Vec::new();
								
								                http_req::request::Request::new(&crates_url)
								                    .header("User-Agent", "https://github.com/sigmaSd/rustman")
								                    .send(&mut crate_metadata)
								                    .unwrap();
								
								                let crates_json = String::from_utf8(crate_metadata).unwrap();
								                //dbg!(&crate_metadata);
								                let crates_json = json::parse(&crates_json).unwrap();

												
												
								                for i in 0..100 {
								                   crates.lock().unwrap().push(
								                        Crate{
								                            name: crates_json["crates"][i]["name"].to_string(),
								                            version: crates_json["crates"][i]["max_version"].to_string(),
								                            description: crates_json["crates"][i]["description"].to_string(),
								                    });
								                }

												dbg!(&crates_json["meta"]["next_page"]);

												//std::process::exit(0);
								                if crates_json["meta"]["next_page"].to_string() == "null" {
								                	
								                    sender.send(()).unwrap();
								                }
								
								                dbg!(&page_idx);

			}));

			}
			if receiver.try_recv().is_ok() {
				break
			}
			page_idx += 1;
        }
        
        threads.into_iter().for_each(|t|t.join().unwrap());
    }

	fn save(&self) {
		use std::io::Write;
		
		let cache_dir = dirs::cache_dir().unwrap().join("rustman");
		std::fs::create_dir_all(&cache_dir);

		let database_path = cache_dir.join("database.toml");
		let mut database_file = std::fs::File::create(database_path).unwrap();

		let database = self.crates.lock().unwrap().clone();
		let database_toml = toml::to_string(&database).unwrap();
		writeln!(database_file, "{}", database_toml);
	}
	
    fn search(&self, name: &str) -> Vec<Crate> {
    	self.crates.lock().unwrap().iter().filter(|c| c.name.contains(name)).map(|c|c.clone()).collect()
    }
}

#[test]
fn database_check() {
    let mut database = Database::new();
    database.update();
    database.save();
}
