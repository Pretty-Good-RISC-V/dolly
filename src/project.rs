use convert_case::{Case, Casing};
use log::{error, trace};
use serde::Deserialize;
use std::{fs, io::Write, path};

#[derive(Debug, Deserialize)]
pub struct Project {
    pub package: Package,

    #[serde(skip)]
    root_path: path::PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
}

impl Project {
    pub fn root_path(&self) -> &path::PathBuf {
        &self.root_path
    }

    pub fn clean(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Ignore any errors from remove_dir_all()
        let _ = fs::remove_dir_all(self.root_path.join("target"));
        Ok(())
    }

    pub fn load(project_file_name: path::PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        trace!("Reading project file...");
        let contents = fs::read_to_string(&project_file_name)?;

        trace!("Parsing project file...");
        let mut project: Project = toml::from_str(&contents)?;

        project.root_path = path::PathBuf::from(
            project_file_name
                .parent()
                .expect("Project path has no parent?  Bug."),
        );

        Ok(project)
    }

    pub fn init(new_project_path: &path::PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        // first, see if the path exists
        if new_project_path.exists() {
            error!(
                "Unable to initialize new project. s{:?} already exists",
                new_project_path
            );
            Err(Box::new(std::io::Error::from(
                std::io::ErrorKind::AlreadyExists,
            )))
        } else {
            std::fs::create_dir_all(new_project_path.as_path().join("src"))?;
            std::fs::create_dir_all(new_project_path.as_path().join("tests"))?;

            let module_name = new_project_path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_case(Case::UpperCamel);

            // Create dolly.toml
            write!(
                std::fs::File::create(new_project_path.as_path().join("dolly.toml"))?,
                r#"[package]
name = {:?}
version = "0.1.0"
"#,
                module_name
            )?;

            // Create simple .gitignore
            write!(
                std::fs::File::create(new_project_path.as_path().join(".gitignore"))?,
                r#"**/target
"#
            )?;

            // Create a simple module
            let filename = format!("src/{}.bsv", module_name);
            write!(
                std::fs::File::create(new_project_path.as_path().join(filename))?,
                r#"interface {};
    method Bool isWorking;
endinterface

module mk{}({});
    method Bool isWorking;
        return True;
    endmethod
endmodule
"#,
                module_name,
                module_name,
                module_name
            )?;

            // Create a simple test
            let filename = format!("tests/{}_tb.bsv", module_name);
            write!(
                std::fs::File::create(new_project_path.as_path().join(filename))?,
                r#"//!topmodule mk{}_tb
import {}::*;

module mk{}_tb(Empty);
    {} my_module <- mk{};

    rule run_it;
        // Required for test to pass.
        $display(">>>PASS");
        $finish();
    endrule
endmodule
"#,
                module_name,
                module_name,
                module_name,
                module_name,
                module_name
            )?;

            Ok(())
        }
    }
}
