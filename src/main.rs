//! dolly is a tool for building Bluespec SystemVerilog (BSV) projects.
#![warn(missing_docs)]

use convert_case::{Case, Casing};
use clap::{Parser, Subcommand};
use log::{error, trace};
use std::{io::Write,path};

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
        Commands::Init { name } => {
            // first, see if the path exists
            if name.exists() {
                error!("Unable to initialize new project. s{:?} already exists", name);
                Err(Box::new(std::io::Error::from(std::io::ErrorKind::AlreadyExists)))
            } else {
                std::fs::create_dir_all(name.as_path().join("src"))?;
                std::fs::create_dir_all(name.as_path().join("tests"))?;

                let module_name = name.file_stem().unwrap().to_string_lossy().to_case(Case::UpperCamel);

                // Create dolly.toml
                write!(std::fs::File::create(name.as_path().join("dolly.toml"))?, r#"
[package]
name = {:?}
version = "0.1.0"
                "#, module_name)?;

                // Create a simple module
                let filename = format!("src/{}.bsv", module_name);
                write!(std::fs::File::create(name.as_path().join(filename))?, r#"
interface {};
    method Bool isWorking;
endinterface

module mk{}({});
    method Bool isWorking;
        return True;
    endmethod
endmodule
                "#, module_name, module_name, module_name)?;

                // Create a simple test
                let filename = format!("tests/{}_tb.bsv", module_name);
                write!(std::fs::File::create(name.as_path().join(filename))?, r#"
//!topmodule mk{}_tb
import {}::*;

module mk{}_tb(Empty);
    {} my_module <- mk{};

    rule run_it;
        $display(">>>PASS");
        $finish();
    endrule
endmodule
                "#, module_name, module_name, module_name, module_name, module_name)?;

                Ok(())
            }
        },
        Commands::Test { name } => {
            let project = load_project(name.clone())?;

            trace!("Project loaded: {:?}", project);

            Builder::find_dependencies(&project, Builder::new())
                .and_then(|builder| Builder::find_modules(&project, builder))
                .and_then(|builder: Builder| Builder::find_tests(&project, builder))
                .and_then(|builder| Builder::build_tests(&project, builder))
                //.and_then(|builder| Builder::run_tests(&project, builder))
                ?;

            Ok(())
        },
        Commands::Version => {
            print!("{} v{}", NAME, VERSION);
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn simple_dot_bsv() -> Result<(), Box<dyn std::error::Error>> {
        let working_dir = std::env::current_dir().unwrap().join("examples/simple");

        pretty_env_logger::init();
        std::env::set_var("RUST_LOG", "trace");

        let project = load_project(Some(working_dir))?;

        Builder::find_dependencies(&project, Builder::new())
            .and_then(|builder| Builder::find_modules(&project, builder))
            .and_then(|builder: Builder| Builder::find_tests(&project, builder))
            .and_then(|builder| Builder::build_tests(&project, builder))?;

        Ok(())
    }
}
