mod parser;
mod util;

// src/main.rs
use clap::Parser;
use flexi_logger;
use log::{error, info, Record};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use util::read_yaml_file;
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
    config: HashMap<String, serde_yml::Value>,
    config_file: PathBuf,
}

fn file_log_format(
    write: &mut dyn Write,
    now: &mut flexi_logger::DeferredNow,
    record: &Record,
) -> std::io::Result<()> {
    write!(
        write,
        "{} [{}] - {}",
        now.format("%Y-%m-%d %H:%M:%S"),
        record.level(),
        &record.args()
    )
}

impl Main {
    fn new(working_dir: PathBuf) -> Result<Self, flexi_logger::FlexiLoggerError> {
        init_logging(&working_dir)?;

        let config_file = working_dir.as_path().join(CONFIG_FILE_NAME);
        let config = read_yaml_file(&config_file);

        for folder in EXPECTED_BOW_FOLDERS {
            fs::create_dir_all(working_dir.join(folder)).expect(&format!(
                "Folder {folder} in working directory should be creatable. Check if you have the right permissions."
            ));
        }

        Ok(Main {
            working_dir,
            config: config.unwrap_or(HashMap::new()),
            config_file,
        })
    }

    fn run_1_import(&self) {
        for bankfolder in std::fs::read_dir(self.working_dir.join(EXPECTED_BOW_FOLDERS[1]))
            .expect(
                "There should be a directory {EXPECTED_BOW_FOLDERS[1]} in the working directory.",
            )
            .into_iter()
            .filter(|x| x.is_ok())
            .map(|x| x.unwrap().path())
        {
            info!("{:?}", bankfolder);
            let parser = parser::BowParser::from_folder(&bankfolder);
            match parser.parse() {
                Ok(_) => info!("Parsing successful"),
                Err(e) => error!("Parsing failed: {e}"),
            }
        }
    }
}

fn init_logging(working_dir: &PathBuf) -> Result<(), flexi_logger::FlexiLoggerError> {
    flexi_logger::Logger::try_with_str("info")?
        .log_to_file(
            flexi_logger::FileSpec::default()
                .directory(working_dir)
                .suppress_timestamp(),
        )
        .duplicate_to_stdout(flexi_logger::Duplicate::Info)
        .print_message()
        .append()
        .format_for_files(file_log_format)
        .format_for_stdout(flexi_logger::colored_default_format)
        .start()?;
    info!("Initialized logging of BOW.");
    Ok(())
}

fn main() {
    let args = Cli::parse();
    let _main = Main::new(args.path).unwrap();
    _main.run_1_import();
}
