use super::project::Project;
use colored::Colorize;
use log::{error, trace, warn};
use regex::Regex;
use std::{collections::HashSet, fs, path, process, str};

struct BuildTarget {
    path: path::PathBuf,
    top_module: Option<String>, // Top module found in test (or the default top module name if None)
}

pub struct Builder {
    modules: HashSet<path::PathBuf>,
    tests: Vec<BuildTarget>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            modules: HashSet::<_>::new(),
            tests: Vec::<_>::new(),
        }
    }

    pub fn find_dependencies(
        _project: &Project,
        build: Builder,
    ) -> Result<Builder, Box<dyn std::error::Error>> {
        Ok(build)
    }

    pub fn find_modules(
        project: &Project,
        builder: Builder,
    ) -> Result<Builder, Box<dyn std::error::Error>> {
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
                let submodules: HashSet<path::PathBuf> = fs::read_to_string(mod_dot_bsv)?
                    .lines()
                    .into_iter()
                    // map from &str -> Option<Capture> matching the regex
                    .flat_map(|line| re.captures(line))
                    // Map from capture to the local module path
                    .map(|capture| current_module_path.join(&capture[1]))
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
            let top_modules: Vec<String> = contents
                .lines()
                .into_iter()
                .flat_map(|line| re.captures(line))
                .map(|capture| capture[1].to_string())
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

    pub fn find_tests(
        project: &Project,
        builder: Builder,
    ) -> Result<Builder, Box<dyn std::error::Error>> {
        let mut builder = builder;
        let re = Regex::new(r"//!topmodule\s+(\w*)\s*")?;

        builder.tests = project
            .root_path()
            .join("tests")
            // read all files in the "tests" directory
            .read_dir()?
            // filter out any Err variants
            .filter(|dir_entry| dir_entry.is_ok())
            // unwrap the paths inside the Ok variants (safe since Err variants were previously rejected)
            .map(|dir_entry| dir_entry.unwrap().path())
            // Filter out any paths that don't end in ".bsv"
            .filter(|path| {
                if let Some(extension) = path.extension() {
                    if extension == "bsv" {
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            // Change from PathBuf to BuildTarget
            .map(|path_buf| {
                // See if any top modules are defined in the file
                let top_module: Option<String> = Self::find_top_module(&re, &path_buf);

                BuildTarget {
                    path: path_buf,
                    top_module,
                }
            })
            .inspect(|test_definition| trace!("Test found: {:?}", &test_definition.path))
            .collect();

        Ok(builder)
    }

    pub fn run_tests(
        project: &Project,
        builder: Builder,
    ) -> Result<Builder, Box<dyn std::error::Error>> {
        let target_path = project.root_path().join("target");
        let mut error_output: Option<std::process::Output> = None;

        //
        // For each test...
        //
        for test in &builder.tests {
            // Create the path object inside the target directory that matches the test path stem.
            let test_build_path = target_path.join(test.path.file_stem().unwrap());

            // Create the test build path if necessary.
            if !test_build_path.exists() {
                fs::create_dir_all(&test_build_path)?;
            }

            // Get the top module name from the test (or the default, if one wasn't found)
            let top_module = test.top_module.clone().unwrap_or("mkTopModule".to_string());

            trace!(
                "Top module for {}: {}",
                test.path.to_string_lossy(),
                top_module
            );

            // Module path creation
            let mut module_path_string: std::ffi::OsString = "%/Libraries:".into();
            let colon: std::ffi::OsString = ":".into();
            for module in &builder.modules {
                module_path_string.push(&colon);
                module_path_string.push(module.as_os_str());
            }

            trace!(
                "Module path string: {}",
                &module_path_string.clone().into_string().unwrap()
            );

            // Compile
            let cmd = process::Command::new("bsc")
                // output directory for .bo and .ba files
                .arg("-bdir")
                .arg(&test_build_path)
                // specify paths to modules/sources
                .arg("-p")
                .arg(module_path_string)
                // compile BSV generating Bluesim object
                .arg("-sim")
                // check and recompile packages that are not up to date
                .arg("-u")
                // Specify a module to elaborate
                .arg("-g")
                .arg(&top_module)
                // Sshhhh
                .arg("-quiet")
                // Check assertions
                .arg("-check-assert")
                //                .arg("-print-flags")
                // The source file
                .arg(&test.path)
                .spawn()?;

            trace!("Compile current dir: {:?}", test_build_path.as_path());
            trace!("Compile source: {:?}", &test.path);

            let output = cmd.wait_with_output()?;
            if !output.status.success() {
                error!(
                    "Compile failed {}",
                    std::str::from_utf8(output.stdout.as_slice()).unwrap()
                );
                error_output = Some(output);
                break;
            }

            // Link
            let mut base_cmd = process::Command::new("bsc");
            let output_file = test_build_path.join(test.path.as_path().file_stem().unwrap());

            let cmd = base_cmd
                // output directory for .bo and .ba files
                .arg("-bdir")
                .arg(&test_build_path)
                // working directory for relative file paths during elaboration
                .arg("-fdir")
                .arg(&test_build_path)
                // output directory for informational files
                .arg("-info-dir")
                .arg(&test_build_path)
                // output directory for Bluesim intermediate files
                .arg("-simdir")
                .arg(&test_build_path)
                // compile BSV generating Bluesim object
                .arg("-sim")
                // check and recompile packages that are not up to date
                .arg("-u")
                .arg("-e")
                .arg(top_module)
                // name the resulting executable
                .arg("-o")
                .arg(output_file)
                // Sshhhh
                .arg("-quiet");

            // Remove C++ warnings on Mac related to deprecated function usage (e.g. sprintf)
            #[cfg(any(unix))]
            let cmd = cmd.arg("-Xc++").arg("-Wno-deprecated-declarations");

            let child = cmd.spawn()?;

            trace!("Linking: {:?}", &test.path);

            let output = child.wait_with_output()?;
            if !output.status.success() {
                error!(
                    "Link failed: {}",
                    std::str::from_utf8(output.stdout.as_slice()).unwrap()
                );
                error_output = Some(output);
                break;
            }

            // Run test
            let test_executable = test.path.as_path().file_stem().unwrap();
            trace!("Testing: {:?}", &test_executable);
            let output = if cfg!(target_os = "windows") {
                std::process::Command::new("cmd")
                    .arg("/C")
                    .arg(test_executable)
                    .current_dir(test_build_path.as_path())
                    .output()?
            } else {
                let executable = format!("./{}", test_executable.to_str().unwrap());

                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(executable)
                    .current_dir(test_build_path.as_path())
                    .output()?
            };

            if !output.status.success() {
                error!(
                    "Test failed: {}",
                    std::str::from_utf8(output.stdout.as_slice()).unwrap()
                );
                error_output = Some(output);
                break;
            } else {
                // Search stdout for ">>>PASS" to see if the test succeeded.
                let stdout = str::from_utf8(output.stdout.as_slice())?;
                if stdout.contains(">>>PASS") {
                    println!(
                        "Test: {} -- {}.",
                        test.path.file_stem().unwrap().to_string_lossy(),
                        "PASSED".green()
                    );
                } else {
                    println!(
                        "Test: {} -- {}.",
                        test.path.file_stem().unwrap().to_string_lossy(),
                        "FAILED".red().bold()
                    );
                    error_output = Some(output);
                    break;
                }
            }
        }

        if error_output.is_some() {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "bsc child process returned non-zero",
            )))
        } else {
            Ok(builder)
        }
    }
}
