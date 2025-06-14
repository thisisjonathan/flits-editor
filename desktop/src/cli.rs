use std::path::PathBuf;

#[derive(Clone)]
pub enum CliParams {
    NoProject,
    OpenProject(PathBuf),
}
pub fn parse_command_line_arguments() -> CliParams {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        return CliParams::OpenProject(PathBuf::from(args[1].clone()));
    }
    CliParams::NoProject
}
