use std::fs;

pub fn file2str(file: &str) -> std::io::Result<String> {
    fs::read_to_string(file)
}

pub fn apmax<T: Clone + PartialOrd>(a: &mut T, b: T) {
    if *a < b {
        *a = b;
    }
}
