//! dolly is a tool for building Bluespec SystemVerilog (BSV) projects.
#![warn(missing_docs)]

use clap::{Parser, Subcommand};
use log::{error, trace};
use std::path;

mod builder;
use builder::Builder;

mod project;
use project::Project;

const NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Build { name: Option<path::PathBuf> },
    Clean { name: Option<path::PathBuf> },
    Init { name: path::PathBuf },
    Test { name: Option<path::PathBuf> },
    Version,
}

fn find_project_file(starting_path: path::PathBuf) -> std::io::Result<path::PathBuf> {
    let full_path = starting_path.as_path().canonicalize()?;
    let mut project_filename: std::io::Result<path::PathBuf> =
        Err(std::io::Error::from(std::io::ErrorKind::NotFound));

    for ancestor in full_path.ancestors() {
        let test_path = path::PathBuf::from(ancestor)
            .as_path()
            .canonicalize()?
            .join("dolly.toml");

        trace!("Looking for project: {:?}", test_path);

        if test_path.exists() {
            if test_path.is_file() {
                trace!("Project found: {}", test_path.as_path().to_string_lossy());
                project_filename = Ok(test_path);
                break;
            } else {
                error!("Project path encountered is not a regular file");
                break;
            }
        }
    }

    project_filename
}

fn load_project(
    explicit_search_root: Option<path::PathBuf>,
) -> Result<Project, Box<dyn std::error::Error>> {
    // Determine the path to begin the search.  If one was provided explicitly, use it; otherwise
    // use the current directory.
    let search_root = explicit_search_root.unwrap_or(path::PathBuf::from("."));

    if let Ok(project_file_name) = find_project_file(search_root) {
        trace!("Loading project file...");
        Project::load(project_file_name)
    } else {
        error!("Project file not found");
        Err(Box::new(std::io::Error::from(std::io::ErrorKind::NotFound)))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    let cli = Cli::parse();

    match &cli.command {
        Commands::Build { name } => {
            let project = load_project(name.clone())?;

            trace!("Project loaded: {:?}", project);

            Builder::find_dependencies(&project, Builder::new())
                .and_then(|builder| Builder::find_modules(&project, builder))
                .and_then(|builder: Builder| Builder::find_top_modules(&project, builder))
                .and_then(|builder| Builder::build_verilog(&project, builder))?;

            Ok(())
        },
        Commands::Clean { name } => {
            let project = load_project(name.clone())?;

            project.clean()
        }
        Commands::Init { name } => Project::init(name),
        Commands::Test { name } => {
            let project = load_project(name.clone())?;

            trace!("Project loaded: {:?}", project);

            let builder = Builder::find_dependencies(&project, Builder::new())
                .and_then(|builder| Builder::find_modules(&project, builder))
                .and_then(|builder: Builder| Builder::find_tests(&project, builder))
                .and_then(|builder| Builder::run_tests(&project, builder))?;

            if builder.all_tests_passed() {
                Ok(())
            } else {
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "No all tests passed",
                )))
            }
        }
        Commands::Version => {
            print!("{} v{}", NAME, VERSION);
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();
    
    /// Setup function that is only run once, even if called multiple times.
    fn setup() {
        INIT.call_once(|| {
            pretty_env_logger::init();
        });
    }

    #[test]
    fn simple_dot_bsv_test() -> Result<(), Box<dyn std::error::Error>> {
        setup();
        let working_dir = std::env::current_dir().unwrap().join("examples/simple");

        std::env::set_var("RUST_LOG", "trace");

        let project = load_project(Some(working_dir))?;

        let builder = Builder::find_dependencies(&project, Builder::new())
            .and_then(|builder| Builder::find_modules(&project, builder))
            .and_then(|builder: Builder| Builder::find_tests(&project, builder))
            .and_then(|builder| Builder::run_tests(&project, builder))?;

        assert_eq!(builder.unit_test_count(), 1);
        assert_eq!(builder.test_count(), 1);
        assert_eq!(builder.all_tests_passed(), true);

        Ok(())
    }

    #[test]
    fn simple_dot_bsv_build() -> Result<(), Box<dyn std::error::Error>> {
        setup();
        let working_dir = std::env::current_dir().unwrap().join("examples/simple");

        std::env::set_var("RUST_LOG", "trace");

        let project = load_project(Some(working_dir))?;

        let builder = Builder::find_dependencies(&project, Builder::new())
            .and_then(|builder| Builder::find_modules(&project, builder))
            .and_then(|builder: Builder| Builder::find_top_modules(&project, builder))
            .and_then(|builder| Builder::build_verilog(&project, builder))?;

        assert_eq!(builder.top_module_count(), 1);

        Ok(())
    }
}
