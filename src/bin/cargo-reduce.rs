use std::path::{Path, PathBuf};
use std::ffi::{OsString};
use std::process::Command;
use clap::clap_app;

use rust_reduce::Runnable;

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

    let find = matches.value_of_lossy("FIND").expect("string to search for").to_owned();
    let mut cmd = vec![matches.value_of_os("CMD").expect("validated").to_owned()];
    let iter = matches.values_of_os("ARGS").expect("validated").map(ToOwned::to_owned);
    cmd.extend(iter);

    let runnable = Standard::new(cmd, find.to_string(), std::env::current_dir().unwrap());

    rust_reduce::reduce( runnable);
}

struct Standard {
    file: PathBuf,
    root_dir: PathBuf,
    cmd: Vec<OsString>,
    /// String to minimise while keeping in output.
    find: String
}

impl Standard {
    fn new(cmd: Vec<OsString>, find: String, root_dir: PathBuf) -> Standard {
        let file: OsString = if root_dir.join("src/main.rs").exists() {
            OsString::from("src/main.rs")//todo naive
        } else {
            OsString::from("src/lib.rs")
        };

        Standard { file: root_dir.join(file), cmd, find, root_dir }
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
            return Err(format!("\nCould not find `{}` in:\nout:\n{}\nerr:\n{}",
                               &self.find, String::from_utf8_lossy(&out.stdout),
                               String::from_utf8_lossy(&out.stderr)));
        } else {
            return Err(format!("Failed to execute: {:#?}", out))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;
    use std::path::PathBuf;
    use std::ffi::OsStr;

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



    #[test]
    fn hello_main_works() {
        let loc = TempDir::new("reduce").unwrap();
        //path needs to be absolute as debugger seems to forget the path.
        cargo(loc.path(), &vec!["new", "testy"]).unwrap();



        let root = loc.path().join("testy");

        let p = root.clone().join("src/main.rs");
        //tests run in parallel by default!
        //std::env::set_current_dir(root).unwrap();

        std::fs::write(&p, r#"
fn unused() {}

pub fn main() {
    println!("Hello, world!");
}
        "#).unwrap();
        println!("Before: {} at {:?}", std::fs::read_to_string(&p).unwrap(), &p);
        let find = "Hello";


        let runnable = Standard::new(vec![OsString::from(home().join(&OsString::from(".cargo/bin/cargo"))),
                                          OsString::from("run")],
                                     find.to_owned(), root);
        assert_eq!(Ok(()), runnable.run());
        rust_reduce::reduce(runnable);
        println!("After: {}", read_file(&p.with_extension("rs.min")));

        assert!(read_file(&p).contains(find));
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

    #[test]
    fn hello_lib_works() {
        // Note: each test gets run in parallel and thus you can't just switch
        // current process' working dir.

        let loc = TempDir::new("reduce").unwrap();
        let mut cmd = std::process::Command::new(home().join(".cargo/bin/cargo"));
        let cmd = cmd.args(vec!["new", "testy", "--lib"]);

        cmd.current_dir(loc.path());
        //let _res = cmd.output();
        cmd.status().unwrap();
        let root = loc.path().join("testy");
        let p = root.clone().join("src/lib.rs");

        //std::env::set_current_dir(root).unwrap();

        println!("Before: {}", std::fs::read_to_string(&p).unwrap());
        let find = "test result: ok";

        let runnable = Standard::new(vec![home().join(".cargo/bin/cargo").into_os_string(),
                                          OsString::from("test")],
                                     find.to_owned(), root);
        assert_eq!(Ok(()), runnable.run());
        rust_reduce::reduce(runnable);
        let minimised = std::fs::read_to_string(&p.with_extension("rs.min")).unwrap();
        println!("After: {}", minimised);

        assert_eq!(minimised.trim(), r#""#);
    }

    #[test]
    fn hello_lib_works_min_to_1_test() {
        let loc = TempDir::new("reduce").unwrap();
        let mut cmd = std::process::Command::new(home()
            .join(".cargo/bin/cargo"));

        let cmd = cmd.args(vec!["new", "testy", "--lib"]);

        let base = loc.path().canonicalize().unwrap();
        println!("base {:?}", base);
        cmd.current_dir(base);
        //let res = cmd.output();
        let _o = cmd.output().unwrap();
        let root = loc.path().join("testy");
        let p = root.clone().join("src/lib.rs");

        //std::env::set_current_dir(root).unwrap();

        println!("Before: {}", std::fs::read_to_string(&p).unwrap());
        let find = "test result: ok. 1 passed";

        let runnable = Standard::new(vec![
//            home().join("projects/replay/target/debug/replay").into_os_string(),
home().join(".cargo/bin/cargo").into_os_string(),
OsString::from("test")],
                                     find.to_owned(), root);
        assert_eq!(Ok(()), runnable.run());
        rust_reduce::reduce(runnable);
        let minimised = std::fs::read_to_string(
            &p.with_extension("rs.min")).unwrap();
        println!("After: {}", minimised);

        assert_eq!(minimised, r#"#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
"#);
    }

    /// We need to make sure we inline things like this otherwise code doesn't compile:
    /// #[cfg(test)]
    /// mod test_utils;
    #[test]
    fn test_mod_pulls_in_a_mod_with_cfg_test_attribute_on_it() {
        let loc = TempDir::new("reduce").unwrap();
        let mut cmd = std::process::Command::new(home()
            .join(".cargo/bin/cargo"));

        let cmd = cmd.args(vec!["new", "testy", "--lib"]);

        let base = loc.path().canonicalize().unwrap();
        println!("base {:?}", base);
        cmd.current_dir(base);
        //let res = cmd.output();
        let _o = cmd.output().unwrap();
        let root = loc.path().join("testy");

        std::fs::write(& root.clone().join("src/lib.rs"), r#"
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
        "#).unwrap();
        std::fs::write(& root.clone().join("src/test_utils.rs"), r#"
pub fn util() -> &'static str {
    "wonderful"
}
        "#).unwrap();

        let p = root.clone().join("src/lib.rs");

        //std::env::set_current_dir(root).unwrap();

        println!("Before: {}", std::fs::read_to_string(&p).unwrap());
        let find = "test result: ok. 1 passed";

        let runnable = Standard::new(vec![
//            home().join("projects/replay/target/debug/replay").into_os_string(),
home().join(".cargo/bin/cargo").into_os_string(),
OsString::from("test")],
                                     find.to_owned(), root);
        assert_eq!(Ok(()), runnable.run());
        rust_reduce::reduce(runnable);
        let minimised = std::fs::read_to_string(
            &p.with_extension("rs.min")).unwrap();
        println!("After: {}", minimised);

        assert_eq!(minimised, r#"#[cfg(test)]
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
    }
}