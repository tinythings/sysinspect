use libsensors::load;
use std::path::Path;

fn main() {
    let spec = match load(Path::new(".")) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error loading sensor specifications: {}", e);
            return;
        }
    };

    println!("{:#?}", spec);
}
