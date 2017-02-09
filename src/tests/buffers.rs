/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

use charmap::CodepointRange;
use memmap::{Mmap, Protection};
use otf::Font;
use outline::OutlineBuilder;
use test::Bencher;

static TEST_FONT_PATH: &'static str = "resources/tests/nimbus-sans/NimbusSanL-Regu.ttf";

#[bench]
fn bench_add_glyphs(bencher: &mut Bencher) {
    let file = Mmap::open_path(TEST_FONT_PATH, Protection::Read).expect("Couldn't open test font");
    unsafe {
        let font = Font::new(file.as_slice()).unwrap();
        let codepoint_ranges = [CodepointRange::new('!' as u32, '~' as u32)];
        let glyph_mapping = font.glyph_mapping_for_codepoint_ranges(&codepoint_ranges)
                               .expect("Couldn't find glyph ranges");

        bencher.iter(|| {
            let mut outline_builder = OutlineBuilder::new();
            for (_, glyph_id) in glyph_mapping.iter() {
                outline_builder.add_glyph(&font, glyph_id).unwrap();
            }
        });
    }
}

