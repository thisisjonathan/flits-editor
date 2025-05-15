use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    thread::{self, JoinHandle},
};

use duct::cmd;

pub fn run_movie<T: Send + Clone + 'static>(
    swf_path: &PathBuf,
    output_arg: T,
    output_callback: fn(line: String, T) -> (),
    end_callback: fn(T) -> (),
) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
    // No need to add .exe on windows, Command does that automatically
    let ruffle_path = std::env::current_exe()?
        .parent()
        .ok_or("Editor executable is not in a directory")?
        .join("dependencies/ruffle");
    let ruffle = cmd!(ruffle_path, swf_path);

    let reader = BufReader::new(ruffle.stderr_to_stdout().reader()?);
    let join_handle = thread::spawn(move || {
        reader
            .lines()
            .filter_map(|line| line.ok())
            .for_each(|line| {
                output_callback(line, output_arg.clone());
            });
        end_callback(output_arg);
    });
    Ok(join_handle)
}
