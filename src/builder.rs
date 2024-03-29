use super::project::Project;
use colored::Colorize;
use convert_case::{Case, Casing};
use log::{error, trace, warn};
use regex::Regex;
use std::{collections::HashSet, fs, path, process, str};

struct BuildTarget {
    path: path::PathBuf,
    top_module: Option<String>, // Top module found in test (or the default top module name if None)
    extra_libraries: HashSet<path::PathBuf>,
}

pub struct Builder {
    modules: HashSet<path::PathBuf>,
    unit_tests: Vec<BuildTarget>,
    tests: Vec<BuildTarget>,
    top_modules: Vec<String>,

    extra_libraries: HashSet<path::PathBuf>,

    all_tests_passed: bool,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            modules: HashSet::<_>::new(),
            unit_tests: Vec::<_>::new(),
            tests: Vec::<_>::new(),
            top_modules: Vec::<_>::new(),
            extra_libraries: HashSet::<_>::new(),
            all_tests_passed: false,
        }
    }

    #[cfg(test)]
    pub fn unit_test_count(&self) -> usize {
        self.unit_tests.len()
    }

    #[cfg(test)]
    pub fn test_count(&self) -> usize {
        self.tests.len()
    }

    #[cfg(test)]
    pub fn top_module_count(&self) -> usize {
        self.top_modules.len()
    }

    pub fn all_tests_passed(&self) -> bool {
        self.all_tests_passed
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
        let extra_library_re = Regex::new(r"//!extra_library\s+(\S*)\s*")?;

        let mut remaining_paths = Vec::<path::PathBuf>::new();
        remaining_paths.push(project.root_path().join("src"));

        let mut first_path = true;

        while let Some(current_module_path) = remaining_paths.pop() {
            trace!("Processing module {:?}", &current_module_path);
            builder.modules.insert(current_module_path.clone());

            let submodule_source = {
                if first_path {
                    first_path = false;
                    format!("{}.bsv", project.package.name.to_case(Case::Pascal))
                } else {
                    format!(
                        "{}.bsv",
                        current_module_path
                            .file_stem()
                            .unwrap()
                            .to_string_lossy()
                            .to_case(Case::Pascal)
                    )
                }
            };

            // Check for a <module>.bsv
            let mod_dot_bsv = current_module_path.join(submodule_source);
            if mod_dot_bsv.exists() {
                // Open the file and look for modules that haven't been encountered
                let submodules: HashSet<path::PathBuf> = fs::read_to_string(&mod_dot_bsv)?
                    .lines()
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

                // BUGBUG: combine this with the above so the file isn't being processed twice.
                let extra_libraries: HashSet<path::PathBuf> = fs::read_to_string(&mod_dot_bsv)?
                    .lines()
                    // map from &str -> Option<Capture> matching the regex
                    .flat_map(|line| extra_library_re.captures(line))
                    // Map from capture to the local module path
                    .map(|capture| current_module_path.join(&capture[1]))
                    // Filter out paths that have already been encountered
                    .filter(|module_path| !builder.modules.contains(module_path))
                    // Collect the results
                    .collect();

                for extra_library in extra_libraries {
                    builder
                        .extra_libraries
                        .insert(extra_library.canonicalize()?);
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

    pub fn find_top_modules(
        project: &Project,
        builder: Builder,
    ) -> Result<Builder, Box<dyn std::error::Error>> {
        let re = Regex::new(r"//!topmodule\s+(\w*)\s*")?;
        let mut builder = builder;
        let top_module_path = project.root_path().join("src").join(format!(
            "{}.bsv",
            project.package.name.to_case(Case::Pascal)
        ));

        let contents = fs::read_to_string(top_module_path)?;
        builder.top_modules = contents
            .lines()
            .flat_map(|line| re.captures(line))
            .map(|capture| capture[1].to_string())
            .collect();

        Ok(builder)
    }

    pub fn build_verilog(
        project: &Project,
        builder: Builder,
    ) -> Result<Builder, Box<dyn std::error::Error>> {
        let top_module_path = project.root_path().join("src").join(format!(
            "{}.bsv",
            project.package.name.to_case(Case::Pascal)
        ));
        if builder.top_modules.is_empty() {
            warn!("Warning - no top modules found in {:?}", top_module_path);
        }

        // Module path creation
        let mut module_path_string: std::ffi::OsString = "%/Libraries".into();
        let colon: std::ffi::OsString = ":".into();
        for module in &builder.modules {
            module_path_string.push(&colon);
            module_path_string.push(module.as_os_str());
        }

        let build_root = project.root_path().join("target");

        for top_module in &builder.top_modules {
            let build_target = BuildTarget {
                path: top_module_path.clone(),
                top_module: Some(top_module.clone()),
                extra_libraries: builder.extra_libraries.clone(),
            };

            // Create the path object inside the target directory that matches the test path stem.
            let build_path = build_root.join(top_module);

            // Create the test build path if necessary.
            if !build_path.exists() {
                fs::create_dir_all(&build_path)?;
            }

            trace!("Compile current dir: {:?}", build_path.as_path());
            trace!("Compile source: {:?}", &build_target.path);

            let output = process::Command::new("bsc")
                // output directory for .bo and .ba files
                .arg("-bdir")
                .arg(&build_path)
                // output directory for .v files
                .arg("-vdir")
                .arg(&build_path)
                // specify paths to modules/sources
                .arg("-p")
                .arg(&module_path_string)
                // compile BSV generating Verilog
                .arg("-verilog")
                // check and recompile packages that are not up to date
                .arg("-u")
                // Specify a module to elaborate
                .arg("-g")
                .arg(top_module)
                // Sshhhh
                .arg("-quiet")
                // The source file
                .arg(&build_target.path)
                .output();

            if let Err(e) = output {
                if let std::io::ErrorKind::NotFound = e.kind() {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Unable to locate 'bsc' program.",
                    )));
                } else {
                    println!("ERROR: Attempting to locate 'bsc' failed.");
                    return Err(Box::new(e));
                }
            }

            let output = output.unwrap();
            if !output.status.success() {
                error!(
                    "Compile failed {}",
                    std::str::from_utf8(output.stdout.as_slice()).unwrap()
                );
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Compile failed",
                )));
            }
        }

        Ok(builder)
    }

    pub fn find_tests(
        project: &Project,
        builder: Builder,
    ) -> Result<Builder, Box<dyn std::error::Error>> {
        let mut builder = builder;
        let re = Regex::new(r"//!topmodule\s+(\w*)\s*")?;

        // Find unit tests
        for module in &builder.modules {
            let mut unit_tests: Vec<BuildTarget> = module
                .read_dir()?
                .filter(|dir_entry| dir_entry.is_ok())
                // unwrap the paths inside the Ok variants (safe since Err variants were previously rejected)
                .map(|dir_entry| dir_entry.unwrap().path())
                // Filter out any paths that don't end in ".bsv"
                .filter(|path| {
                    let mut accept = false;
                    if let Some(extension) = path.extension() {
                        if extension == "bsv" {
                            if let Some(stem) = path.file_stem() {
                                if stem.to_string_lossy().ends_with("_tb") {
                                    accept = true
                                }
                            }
                        }
                    }
                    accept
                })
                // Change from PathBuf to BuildTarget
                .map(|path_buf| {
                    // See if any top modules are defined in the file
                    let top_module: Option<String> = Self::find_top_module(&re, &path_buf);

                    BuildTarget {
                        path: path_buf,
                        top_module,
                        extra_libraries: builder.extra_libraries.clone(),
                    }
                })
                .inspect(|test_definition| trace!("Unit Test found: {:?}", &test_definition.path))
                .collect();

            builder.unit_tests.append(&mut unit_tests);
        }

        // Find top level integration tests
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
                    extension == "bsv"
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
                    extra_libraries: builder.extra_libraries.clone(),
                }
            })
            .inspect(|test_definition| trace!("Test found: {:?}", &test_definition.path))
            .collect();

        Ok(builder)
    }

    fn compile_build_target(
        module_path_string: &std::ffi::OsStr,
        build_root: &path::Path,
        target: &BuildTarget,
    ) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        // Create the path object inside the target directory that matches the test path stem.
        let test_build_path = build_root.join(target.path.file_stem().unwrap());

        // Create the test build path if necessary.
        if !test_build_path.exists() {
            fs::create_dir_all(&test_build_path)?;
        }

        // Get the top module name from the test (or the default, if one wasn't found)
        let top_module = target
            .top_module
            .clone()
            .unwrap_or("mkTopModule".to_string());

        trace!(
            "Top module for {}: {}",
            target.path.to_string_lossy(),
            top_module
        );

        // Compile
        let cmd = process::Command::new("bsc")
            // output directory for .bo and .ba files
            .arg("-bdir")
            .arg(&test_build_path)
            // generate schedule file
            .arg("-info-dir")
            .arg(&test_build_path)
            .arg("-show-schedule")
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
            .arg(&target.path)
            .spawn();

        if let Err(e) = cmd {
            if let std::io::ErrorKind::NotFound = e.kind() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Unable to locate 'bsc' program.",
                )));
            } else {
                println!("ERROR: Attempting to locate 'bsc' failed.");
                return Err(Box::new(e));
            }
        }

        let cmd = cmd.unwrap();
        trace!("Compile current dir: {:?}", test_build_path.as_path());
        trace!("Compile source: {:?}", &target.path);

        let output = cmd.wait_with_output()?;
        if output.status.success() {
            trace!("Compilation succeeded: {:?}", &target.path);
            Ok(output)
        } else {
            error!(
                "Compile failed {}",
                std::str::from_utf8(output.stdout.as_slice()).unwrap()
            );
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Compile failed",
            )))
        }
    }

    fn link_build_target(
        build_root: &path::Path,
        target: &BuildTarget,
    ) -> Result<path::PathBuf, Box<dyn std::error::Error>> {
        let test_build_path = build_root.join(target.path.file_stem().unwrap());

        // Get the top module name from the test (or the default, if one wasn't found)
        let top_module = target
            .top_module
            .clone()
            .unwrap_or("mkTopModule".to_string());

        // Determine the name/path of the resulting output file.
        let output_file = test_build_path.join(target.path.as_path().file_stem().unwrap());

        let mut cmd = process::Command::new("bsc");
        let cmd = cmd
            // output directory for .bo and .ba files
            .arg("-bdir")
            .arg(&test_build_path)
            // working directory for relative file paths during elaboration
            .arg("-fdir")
            .arg(&test_build_path)
            // generate schedule file
            .arg("-info-dir")
            .arg(&test_build_path)
            .arg("-show-schedule")
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
            .arg(&output_file)
            // Sshhhh
            .arg("-quiet");

        // Remove C++ warnings on Mac related to deprecated function usage (e.g. sprintf)
        #[cfg(unix)]
        let cmd = cmd.arg("-Xc++").arg("-Wno-deprecated-declarations");

        // Add any extra libraries.
        let cmd = {
            let mut cmd = cmd;
            for extra_library in &target.extra_libraries {
                cmd = cmd.arg(extra_library);
            }

            cmd
        };

        let child = cmd.spawn();

        if let Err(e) = child {
            if let std::io::ErrorKind::NotFound = e.kind() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Unable to locate 'bsc' program.",
                )));
            } else {
                println!("ERROR: Attempting to locate 'bsc' failed.");
                return Err(Box::new(e));
            }
        }

        let child = child.unwrap();
        trace!("Linking: {:?}", &target.path);

        let output = child.wait_with_output()?;
        if output.status.success() {
            trace!("Link succeded: {:?}", &target.path);
            Ok(output_file)
        } else {
            error!(
                "Link failed: {}",
                std::str::from_utf8(output.stdout.as_slice()).unwrap()
            );
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Link failed",
            )))
        }
    }

    fn test_build_target(
        target_executable: &path::Path,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        trace!("Testing: {:?}", &target_executable);
        let output = if cfg!(target_os = "windows") {
            std::process::Command::new("cmd")
                .arg("/C")
                .arg(target_executable)
                .output()?
        } else {
            std::process::Command::new("sh")
                .arg("-c")
                .arg(target_executable)
                .output()?
        };

        if !output.status.success() {
            error!(
                "Test failed: {}",
                std::str::from_utf8(output.stdout.as_slice()).unwrap()
            );
            Ok(false)
        } else {
            // Search stdout for ">>>PASS" to see if the test succeeded.
            let stdout = str::from_utf8(output.stdout.as_slice())?;
            if stdout.contains(">>>PASS") {
                println!(
                    "Test: {} -- {}.",
                    target_executable.file_stem().unwrap().to_string_lossy(),
                    "PASSED".green()
                );
                Ok(true)
            } else {
                println!("{}", stdout);
                println!(
                    "Test: {} -- {}.",
                    target_executable.file_stem().unwrap().to_string_lossy(),
                    "FAILED".red().bold()
                );
                Ok(false)
            }
        }
    }

    fn run_test(
        module_path_string: &std::ffi::OsStr,
        build_root: &path::Path,
        test: &BuildTarget,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        Self::compile_build_target(module_path_string, build_root, test)?;
        let test_executable = Self::link_build_target(build_root, test)?;
        Self::test_build_target(test_executable.as_path())
    }

    pub fn run_tests(
        project: &Project,
        builder: Builder,
    ) -> Result<Builder, Box<dyn std::error::Error>> {
        let mut builder = builder;
        let build_root = project.root_path().join("target");

        // Module path creation
        let mut module_path_string: std::ffi::OsString = "%/Libraries".into();
        let colon: std::ffi::OsString = ":".into();
        for module in &builder.modules {
            module_path_string.push(&colon);
            module_path_string.push(module.as_os_str());
        }

        //
        // For each test
        //
        builder.all_tests_passed = true;
        for test in builder.unit_tests.iter().chain(builder.tests.iter()) {
            let test_passed =
                Self::run_test(module_path_string.as_os_str(), build_root.as_path(), test)?;
            if !test_passed {
                builder.all_tests_passed = false;
                break;
            }
        }

        Ok(builder)
    }
}
