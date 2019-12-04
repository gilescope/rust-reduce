use std::path::Path;
use std::ffi::OsString;

use clap::clap_app;
use rust_reduce::TestScript;

fn main() {
    let matches = clap_app!(("rust-reduce") =>
        (version: clap::crate_version!())
        (@arg CMD: * "Command to run.")
        (@arg ARGS: * ... "Arguments to the command to run.

The last argument must be the path of the existing file of interest. CMD will be invoked with the last argument replaced with the path to a temporary file.

You can use `--` to separate ARGS from any arguments passed to `rust-reduce`.")
        (after_help: "\
`rust-reduce` will try to make the source file smaller by interpreting it as valid Rust code and intelligently removing parts of the code. After each removal, the given command will be run but passing a path to a file containing the reduced code. The command should return 0 if run on the original input, and also if the reduced code is interesting, non-0 otherwise.

The original file will be overwritten with the smallest interesting reduced version, if found. This happens while `rust-reduce` is running. The original file will be backed up with the `.orig` suffix. If `rustfmt` is found, it will be used to clean up the output.

A common way to use `rust-reduce` is to write a short shell script that runs `rustc` and greps the compiler output for a particular error message. NB. you will want to look for a specific error message because while `rust-reduce` will generate syntactically correct code, it's not guaranteed to compile.

The original file may refer to modules in different files, these will be inlined and reduced along with the main file.")
    ).get_matches();

    let mut cmd = vec![matches.value_of_os("CMD").expect("validated").to_owned()];
    let mut iter = matches.values_of_os("ARGS").expect("validated").map(ToOwned::to_owned);
    let file: OsString = iter.next_back().expect("validated");
    cmd.extend(iter);
    let action = TestScript{ cmd, path: Path::new(&file) };
    rust_reduce::reduce( action);
}
