/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

use batch::GlyphRange;
use buffers::GlyphBuffers;
use charmap::CodepointRange;
use memmap::{Mmap, Protection};
use otf::FontData;
use test::Bencher;

static TEST_FONT_PATH: &'static str = "resources/tests/nimbus-sans/NimbusSanL-Regu.ttf";

#[bench]
fn bench_add_glyphs(bencher: &mut Bencher) {
    let file = Mmap::open_path(TEST_FONT_PATH, Protection::Read).expect("Couldn't open test font");
    unsafe {
        let font = FontData::new(file.as_slice());
        let cmap = font.cmap_table().unwrap();
        let glyf = font.glyf_table().unwrap();
        let head = font.head_table().unwrap();
        let loca = font.loca_table(&head).unwrap();
        let codepoint_ranges = [CodepointRange::new('!' as u32, '~' as u32)];
        let glyph_ranges = cmap.glyph_ranges_for_codepoint_ranges(&codepoint_ranges)
                               .expect("Couldn't find glyph ranges");

        bencher.iter(|| {
            let mut buffers = GlyphBuffers::new();
            for glyph_id in glyph_ranges.iter().flat_map(GlyphRange::iter) {
                buffers.add_glyph(glyph_id as u32, &loca, &glyf).unwrap()
            }
        });
    }
}

