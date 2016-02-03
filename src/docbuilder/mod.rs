
pub mod crte;

use std::io::prelude::*;
use std::io;
use std::fmt;
use std::env;
use std::path::PathBuf;
use std::fs;
use std::process::{Command, Output};

use toml;
use regex::Regex;


pub struct DocBuilder {
    keep_build_directory: bool,
    destination: PathBuf,
    chroot_path: PathBuf,
    chroot_user: String,
    build_dir: PathBuf,
    crates_io_index_path: PathBuf,
    logs_path: PathBuf,
    skip_if_exists: bool,
    skip_if_log_exists: bool,
    build_only_latest_version: bool,
    debug: bool,
}


#[derive(Debug)]
pub enum DocBuilderError {
    DownloadCrateError(String),
    ExtractCrateError(String),
    BuildDocForCratePath(io::Error),
    LogFileError(io::Error),
    RemoveBuildDir(io::Error),
    RemoveCrateFile(io::Error),
    RemoveOldDoc(io::Error),
    SkipLogFileExists,
    SkipDocumentationExists,
    HandleLocalDependenciesError,
    LocalDependencyDownloadError(String),
    LocalDependencyExtractCrateError(String),
    LocalDependencyDownloadDirNotExist,
    LocalDependencyIoError(io::Error),
    FailedToBuildCrate,

    CopyDocumentationCargoTomlNotFound(io::Error),
    CopyDocumentationLibNameNotFound,
    DocumentationNotFound,
    CopyDocumentationIoError(io::Error),
}


// This error only occurs if check_dirs fails
pub enum DocBuilderPathError {
    DestinationPathNotExists,
    ChrootPathNotExists,
    BuildDirectoryNotExists,
    CratesIoIndexPathNotExists,
    LogsPathNotExists,
}


impl fmt::Debug for DocBuilderPathError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            DocBuilderPathError::DestinationPathNotExists =>
                write!(f, "Destination path not exists"),
            DocBuilderPathError::ChrootPathNotExists =>
                write!(f, "Chroot path not exists"),
            DocBuilderPathError::BuildDirectoryNotExists =>
                write!(f, "Build directory path not exists"),
            DocBuilderPathError::CratesIoIndexPathNotExists =>
                write!(f, "crates.io-index path not exists"),
            DocBuilderPathError::LogsPathNotExists =>
                write!(f, "Logs path not exists"),
        }
    }
}


impl Default for DocBuilder {
    fn default() -> DocBuilder {

        let cwd = env::current_dir().unwrap();

        let (destination, chroot_path, build_dir, crates_io_index_path, logs_path) =
            generate_paths(cwd);

        DocBuilder {
            destination: destination,
            chroot_path: chroot_path,
            build_dir: build_dir,
            crates_io_index_path: crates_io_index_path,
            logs_path: logs_path,

            chroot_user: "onur".to_string(),

            keep_build_directory: false,
            skip_if_exists: false,
            skip_if_log_exists: false,
            build_only_latest_version: false,
            debug: false,
        }
    }
}


impl fmt::Debug for DocBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               "DocBuilder {{ destination: {:?}, chroot_path: {:?}, chroot_user_home_dir: {:?}, \
               crates_io_index_path: {:?}, logs_path: {:?}, chroot_user: {:?}, \
                keep_build_directory: {:?}, skip_if_exists: {:?}, skip_if_log_exists: {:?}, debug: \
                {:?} }}",
                self.destination,
                self.chroot_path,
                self.build_dir,
                self.crates_io_index_path,
                self.logs_path,
                self.chroot_user,
                self.keep_build_directory,
                self.skip_if_exists,
                self.skip_if_log_exists,
                self.debug)
    }
}


impl DocBuilder {

    /// Creates new DocBuilder from prefix
    pub fn from_prefix(prefix: PathBuf) -> DocBuilder {

        let (destination, chroot_path, build_dir, crates_io_index_path, logs_path) =
            generate_paths(prefix);

        DocBuilder {
            destination: destination,
            chroot_path: chroot_path,
            build_dir: build_dir,
            crates_io_index_path: crates_io_index_path,
            logs_path: logs_path,

            .. Default::default()
        }

    }

    /// Set destination
    pub fn destination(&mut self, path: PathBuf) {
        self.destination = path;
    }

    /// Set chroot path
    pub fn chroot_path(&mut self, path: PathBuf) {
        self.chroot_path = path;
        self.build_dir_path();
    }

    /// Set build dir
    pub fn build_dir_path(&mut self) {
        let mut build_dir_path = PathBuf::from(&self.chroot_path);
        build_dir_path.push("home");
        build_dir_path.push(&self.chroot_user);
        self.build_dir = build_dir_path;
    }

    /// Set chroot user name
    pub fn chroot_user(&mut self, user: String) {
        self.chroot_user = user;
        self.build_dir_path();
    }

    /// Set crates.io-index path
    pub fn crates_io_index_path(&mut self, path: PathBuf) {
        self.crates_io_index_path = path;
    }

    /// Set logs path
    pub fn logs_path(&mut self, path: PathBuf) {
        self.logs_path = path;
    }

    pub fn keep_build_directory(&mut self, b: bool) {
        self.keep_build_directory = b;
    }

    pub fn skip_if_exists(&mut self, b: bool) {
        self.skip_if_exists = b;
    }

    pub fn skip_if_log_exists(&mut self, b: bool) {
        self.skip_if_log_exists = b;
    }

    pub fn build_only_latest_version(&mut self, b: bool) {
        self.build_only_latest_version = b;
    }


    pub fn check_paths(&self) -> Result<(), DocBuilderPathError> {
        if !self.destination.exists() {
            return Err(DocBuilderPathError::DestinationPathNotExists)
        }
        if !self.chroot_path.exists() {
            return Err(DocBuilderPathError::ChrootPathNotExists)
        }
        if !self.build_dir.exists() {
            return Err(DocBuilderPathError::BuildDirectoryNotExists)
        }
        if !self.crates_io_index_path.exists() {
            return Err(DocBuilderPathError::CratesIoIndexPathNotExists)
        }
        if !self.crates_io_index_path.exists() {
            return Err(DocBuilderPathError::LogsPathNotExists)
        }
        Ok(())
    }


    pub fn check_crates_io_index_path(&self) -> Result<(), DocBuilderPathError> {
        if !self.crates_io_index_path.exists() {
            return Err(DocBuilderPathError::CratesIoIndexPathNotExists)
        }
        Ok(())
    }

    /// This functions reads files in crates.io-index and tries to build
    /// documentation for crates.
    pub fn build_doc_for_every_crate(&self) -> Result<(), DocBuilderError> {
        self.build_doc_for_crate_path(&self.crates_io_index_path)
    }


    fn build_doc_for_crate_path(&self, path: &PathBuf) -> Result<(), DocBuilderError> {
        for dir in try!(path.read_dir().map_err(DocBuilderError::BuildDocForCratePath)) {

            let path = dir.unwrap().path();

            // skip files under .git and config.json
            if path.to_str().unwrap().contains(".git") ||
                path.file_name().unwrap() == "config.json" {
                    continue;
                }

            if path.is_dir() {
                try!(self.build_doc_for_crate_path(&path));
                continue;
            }

            if let Ok(crte) = crte::Crate::from_cargo_index_file(path) {
                self.build_doc_for_crate(&crte);
            }
        }

        Ok(())
    }


    /// Builds documentation for crate
    ///
    /// This function will try to build documentation for every version of crate
    pub fn build_doc_for_crate(&self, crte: &crte::Crate) {
        for i in 0..crte.versions.len() {
            if let Err(e) = self.build_doc_for_crate_version(crte, i) {
                println!("Failed to build docs for crate {}-{}: {:#?}",
                         &crte.name, &crte.versions[i], e)
            }

            // if self.build_only_latest_version is true
            // we are skipping oldest versions of crate
            if self.build_only_latest_version {
                break;
            }
        }
    }


    fn open_log_for_crate(&self,
                          crte: &crte::Crate,
                          version_index: usize) -> Result<fs::File, DocBuilderError> {
        let mut log_path = PathBuf::from(&self.logs_path);
        log_path.push(&crte.name);

        if !log_path.exists() {
            try!(fs::create_dir_all(&log_path).map_err(DocBuilderError::LogFileError));
        }

        log_path.push(format!("{}-{}.log",
                              &crte.name,
                              &crte.versions[version_index]));

        if self.skip_if_log_exists && log_path.exists() {
            return Err(DocBuilderError::SkipLogFileExists);
        }

        fs::OpenOptions::new().write(true).create(true)
            .open(log_path).map_err(DocBuilderError::LogFileError)
    }



    /// Returns package folder inside build directory
    fn crate_root_dir(&self, crte: &crte::Crate, version_index: usize) -> PathBuf {
        let mut package_root = PathBuf::from(&self.build_dir);
        package_root.push(crte.canonical_name(version_index));
        package_root
    }


    fn remove_old_doc(&self,
                      crte: &crte::Crate,
                      version_index: usize) -> Result<(), DocBuilderError> {
        let mut path = PathBuf::from(&self.destination);
        path.push(format!("{}/{}", crte.name, crte.versions[version_index]));

        if path.exists() {
            try!(fs::remove_dir_all(path).map_err(DocBuilderError::RemoveOldDoc));
        }

        Ok(())
    }


    /// Checks Cargo.toml for [lib] and return name of lib.
    fn find_lib_name(&self, root_dir: &PathBuf) -> Result<String, DocBuilderError> {

        let mut cargo_toml_path = PathBuf::from(&root_dir);
        cargo_toml_path.push("Cargo.toml");

        let mut cargo_toml_fh = try!(fs::File::open(cargo_toml_path)
                                     .map_err(DocBuilderError::LocalDependencyIoError));
        let mut cargo_toml_content = String::new();
        try!(cargo_toml_fh.read_to_string(&mut cargo_toml_content)
             .map_err(DocBuilderError::CopyDocumentationCargoTomlNotFound));

        toml::Parser::new(&cargo_toml_content[..]).parse().as_ref()
            .and_then(|cargo_toml| cargo_toml.get("lib"))
            .and_then(|lib| lib.as_table())
            .and_then(|lib_table| lib_table.get("name"))
            .and_then(|lib_name| lib_name.as_str())
            .and_then(|lib_name| Some(String::from(lib_name)))
            .ok_or(DocBuilderError::HandleLocalDependenciesError)
    }


    /// Returns Err(DocBuilderError::SkipDocumentationExists) if self.skip_is_exists true and
    /// documentation is already exists at destination path.
    fn is_crate_doc_exists(&self,
                           crte: &crte::Crate,
                           version_index: usize) -> Result<(), DocBuilderError> {
        // do not skip unless it's requested
        if !self.skip_if_exists {
            return Ok(())
        }

        let mut destination = PathBuf::from(&self.destination);
        destination.push(format!("{}/{}", &crte.name, &crte.versions[version_index]));
        if destination.exists() {
            return Err(DocBuilderError::SkipDocumentationExists)
        }

        Ok(())
    }


    fn find_doc(&self,
                crte: &crte::Crate,
                version_index: usize) -> Result<(PathBuf, PathBuf), DocBuilderError> {
        let mut path = self.crate_root_dir(crte, version_index);

        // get src directory
        let mut src_path = self.crate_root_dir(crte, version_index);
        src_path.push("target/doc/src");

        // if [lib] name exist in Cargo.toml check this directory
        // documentation will be inside this directory
        if let Ok(lib_path) = self.find_lib_name(&path) {
            let mut lib_full_path = PathBuf::from(&path);
            lib_full_path.push(format!("target/doc/{}", lib_path));
            let mut src_full_path = PathBuf::from(&path);
            src_full_path.push(format!("target/doc/src/{}", lib_path));
            if lib_full_path.exists() && src_full_path.exists() {
                return Ok((lib_full_path, src_full_path));
            }
        }

        // start looking into target/doc
        path.push("target/doc");

        // check crate name
        let mut crate_path = PathBuf::from(&path);
        crate_path.push(&crte.name);
        src_path.push(&crte.name);
        if crate_path.exists() && src_path.exists() {
            return Ok((crate_path, src_path));
        }

        // some crates are using '-' in their name but actual name contains '_'
        let actual_crate_name = &crte.name.replace("-", "_");
        // I need another fresh src_path here
        // FIXME: This function became a mess after I introduced src_path
        let mut src_path = self.crate_root_dir(crte, version_index);
        src_path.push("target/doc/src");
        // I think it's safe to push into path now
        path.push(actual_crate_name);
        src_path.push(actual_crate_name);
        if path.exists() && src_path.exists() {
            return Ok((path, src_path));
        }

        Err(DocBuilderError::DocumentationNotFound)
    }


    /// Builds documentation for crate
    ///
    /// This operation involves following process:
    ///
    /// * Cleaning up build directory
    /// * Downloading crate
    /// * Extracting it into build directory (chroot dir home directory)
    /// * Building crate documentation with chroot
    /// * Checking build directory for if crate actually has any documentation
    /// * Copying crate documentation into destination path
    /// * Cleaning up build directory
    /// * Removing downloaded crate file
    pub fn build_doc_for_crate_version(&self,
                                       crte: &crte::Crate,
                                       version_index: usize) -> Result<(), DocBuilderError> {
        try!(self.is_crate_doc_exists(&crte, version_index));

        // TODO try to replace noob style logging
        let mut log_file = try!(self.open_log_for_crate(&crte, version_index));

        println!("Building documentation for {}-{}", crte.name, crte.versions[version_index]);

        // build docs
        try!(writeln!(log_file, "Building documentation")
             .map_err(DocBuilderError::LogFileError));
        let (status, message) = match self.build_doc_in_chroot(&crte, version_index) {
            Ok(m) => (true, m),
            Err(m) => (false, m),
        };
        try!(writeln!(log_file, "{}", message)
             .map_err(DocBuilderError::LogFileError));

        if status {
            // copy docs
            try!(self.copy_doc(&crte, version_index));
            Ok(())
        } else {
            Err(DocBuilderError::FailedToBuildCrate)
        }
    }


    fn copy_doc(&self, crte: &crte::Crate, version_index: usize) -> Result<(), DocBuilderError> {

        // remove old documentation just in case
        try!(self.remove_old_doc(&crte, version_index));

        let (doc_path, src_path) = try!(self.find_doc(&crte, version_index));

        // copy documentation to destination/crate/version
        let mut destination = PathBuf::from(&self.destination);
        destination.push(format!("{}/{}", &crte.name, &crte.versions[version_index]));
        try!(copy_files(&doc_path, &destination, true));

        // copy search-index.js
        let mut search_index_js_path = self.crate_root_dir(&crte, version_index);
        search_index_js_path.push("target/doc/search-index.js");
        let mut search_index_js_destination_path = PathBuf::from(&destination);
        search_index_js_destination_path.push("search-index.js");
        if search_index_js_path.exists() {
            try!(fs::copy(search_index_js_path, search_index_js_destination_path)
                 .map_err(DocBuilderError::CopyDocumentationIoError));
        }

        // copy source to destination/crate/version/src
        destination.push("src");
        try!(copy_files(&src_path, &destination, true));

        Ok(())
    }


    /// Build documentation of a crate in chroot environment
    fn build_doc_in_chroot(&self,
                           crte: &crte::Crate,
                           version_index: usize) -> Result<String, String> {
        command_result(Command::new("sudo")
                       .arg("chroot")
                       .arg(&self.chroot_path)
                       .arg("su").arg("-").arg(&self.chroot_user)
                       .arg("-c")
                       .arg(format!("crate-builder -c {} {}",
                                    &crte.name, &crte.versions[version_index]))
                       .output()
                       .unwrap())
    }

}



/// a simple function to capture command output
pub fn command_result(output: Output) -> Result<String, String> {
    let mut command_out = String::from_utf8_lossy(&output.stdout).into_owned();
    command_out.push_str(&String::from_utf8_lossy(&output.stderr).into_owned()[..]);
    match output.status.success() {
        true => Ok(command_out),
        false => Err(command_out)
    }
}



/// A simple function to copy files from source to destination
pub fn copy_files(source: &PathBuf,
              destination: &PathBuf,
              handle_html: bool) -> Result<(), DocBuilderError> {

    // Make sure destination directory is exists
    if !destination.exists() {
        try!(fs::create_dir_all(&destination)
             .map_err(DocBuilderError::LocalDependencyIoError));
    }

    for file in try!(source.read_dir().map_err(DocBuilderError::LocalDependencyIoError)) {

        let file = try!(file.map_err(DocBuilderError::LocalDependencyIoError));
        let mut destination_full_path = PathBuf::from(&destination);
        destination_full_path.push(file.file_name());

        let metadata = try!(file.metadata().map_err(DocBuilderError::LocalDependencyIoError));

        if metadata.is_dir() {
            try!(fs::create_dir_all(&destination_full_path)
                 .map_err(DocBuilderError::LocalDependencyIoError));
            try!(copy_files(&file.path(), &destination_full_path, handle_html));
        } else if handle_html && file.file_name().into_string().unwrap().ends_with(".html") {
            try!(copy_html(&file.path(), &destination_full_path));
        } else {
            try!(fs::copy(&file.path(), &destination_full_path)
                 .map_err(DocBuilderError::LocalDependencyIoError));
        }

    }
    Ok(())
}


fn copy_html(source: &PathBuf, destination: &PathBuf) -> Result<(), DocBuilderError> {

    let source_file = try!(fs::File::open(source)
                           .map_err(DocBuilderError::CopyDocumentationIoError));
    let mut destination_file = try!(fs::OpenOptions::new()
                                    .write(true).create(true).open(destination)
                                    .map_err(DocBuilderError::CopyDocumentationIoError));

    let reader = io::BufReader::new(source_file);

    for line in reader.lines() {
        let mut line = try!(line.map_err(DocBuilderError::CopyDocumentationIoError));

        // replace css links
        if Regex::new(r#"href=".*\.css""#).unwrap().is_match(&line[..]) {
            line = Regex::new(r#"href="(.*\.css)""#).unwrap()
                .replace_all(&line[..], "href=\"../$1\"");
        }

        // replace search-index.js links
        else if Regex::new(r#"<script.*src=".*search-index\.js""#).unwrap().is_match(&line[..]) {
            line = Regex::new(r#"src="\.\./(.*\.js)""#).unwrap()
                .replace_all(&line[..], "src=\"$1\"");
        }

        // replace javascript library links
        else if Regex::new(r#"<script.*src=".*(jquery|main|playpen)\.js""#).unwrap()
            .is_match(&line[..]) {
            line = Regex::new(r#"src="(.*\.js)""#).unwrap()
                .replace_all(&line[..], "src=\"../$1\"");
        }

        // replace source file links
        // we are placing target/doc/src into destinaton/crate/version/src
        else if Regex::new(r#"href='.*?\.\./src"#).unwrap().is_match(&line[..]) {
            line = Regex::new(r#"href='(.*?)\.\./src/[\w-]+/"#).unwrap()
                .replace_all(&line[..], "href='$1src/");
        }

        // FIXME: I actually forgot what I was replacing here. Probably crate links
        else if Regex::new(r#"href='.*?\.\./[\w_-]+"#).unwrap().is_match(&line[..]) {
            line = Regex::new(r#"href='(.*?)\.\./[\w_-]+/"#).unwrap()
                .replace_all(&line[..], "href='$1");
        }

        // replace window.rootPath
        else if Regex::new(r#"window.rootPath = "(.*?)\.\./"#).unwrap().is_match(&line[..]) {
            line = Regex::new(r#"window.rootPath = "(.*?)\.\./"#).unwrap()
                .replace_all(&line[..], "window.rootPath = \"$1");
        }

        try!(destination_file.write(line.as_bytes())
             .map_err(DocBuilderError::CopyDocumentationIoError));
        // need to write consumed newline
        try!(destination_file.write(&['\n' as u8])
             .map_err(DocBuilderError::CopyDocumentationIoError));
    }

    Ok(())
}


fn generate_paths(prefix: PathBuf) -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {

    let mut destination = PathBuf::from(&prefix);
    destination.push("public_html/crates");

    let mut chroot_path = PathBuf::from(&prefix);
    chroot_path.push("chroot");

    let mut build_dir = PathBuf::from(&prefix);
    build_dir.push(&chroot_path);
    build_dir.push("home/onur");

    let mut crates_io_index_path = PathBuf::from(&prefix);
    crates_io_index_path.push("crates.io-index");

    let mut logs_path = PathBuf::from(&prefix);
    logs_path.push("logs");

    (destination, chroot_path, build_dir, crates_io_index_path, logs_path)
}
