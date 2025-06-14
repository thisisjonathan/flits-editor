// simple program to test exporting without needing to recompile the whole editor
// run in the root of the project: cargo run -p flits-core
// TODO: don't hardcode project path
fn main() {
    println!("Loading...");
    let movie = flits_core::Movie::load("example/movie.json".into()).unwrap();

    println!("Exporting...");
    let swf_path = "example/output.swf";
    movie.export("example".into(), swf_path.into()).unwrap();

    println!("Running...");
    let join_handle = flits_core::run::run_movie(
        &swf_path.into(),
        (),
        |line, _| {
            println!("{}", line);
        },
        |_| {},
    )
    .unwrap();
    join_handle.join().unwrap();
}
