use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize, Serialize)]
struct Novel {
    pages: Vec<Vec<String>>,
}

impl Novel {
    fn new(pages: Vec<Vec<String>>) -> Novel {
        Novel { pages }
    }
}

fn run() {
    let filename = String::from("test");
    let path = format!("./novels/text/{}.txt", filename);
    let file = fs::read_to_string(path).expect("file read");
    let lines: Vec<String> = file
        .trim()
        .split("\n")
        .map(|line| line.to_string())
        .collect();

    let pages: Vec<Vec<String>> = lines.chunks(20).map(|chunk| chunk.to_vec()).collect();
    let result = serde_json::to_string(&Novel::new(pages));
    match result {
        Ok(data) => {
            println!("{}", data);
            let path = format!("./novels/json/{}.json", filename);
            fs::create_dir_all("./novels/json").expect("could not create the folder");
            fs::write(path, data).expect("Unable to write file")
        }
        Err(err) => println!("{}", err),
    }
}
fn main() {
    run()
}
