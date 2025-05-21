## Running the editor

The editor expects a `dependencies` directory to exist in the same directory as the binary.
This folder should contain [ruffle](https://ruffle.rs) and [mtasc](http://tech.motion-twin.com/mtasc.html) binaries.
For example, when running `cargo run` (in debug mode) you should have the following directory structure:
```
target
  debug
    dependencies
      std
        <std files used by mtasc>
      std8
        <std8 files used by mtasc>
      mtasc
      ruffle
```
On windows the file names of the binaries should end with `.exe`.

## License

GNU General Public License v3.0 or later. See [LICENSE](LICENSE) to see the full text.
The files in the `windowing/src` and `editor/src/desktop` folders are adapted from [Ruffle](https://ruffle.rs) which is licensed under Apache V2.0/MIT.