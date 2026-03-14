use hayazip::ZipArchive;

fn main() {
    println!("Opening test.zip");
    match ZipArchive::open("test.zip") {
        Ok(archive) => {
            println!("Opened. Entries: {}", archive.entries().len());
            for entry in archive.entries() {
                println!("- {} @ {}", entry.filename, entry.local_header_offset);
                match entry.data_offset(archive.get_mmap()) {
                    Ok(off) => println!("  Data offset: {}", off),
                    Err(e) => println!("  Error getting data offset: {:?}", e),
                }
            }
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
}
