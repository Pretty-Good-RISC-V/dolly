use log::trace;
use serde::Deserialize;
use std::{fs, path};

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
}
