use hayazip::extract;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <archive.zip> <out_dir>", args[0]);
        std::process::exit(1);
    }

    let archive_path = &args[1];
    let out_dir = &args[2];

    let start = Instant::now();

    match extract(archive_path, out_dir) {
        Ok(_) => {
            let duration = start.elapsed();
            println!("Extraction successful in {:?}", duration);
        }
        Err(e) => {
            eprintln!("Extraction failed: {:?}", e);
            std::process::exit(1);
        }
    }
}
