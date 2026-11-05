use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead};

// 🧑 "Try to make Rust a good scripting target"
fn main() -> Result<(), Box<dyn Error>> {
    let paths: Vec<_> = std::env::args_os().skip(1).collect();
    if paths.is_empty() {
        println!("{}", count_lines(io::stdin().lock())?);
    } else {
        for path in paths {
            let n_lines = count_lines(io::BufReader::new(File::open(&path)?))?;
            println!("{n_lines}\t{}", path.to_string_lossy());
        }
    }
    Ok(())
}

fn count_lines(input: impl BufRead) -> io::Result<usize> {
    input.lines().try_fold(0, |n, line| line.map(|_| n + 1))
}
