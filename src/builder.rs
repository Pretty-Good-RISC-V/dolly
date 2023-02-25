use log::{error, warn, trace};
use regex::Regex;
use std::{
    fs,
    collections::HashSet,
    path,
    process,
};
use super::project::Project;

struct TestDefinition {
    path: path::PathBuf,
    top_module: Option<String>, // Top module found in test (or the default top module name if None)
}

pub struct Builder {
    modules: HashSet<path::PathBuf>,
    tests: Vec<TestDefinition>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            modules: HashSet::<_>::new(),
            tests: Vec::<_>::new(),
        }
    }

    pub fn find_dependencies(_project: &Project, build: Builder) -> Result<Builder, Box<dyn std::error::Error>> {
        Ok(build)
    }

    pub fn find_modules(project: &Project, builder: Builder) -> Result<Builder, Box<dyn std::error::Error>> {
        let mut builder = builder;
        let re = Regex::new(r"//!submodule\s+(\w*)\s*")?;

        let mut remaining_paths = Vec::<path::PathBuf>::new();
        remaining_paths.push(project.root_path().join("src"));

        while !remaining_paths.is_empty() {
            let current_module_path = remaining_paths.pop().unwrap();

            trace!("Processing module {:?}", &current_module_path);
            builder.modules.insert(current_module_path.clone());

            let mod_dot_bsv = current_module_path.join("mod.bsv");
            if mod_dot_bsv.exists() {
                // Open the file and look for modules that haven't been encountered
                let submodules: HashSet<path::PathBuf> = fs::read_to_string(mod_dot_bsv)?.lines().into_iter()
                    // map from &str -> Option<Capture> matching the regex
                    .flat_map(|line| {
                        re.captures(line)
                    })
                    // Map from capture to the local module path
                    .map(|capture| {
                        current_module_path.join(&capture[1])
                    })
                    // Filter out paths that have already been encountered
                    .filter(|module_path| !builder.modules.contains(module_path))
                    // Collect the results
                    .collect();

                // Add submodules to the array of modules to be processed.
                for submodule in submodules {
                    remaining_paths.push(submodule);
                }
            }
        }

        Ok(builder)
    }

    fn find_top_module(re: &Regex, path: &path::PathBuf) -> Option<String> {
        let mut top_module = None;

        if let Ok(contents) = fs::read_to_string(path) {
            let top_modules: Vec<String> = contents.lines().into_iter()
                .flat_map(|line| {
                    re.captures(line)
                })
                .map(|capture| {
                    capture[1].to_string()
                })
                .collect();

            if !top_modules.is_empty() {
                if top_modules.len() > 1 {
                    warn!("Warning: multiple top modules specified for {:?}", path);
                } else {
                    top_module = Some(top_modules[0].clone());
                }
            }

            top_module
        } else {
            None
        }
    }

    pub fn find_tests(project: &Project, builder: Builder) -> Result<Builder, Box<dyn std::error::Error>> {
        let mut builder = builder;
        let re = Regex::new(r"//!topmodule\s+(\w*)\s*")?;

        builder.tests = project.root_path().join("tests")
            .read_dir()?
            .flat_map(|dir_entry| dir_entry.map(|e| {
                // See if any top modules are defined in the file
                let top_module: Option<String> = Self::find_top_module(&re, &e.path());
                
                TestDefinition {
                    path: e.path(),
                    top_module,
                }
            }))
            .inspect(|test_definition| trace!("Test found: {:?}", &test_definition.path))
            .collect();

        Ok(builder)
    }

    pub fn build_tests(project: &Project, builder: Builder) -> Result<Builder, Box<dyn std::error::Error>> {
        let target_path = project.root_path().join("target");
        let mut error_output: Option<std::process::Output> = None;

        for test in &builder.tests {
            let test_build_path = target_path.join(test.path.file_stem().unwrap());

            // Create the test build path if necessary
            if !test_build_path.exists() {
                fs::create_dir_all(&test_build_path)?;
            }

            let top_module = test.top_module.clone().unwrap_or("mkTopModule".to_string());

            trace!("Top module for {}: {}", test.path.to_string_lossy(), top_module);

            let cmd = process::Command::new("bsc")
                .current_dir(test_build_path.as_path())
                .arg("-sim")
                .arg("-u")
                .arg("-e")
                .arg(top_module)
                .arg(&test.path)
                .spawn()?
                ;

            let output = cmd.wait_with_output()?;
            if !output.status.success() {
                error!("{}", std::str::from_utf8(output.stdout.as_slice()).unwrap());
                error_output = Some(output);
                break;
            }
        }

        if error_output.is_some() {
            Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "bsc child process returned non-zero")))
        } else {
            Ok(builder)
        }
    }
}
