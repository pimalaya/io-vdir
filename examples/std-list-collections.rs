use std::{
    env,
    io::{stdin, stdout, Write as _},
    path::PathBuf,
};

use io_fs::runtimes::std::handle;
use io_vdir::coroutines::list_collections::{ListCollections, ListCollectionsResult};

fn main() {
    let _ = env_logger::try_init();

    let path: PathBuf = match env::var("DIR") {
        Ok(dir) => dir.into(),
        Err(_) => read_line("Collections home path?").into(),
    };

    let mut arg = None;
    let mut coroutine = ListCollections::new(&path);

    let collections = loop {
        match coroutine.resume(arg) {
            ListCollectionsResult::Ok(collections) => break collections,
            ListCollectionsResult::Err(err) => panic!("{err}"),
            ListCollectionsResult::Io(io) => arg = Some(handle(io).unwrap()),
        }
    };

    println!("Collection paths:");

    for collection in collections {
        println!(" - {}", collection.path.display());
    }
}

fn read_line(prompt: &str) -> String {
    print!("{prompt} ");
    stdout().flush().unwrap();
    let mut line = String::new();
    stdin().read_line(&mut line).unwrap();
    line.trim().to_owned()
}
