use hayazip::create_zip;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <source_path> <archive.zip>", args[0]);
        std::process::exit(1);
    }

    let source_path = &args[1];
    let archive_path = &args[2];
    let start = Instant::now();

    match create_zip(source_path, archive_path) {
        Ok(_) => {
            let duration = start.elapsed();
            println!("Archive creation successful in {:?}", duration);
        }
        Err(e) => {
            eprintln!("Archive creation failed: {:?}", e);
            std::process::exit(1);
        }
    }
}
