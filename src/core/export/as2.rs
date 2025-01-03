use std::{collections::HashMap, path::PathBuf};

use swf::{
    avm1::types::{Action, ConstantPool, Push},
    CharacterId, SwfStr, Tag,
};

use crate::core::{Movie, Symbol, SymbolIndex};

pub(super) fn compile_as2(
    movie: &Movie,
    symbol_index_to_character_id: &HashMap<SymbolIndex, CharacterId>,
    project_directory: PathBuf,
    swf_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let dependencies_dir = std::env::current_exe()?
        .parent()
        .ok_or("Editor executable is not in a directory")?
        .join("dependencies");
    // No need to add .exe on windows, Command does that automatically
    let mtasc_path = dependencies_dir.join("mtasc");

    let src_dir = project_directory.join("src");

    let mut command = std::process::Command::new(mtasc_path);
    command.arg("-swf").arg(swf_path.clone());
    command.arg("-version").arg("8"); // use newer as2 standard library
    command.arg("-cp").arg(dependencies_dir.join("std")); // set class path
    command.arg("-cp").arg(dependencies_dir.join("std8")); // set class path for version 8
    command.arg("-cp").arg(src_dir.clone()); // also look for classes in the src directory, otherwise you can't extend your own classes
    command.arg("-frame").arg(movie.num_frames().to_string()); // put classes in last frame
    command.arg("-infer"); // automatically infer types of variables

    let mut at_least_one_file = false;
    std::fs::create_dir_all(src_dir.clone())?;
    // TODO: subdirectories
    for src_file in src_dir.read_dir()? {
        // this needs to be relative to the class path
        command.arg(src_file?.file_name());
        at_least_one_file = true;
    }

    if at_least_one_file {
        let output = command.output().map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                "Could not find mtasc executable. There is supposed to be a 'dependencies' directory in the same directory as this program with the mtasc executable.".into()
            }
            _ => format!("Unable to run mtasc (as2 compiler): {}", err),
        })?;

        if !output.status.success() {
            return Err(format!(
                "{}{}",
                std::str::from_utf8(&output.stdout)?,
                std::str::from_utf8(&output.stderr)?
            )
            .into());
        }

        // put placeobject after the class definitions, otherwise it won't work
        let file = std::fs::File::open(swf_path.clone())?;
        let reader = std::io::BufReader::new(file);
        let swf_buf = swf::decompress_swf(reader)?;
        let mut swf = swf::parse_swf(&swf_buf)?;

        // add actions to call Object.registerClass for each movieclip with a class
        let mut symbol_index = 0;
        let mut action_datas = vec![];
        for symbol in &movie.symbols {
            if let Symbol::MovieClip(movieclip) = symbol {
                if movieclip.properties.class_name.len() > 0 {
                    let mut action_data: Vec<u8> = vec![];
                    let mut action_writer =
                        swf::avm1::write::Writer::new(&mut action_data, swf.header.version());
                    let action = Action::ConstantPool(ConstantPool {
                        strings: vec![
                            SwfStr::from_utf8_str("Object"),
                            SwfStr::from_utf8_str("registerClass"),
                            SwfStr::from_utf8_str(&movieclip.properties.name),
                            SwfStr::from_utf8_str(&movieclip.properties.class_name),
                        ],
                    });
                    action_writer.write_action(&action)?;
                    let action = Action::Push(Push {
                        values: vec![swf::avm1::types::Value::ConstantPool(3)],
                    });
                    action_writer.write_action(&action)?;
                    let action = Action::GetVariable;
                    action_writer.write_action(&action)?;
                    let action = Action::Push(Push {
                        values: vec![
                            swf::avm1::types::Value::ConstantPool(2),
                            swf::avm1::types::Value::Int(2),
                            swf::avm1::types::Value::ConstantPool(0),
                        ],
                    });
                    action_writer.write_action(&action)?;
                    let action = Action::GetVariable;
                    action_writer.write_action(&action)?;
                    let action = Action::Push(Push {
                        values: vec![swf::avm1::types::Value::ConstantPool(1)],
                    });
                    action_writer.write_action(&action)?;
                    let action = Action::CallMethod;
                    action_writer.write_action(&action)?;
                    let action = Action::Pop;
                    action_writer.write_action(&action)?;
                    // Flash player crashes without this end action, see: https://github.com/ruffle-rs/ruffle/issues/18560
                    let action = Action::End;
                    action_writer.write_action(&action)?;
                    action_datas.push(action_data);
                }
            }
            symbol_index += 1;
        }
        symbol_index = 0;
        let mut action_nr = 0;
        for symbol in &movie.symbols {
            if let Symbol::MovieClip(movieclip) = symbol {
                if movieclip.properties.class_name.len() > 0 {
                    let character_id = *symbol_index_to_character_id
                        .get(&symbol_index)
                        .ok_or("MovieClip with unknown character id")?;
                    // -1 because of ShowFrame
                    swf.tags.insert(
                        swf.tags.len() - 1,
                        Tag::DoInitAction {
                            id: character_id,
                            action_data: &action_datas[action_nr],
                        },
                    );
                    action_nr += 1;
                }
            }
            symbol_index += 1;
        }

        let mut tags_to_place_at_end = vec![];
        let mut index = 0;
        let mut frame = 0;
        // find tags
        for tag in &swf.tags {
            // skip frames before the last one (don't mess with the preloader)
            if frame < swf.header.num_frames() - 1 {
                if let Tag::ShowFrame = tag {
                    frame += 1;
                }
                index += 1;
                continue;
            }
            if matches!(tag, Tag::PlaceObject(_)) {
                tags_to_place_at_end.push(index);
            }
            index += 1;
        }

        // iterate in reverse order to make sure placing the tag at the end doesn't change the index of the other tags
        for index_reference in tags_to_place_at_end.iter().rev() {
            let index = *index_reference;
            // length minus 2 because it swaps with the next one and ShowFrame still needs to be last
            for swap_index in index..swf.tags.len() - 2 {
                swf.tags.swap(swap_index, swap_index + 1);
            }
        }

        // write the new version
        let file = std::fs::File::create(swf_path)?;
        let writer = std::io::BufWriter::new(file);
        swf::write_swf(&swf.header.swf_header(), &swf.tags, writer)?;
    }
    Ok(())
}
