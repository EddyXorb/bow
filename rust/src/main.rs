mod parser;
mod util;

// src/main.rs
use clap::Parser;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use util::read_yaml_file;
use walkdir::WalkDir;
/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
struct Cli {
    /// Folder to work in
    #[arg(short, long)]
    #[arg(short, long, default_value = "test_wd")]
    path: std::path::PathBuf,
}

const EXPECTED_BOW_FOLDERS: [&str; 6] = [
    "1_imports",
    "1_imports\\bank",
    "2_rules",
    "3_manual",
    "4_output",
    "5_analysis",
];

const CONFIG_FILE_NAME: &str = "config.yml";
// inlay - hints: press CTRL + ALT
struct Main {
    working_dir: PathBuf,
    config: HashMap<String, serde_yaml::Value>,
    config_file: PathBuf,
}

impl Main {
    fn new(working_dir: PathBuf) -> Self {
        let config_file = working_dir.as_path().join(CONFIG_FILE_NAME);
        let config = read_yaml_file(&config_file);

        for folder in EXPECTED_BOW_FOLDERS {
            fs::create_dir_all(working_dir.join(folder)).expect(&format!(
                "Folder {folder} in working directory should be creatable. Check if you have the right permissions."
            ));
        }

        Main {
            working_dir,
            config: config.unwrap_or(HashMap::new()),
            config_file,
        }
    }

    fn run_1_import(&self) {
        for bankfolder in WalkDir::new(self.working_dir.join(EXPECTED_BOW_FOLDERS[1])).min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter(|x| x.is_ok())
            .map(|x| x.unwrap().into_path())
            .filter(|x| x.is_dir())
        {
            print!("{:?}", bankfolder);
            let parser = parser::BowParser::new(&bankfolder);
            parser.parse();
        }
    }
}

fn main() {
    let args = Cli::parse();
    let _main = Main::new(args.path);
    _main.run_1_import();
}
