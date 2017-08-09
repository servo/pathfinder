// pathfinder/font-renderer/src/tests.rs

use app_units::Au;
use euclid::Size2D;
use std::fs::File;
use std::io::Read;
use {FontContext, FontInstanceKey, FontKey, GlyphDimensions, GlyphKey};

static TEST_FONT_PATH: &'static str = "resources/tests/nimbus-sans/NimbusSanL-Regu.ttf";
const TEST_FONT_SIZE: Au = Au(60 * 16);
const TEST_FIRST_GLYPH_INDEX: u32 = 0x1f;

// Nimbus Sans Regular 16pt., 'A'
const EXPECTED_GLYPH_ORIGIN: [i32; 2] = [1, 12];
const EXPECTED_GLYPH_SIZE: [u32; 2] = [8, 12];
const EXPECTED_GLYPH_ADVANCE: f32 = 9.0;

#[test]
fn test_font_context_glyph_dimensions() {
    let mut font_context = FontContext::new();

    let font_key = FontKey::new();
    let mut bytes = vec![];
    File::open(TEST_FONT_PATH).unwrap().read_to_end(&mut bytes).unwrap();
    font_context.add_font_from_memory(&font_key, bytes, 0).unwrap();

    let font_instance = FontInstanceKey::new(&font_key, TEST_FONT_SIZE);
    let glyph_key = GlyphKey::new('A' as u32 - TEST_FIRST_GLYPH_INDEX);
    let glyph_dimensions = font_context.glyph_dimensions(&font_instance, &glyph_key).unwrap();

    assert_eq!(glyph_dimensions, GlyphDimensions {
        origin: EXPECTED_GLYPH_ORIGIN.into(),
        size: Size2D::new(EXPECTED_GLYPH_SIZE[0], EXPECTED_GLYPH_SIZE[1]),
        advance: EXPECTED_GLYPH_ADVANCE,
    })
}
