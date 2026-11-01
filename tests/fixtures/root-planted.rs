// Generic Root-profile input. It is never compiled.
use std::env as environment;
use std::fs as disk;
use std::path::Path;

fn root(path: &Path) {
    let _ = disk::read_to_string("settings");
    let _ = environment::temp_dir();
    let _ = std::net::TcpStream::connect("localhost:1");
    let _ = std::thread::spawn(|| {});
    let _ = path.exists();
    unsafe {}
}

#[path = "generated.rs"]
mod generated;
