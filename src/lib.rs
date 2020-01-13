// Copyright (c) Jethro G. Beekman
//
// This file is part of rust-reduce.
//
// rust-reduce is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// rust-reduce is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with rust-reduce.  If not, see <https://www.gnu.org/licenses/>.

use std::{ffi::OsString, io::Write, process::{Command, Stdio}};

use quote::ToTokens;
use syn_inline_mod::{Error as InlineError, InlinerBuilder};
use std::path::Path;

mod transforms;

pub fn reduce<R: Runnable>(runnable: R)
{
    if let Err(msg) = runnable.run() {
        eprintln!("rust-reduce: run with initial input did not indicate success: {}", msg);
        std::process::exit(1);
    }

    //println!("Reducing {:?}", runnable.get_path());
    //println!("Reducing {:?}", runnable.get_path().canonicalize().unwrap());
    std::io::stdout().flush().unwrap();
    let original = std::fs::read(runnable.get_path()).unwrap();

    let mut inlined_file = match InlinerBuilder::new()
        .error_not_found(true)
        .parse_and_inline_modules(runnable.get_path()) {
        Ok(f) => f,
        Err(InlineError::NotFound(missing)) => {
            eprintln!("rust-reduce: file not found");
            for (modname, loc) in missing {
                eprintln!("    mod {} @ {}:{}", modname, loc.path.display(), loc.line);
            }
            std::process::exit(1);
        },
        Err(err) => unimplemented!("This wasn't supposed to happen. {:?}", err)
    };

    let mut try_compile = |reduced_syn_file: &syn::File| {
        let path = runnable.get_path();
        let mut file = std::fs::File::create(path)
            .unwrap_or_else(|_| panic!("{:?}", path));
        write!(file, "{}", reduced_syn_file.into_token_stream()).unwrap();
        runnable.run()
    };

    // Write the inlined file:
    try_compile(&inlined_file).unwrap();

    println!("Pruning items");
    transforms::prune_items::prune_items(&mut inlined_file, &mut try_compile);
    println!("Removing #[derive] attributes");
    transforms::remove_derive_attrs::remove_derive_attrs(&mut inlined_file, &mut try_compile);
    println!("Removing #[doc] attributes");
    transforms::remove_doc_attrs::remove_doc_attrs(&mut inlined_file, &mut try_compile);
    println!("Clearing block bodies - {{}}");
    transforms::empty_blocks::empty_blocks(&mut inlined_file, &mut try_compile);
    println!("Clearing block bodies - unimplemented");
    transforms::clear_blocks::clear_blocks(&mut inlined_file, &mut try_compile);
    println!("Removing pub");
    transforms::privatiser::privatise_items(&mut inlined_file, &mut try_compile);

    // Ensure a successful file is written:
    try_compile(&inlined_file).unwrap();

    if let Err(msg) = Command::new("cargo")
        .args(vec!["fmt"])
        .current_dir(&runnable.root())
        .output() {
        eprintln!("cargo fmt failed/not found so min unformatted. {}", msg);
    }

    //Put the original one back...
    //let min = std::fs::read(runnable.get_path()).unwrap();

//    let min_path = runnable.get_path()
//        .with_extension("rs");
//    println!("writing min to {:?}", &min_path);
//    std::fs::copy(runnable.get_path(), min_path).unwrap();
//
//    //std::fs::write(min_path, min).unwrap();
//
//    std::fs::write(runnable.get_path(), original).unwrap();
}

pub trait Runnable {
    fn root(&self) -> &Path;
    fn get_path(&self) -> &Path;
    fn run(&self) -> Result<(), String>;
}

pub struct TestScript<'me>{
    pub cmd: Vec<OsString>,
    pub path: &'me std::path::Path
}

impl <'me> Runnable for TestScript<'me> {
    fn root(&self) -> &Path {
        unimplemented!()
    }

    fn get_path(&self) -> &Path {
        &self.path
    }

    fn run(&self) -> Result<(), String> {
        let (cmd, args) = self.cmd.split_first()
            .expect("validated");

        match Command::new(cmd)
            .args(args)
            .arg(&self.path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            {
                Ok(ref stat) if stat.success() => Ok(()),
                _ => Err("Exit code was non-zero.".to_string())
            }
    }
}