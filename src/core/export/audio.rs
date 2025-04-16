use std::{io::Read, path::PathBuf};

use swf::{AudioCompression, ExportedAsset, Sound, SoundFormat, Tag};

use super::{Arenas, SwfBuilder};

pub(super) fn build_audio<'a>(
    swf_builder: &mut SwfBuilder<'a>,
    arenas: &'a Arenas,
    directory: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let asset_dir = directory.join("assets");
    let fs_assets = std::fs::read_dir(asset_dir)?;
    for fs_asset in fs_assets {
        let file = fs_asset?;
        let file_name = file
            .file_name()
            .into_string()
            .map_err(|original_os_string| {
                format!("Non utf-8 filename: '{:?}'", original_os_string)
            })?;
        if file_name.ends_with(".mp3") {
            build_mp3(swf_builder, arenas, file, file_name.clone())
                .map_err(|err| format!("Error decoding '{}': {}", file_name, err))?;
        } else if file_name.ends_with(".wav") {
            build_wav(swf_builder, arenas, file, file_name.clone())
                .map_err(|err| format!("Error decoding '{}': {}", file_name, err))?;
        }
    }
    Ok(())
}

fn build_wav<'a>(
    swf_builder: &mut SwfBuilder<'a>,
    arenas: &'a Arenas,
    file: std::fs::DirEntry,
    file_name: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let reader = hound::WavReader::open(file.path())?;
    let duration = reader.duration();
    let spec = reader.spec();

    if !(spec.channels == 1 || spec.channels == 2) {
        return Err(format!(
            "Wave file should have 1 or 2 channels, has {}",
            spec.channels
        )
        .into());
    }
    if !(spec.bits_per_sample == 8 || spec.bits_per_sample == 16) {
        return Err(format!(
            "Wave file should have 8 or 16 bits per sample, has {}",
            spec.bits_per_sample
        )
        .into());
    }
    let suppored_sample_rate = match spec.sample_rate {
        5512 => true,
        11025 => true,
        22050 => true,
        44100 => true,
        _ => false,
    };
    if !suppored_sample_rate {
        return Err(format!(
            "Wave file should have a sample rate of 5512, 11025, 22050 or 44100, is {}",
            spec.sample_rate
        )
        .into());
    }

    let mut data: Vec<u8> = vec![];
    // use the underlying reader because we just want the data instead of decoding it ourselves
    reader.into_inner().read_to_end(&mut data)?;
    let character_id = swf_builder.next_character_id();
    swf_builder.tags.push(Tag::DefineSound(Box::new(Sound {
        id: character_id,
        format: SoundFormat {
            compression: AudioCompression::Uncompressed,
            sample_rate: spec.sample_rate as u16,
            is_stereo: spec.channels == 2,
            is_16_bit: spec.bits_per_sample == 16,
        },
        num_samples: duration,
        data: arenas.data.alloc(data),
    })));
    swf_builder.tags.push(Tag::ExportAssets(vec![ExportedAsset {
        id: character_id,
        name: arenas.alloc_swf_string(file_name),
    }]));
    Ok(())
}

fn build_mp3<'a>(
    swf_builder: &mut SwfBuilder<'a>,
    arenas: &'a Arenas,
    file: std::fs::DirEntry,
    file_name: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let data: Vec<u8> = std::fs::read(file.path())?;
    // TODO: swfmill adds padding to the data, but it seems to work in Flash player without that padding?
    // see: https://github.com/djcsdy/swfmill/blob/master/src/swft/swft_import_mp3.cpp#L213
    let (header, samples) = puremp3::read_mp3(data.as_slice())?;

    if !(header.channels.num_channels() == 1 || header.channels.num_channels() == 2) {
        return Err(format!(
            "Mp3 should have 1 or 2 channels, has {}",
            header.channels.num_channels()
        )
        .into());
    }
    let suppored_sample_rate = match header.sample_rate.hz() {
        5512 => false, // not allowed for mp3 according to the spec
        11025 => true,
        22050 => true,
        44100 => true,
        _ => false,
    };
    if !suppored_sample_rate {
        return Err(format!(
            "Mp3 should have a sample rate of 11025, 22050 or 44100, is {}",
            header.sample_rate.hz()
        )
        .into());
    }

    // TODO: this decodes the whole mp3 just to get the sample count
    // this is inefficient, it should just read the frame data
    let duration = samples.count();
    let character_id = swf_builder.next_character_id();
    swf_builder.tags.push(Tag::DefineSound(Box::new(Sound {
        id: character_id,
        format: SoundFormat {
            compression: AudioCompression::Mp3,
            sample_rate: header.sample_rate.hz() as u16,
            is_stereo: header.channels.num_channels() == 2,
            // according to the spec, this is ignored for compressed formats like mp3 and always decoded to 16 bits
            is_16_bit: true,
        },
        num_samples: duration as u32,
        data: arenas.data.alloc(data),
    })));
    swf_builder.tags.push(Tag::ExportAssets(vec![ExportedAsset {
        id: character_id,
        name: arenas.alloc_swf_string(file_name),
    }]));
    Ok(())
}
