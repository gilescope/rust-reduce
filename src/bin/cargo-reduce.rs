use std::path::{Path, PathBuf};
use std::ffi::{OsString};
use std::process::Command;
use clap::clap_app;

use rust_reduce::Runnable;
use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    workspace: Option<WorkspaceConfig>,
    lib: Option<LibConfig>,
    bin: Option<Vec<BinConfig>>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceConfig {
    members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LibConfig {
    name: String,
    path: String,
}

#[derive(Debug, Deserialize)]
struct BinConfig {
    name: String,
    path: String,
}

fn main() {
    let matches = clap_app!(("cargo-reduce") =>
        (version: clap::crate_version!())
        (@arg FIND: * "Text indicating success.")
        (@arg CMD: * "Command to run.")
        (@arg ARGS: * ... "Arguments to the command to run E.g. cargo run / cargo test / cargo build.")
        (after_help: "`cargo-reduce` will try to make the source file smaller by interpreting it as valid Rust code and intelligently removing parts of the code. After each removal, the given command will be run with reduced code.

The original file will be overwritten with the smallest interesting reduced version, if found. This happens while `rust-reduce` is running. The original file will be backed up with the `.orig` suffix. If `rustfmt` is found, it will be used to clean up the output.

The original file may refer to modules in different files, these will be inlined and reduced along with the main file.")
    ).get_matches();
    //TODO recurse?
    let decoded: Config = toml::from_str(&std::fs::read_to_string("Cargo.toml")
        .expect("Cargo.toml file not found"))
        .expect("Can't parse Cargo.toml");
    if let Some(cfg) = decoded.workspace {
        println!("Found workspace: {:?}", &cfg.members);
    }


    //let mut h = std::collections::HashMap::new();
    //h.insert("a", "b");
    //h.insert("c", "d");
    //panic!("does it break");

    let find = matches.value_of_lossy("FIND").expect("string to search for").to_owned();
    let mut cmd = vec![matches.value_of_os("CMD").expect("validated").to_owned()];
    let iter = matches.values_of_os("ARGS").expect("validated").map(ToOwned::to_owned);
    cmd.extend(iter);

    let runnable = Standard::new(cmd, find.to_string(),
                                 std::env::current_dir().unwrap());

    rust_reduce::reduce( runnable);
}

struct Standard {
    file: PathBuf,
    root_dir: PathBuf,
    cmd: Vec<OsString>,
    /// String to minimise while keeping in output.
    find: String
}

/// Recursively list all entry points. (for now excluding examples)
fn entry_points(base_path: PathBuf, results: &mut Vec<PathBuf>) {
    let cargo_toml = base_path.join("Cargo.toml");
    let cargo_toml : Config = toml::from_str(&std::fs::read_to_string(cargo_toml)
        .unwrap()).unwrap();

    let initial = results.len();

    if let Some(lib) = cargo_toml.lib {
        results.push(base_path.join(lib.path));
    }

    if let Some(bin) = cargo_toml.bin {
        for b in bin {
            results.push(base_path.join(b.path));
        }
    }

    if let Some(workspace) = cargo_toml.workspace {
        for krate in workspace.members {
            entry_points(base_path.join(krate), results )
        }
    }

    if initial == results.len()
    {
        //Assume main
        results.push(base_path.join("src/main.rs"));
    }
}

impl Standard {

    fn new(cmd: Vec<OsString>, find: String, root_dir: PathBuf) -> Standard {
        //todo: for now minimise first thing.

        let mut results = Vec::new();
        entry_points(root_dir.clone(), &mut results);

//        let file: OsString = if Some(lib) = config.lib {
//            OsString::from(lib.path)
//
//        } else if let Some(workspace) = config.worspace {
//            let cargo_toml = format!("{}/Cargo.toml", workspace.members[0]);
//
//            let cargo_toml = toml::from_str(&std::fs::read_to_string(cargo_toml)
//                .unwrap()).unwrap();
//        } else {
//            assert!(root_dir.join("src/main.rs").exists());
//            OsString::from("src/main.rs")
//        };
        println!("Found entry points: {:#?}, picking first", results);


        Standard { file: results[0].clone(), cmd, find, root_dir }
    }
}

impl Runnable for Standard {
    fn root(&self) -> &Path {
        &self.root_dir
    }

    fn get_path(&self) -> &Path {
        &self.file
    }

    fn run(&self) -> Result<(), String> {
        let (cmd, args) = self.cmd.split_first().expect("validated");
        let out = Command::new(cmd)
            .args(args)
            .current_dir(&self.root_dir)
            .output();
        if let Ok(out) = out {
            if String::from_utf8_lossy(&out.stdout).contains(&self.find) {
                return Ok(());
            }
            if String::from_utf8_lossy(&out.stderr).contains(&self.find)
            {
                return Ok(());
            }
            Err(format!("\nCould not find `{}` in:\nout:\n{}\nerr:\n{}",
                               &self.find, String::from_utf8_lossy(&out.stdout),
                               String::from_utf8_lossy(&out.stderr)))
        } else {
            println!("Couldn't find program to execute");
            Err(format!("Failed to execute: {:#?}", out)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;
    use std::path::PathBuf;
    use std::ffi::OsStr;
    use std::error::Error;

    fn home() -> PathBuf {
        Path::new(&std::env::var_os("HOME").unwrap_or_else(|| {
            let mut s = std::env::var_os("HOMEDRIVE").expect("HOMEDRIVE");
            s.push(&std::env::var_os("HOMEPATH").expect("HOMEPATH"));
            s
        })).to_owned()
    }
    
    #[test]
    fn test_find() {
        let r = Standard::new(vec![OsString::from("echo"),
                                   OsString::from("needle")],
                              "needle".to_string(),
                              std::env::current_dir().unwrap());
        assert_eq!(Ok(()), r.run());
    }

    #[test]
    fn test_not_find() {
        let r = Standard::new(vec![OsString::from("echo"),
                                   OsString::from("haystack")],
                              "needle".to_string(),
                              std::env::current_dir().unwrap());
        assert!(r.run().is_err());
    }

    fn reduce(root: PathBuf, find: &str, cargo_arg: &str) {
        let args = vec![
            OsString::from(home().join(&OsString::from(".cargo/bin/cargo"))),
            OsString::from(cargo_arg)];

        let runnable = Standard::new(args, find.to_owned(), root);
        assert_eq!(Ok(()), runnable.run());
        rust_reduce::reduce(runnable);
    }

    fn cargo<I,S>(pwd: &Path, args: I) -> std::io::Result<std::process::ExitStatus>
        where I: IntoIterator<Item=S>, S: AsRef<OsStr>
    {
        let cargo_path = home().join(&OsString::from(".cargo/bin/cargo"));
        let mut cmd = std::process::Command::new(&cargo_path);
        let cmd = cmd.args(args);
        cmd.current_dir(pwd);
        cmd.status()
    }

    fn read_file(path: &Path) -> String {
        match std::fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(msg) => panic!("{:?} gave error {}", path, msg)
        }
    }

    type Test = Result<(), Box<dyn Error>>;

    /// path needs to be absolute as debugger seems to forget the path.
    /// tests run in parallel by default! - don't rely on pwd.
    #[test]
    fn hello_main_works() -> Test {
        let loc = TempDir::new("reduce")?;
        let root = loc.path().join("testy");
        let main_file = root.join("src/main.rs");

        cargo(loc.path(), &vec!["new", "testy"])?;

        std::fs::write(&main_file, r#"
fn unused() {}

pub fn main() {
    println!("Hello, world!");
}
        "#)?;

        let find = "Hello";
        reduce(root, find, "run");

        assert!(read_file(&main_file.with_extension("rs.min")).contains(find));
        Ok(())
    }

    #[test]
    fn hello_lib_works() -> Test {
        let loc = TempDir::new("reduce")?;
        let root = loc.path().join("testy");
        let p = root.join("src/lib.rs");

        cargo(loc.path(), &vec!["new", "testy", "--lib"])?;

        reduce(root, "test result: ok","test");

        assert_eq!(std::fs::read_to_string(&p.with_extension("rs.min"))?.trim(), r#""#);
        Ok(())
    }

    #[test]
    fn hello_lib_works_min_to_1_test() -> Test {
        let loc = TempDir::new("reduce")?;
        cargo(loc.path(), &vec!["new", "testy", "--lib"])?;

        let root = loc.path().join("testy");
        let p = root.join("src/lib.rs");

        reduce(root, "test result: ok. 1 passed","test");
        let minimised = std::fs::read_to_string(
            &p.with_extension("rs.min"))?;

        assert_eq!(minimised, r#"#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
"#);
        Ok(())
    }

    /// We need to make sure we inline things like this otherwise code doesn't compile:
    /// #[cfg(test)]
    /// mod test_utils;
    #[test]
    fn test_mod_pulls_in_a_mod_with_cfg_test_attribute_on_it() -> Test {
        let loc = TempDir::new("reduce")?;
        let root = loc.path().join("testy");

        cargo(loc.path(), &vec!["new", "testy", "--lib"])?;

        std::fs::write(& root.join("src/lib.rs"), r#"
#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod tests {
    use crate::test_utils;

    #[test]
    fn it_works() {
        assert_eq!("wonderful", test_utils::util());
    }
}
        "#)?;
        std::fs::write(& root.join("src/test_utils.rs"), r#"
pub fn util() -> &'static str {
    "wonderful"
}
        "#)?;

        let p = root.join("src/lib.rs");

        reduce(root, "test result: ok. 1 passed","test");
        assert_eq!(std::fs::read_to_string(
            &p.with_extension("rs.min"))?, r#"#[cfg(test)]
mod test_utils {
    pub fn util() -> &'static str {
        unimplemented!()
    }
}
#[cfg(test)]
mod tests {
    use crate::test_utils;
    #[test]
    fn it_works() {}
}
"#);
        Ok(())
    }
}