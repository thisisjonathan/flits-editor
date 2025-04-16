use swf::{ExportedAsset, Sprite, Tag};

use crate::core::{MovieClip, SymbolIndex};

use super::{get_placed_symbols_tags, Arenas, SwfBuilder, SwfBuilderTag};

pub(super) fn build_movieclip_outer(
    symbol_index: SymbolIndex,
    _movieclip: &MovieClip,
    swf_builder: &mut SwfBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    let character_id = swf_builder.next_character_id();
    swf_builder
        .symbol_index_to_character_id
        .insert(symbol_index, character_id);
    swf_builder
        .symbol_index_to_tag_index
        .insert(symbol_index, swf_builder.tags.len());
    swf_builder
        .tags
        .push(SwfBuilderTag::Tag(Tag::DefineSprite(Sprite {
            id: character_id,
            num_frames: 1,
            tags: vec![], // these are filled in by build_movieclip_inner()
        })));
    Ok(())
}

pub(super) fn build_movieclip_inner<'a>(
    symbol_index: SymbolIndex,
    movieclip: &MovieClip,
    swf_builder: &mut SwfBuilder<'a>,
    arenas: &'a Arenas,
) -> Result<(), Box<dyn std::error::Error>> {
    let inner_tags = get_placed_symbols_tags(&movieclip.place_symbols, swf_builder)?;
    let tag = &mut swf_builder.tags[swf_builder.symbol_index_to_tag_index[&symbol_index]];
    let SwfBuilderTag::Tag(Tag::DefineSprite(define_sprite_tag)) = tag else {
        return Err(format!(
            "The tag for the movieclip with symbol index {} is not a DefineSprite tag",
            symbol_index
        )
        .into());
    };
    define_sprite_tag.tags = inner_tags;

    // ffdec gives a warning about export asset tags for assets where not all the symbols inside are defined
    // that's why we only create the export assets tag in the second iteration, after we've created all the regular tags

    // export all movieclips, this allows you to create them with attachMovie
    // this is easier than having to remember to check a box for each one
    // TODO: is there a reason not to do this?
    swf_builder
        .tags
        .push(SwfBuilderTag::Tag(Tag::ExportAssets(vec![ExportedAsset {
            id: swf_builder.symbol_index_to_character_id[&symbol_index],
            name: arenas.alloc_swf_string(movieclip.properties.name.clone()),
        }])));

    Ok(())
}
