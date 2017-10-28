// pathfinder/font-renderer/src/tests.rs

use app_units::Au;
use env_logger;
use euclid::Size2D;
use euclid::approxeq::ApproxEq;
use pathfinder_path_utils::{PathBuffer, Subpath};
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use {FontContext, FontInstance, FontKey, GlyphDimensions, GlyphKey, SubpixelOffset};

static TEST_FONT_PATH: &'static str = "../resources/fonts/nimbus-sans/NimbusSanL-Regu.ttf";
const TEST_FONT_SIZE: Au = Au(60 * 16);
const TEST_GLYPH_ID: u32 = 68;  // 'a'

const EXPECTED_GLYPH_ORIGIN: [i32; 2] = [0, 9];
const EXPECTED_GLYPH_SIZE: [u32; 2] = [9, 9];
const EXPECTED_GLYPH_ADVANCE: f32 = 9.0;

static EXPECTED_GLYPH_ENDPOINTS: [[f32; 2]; 34] = [
    [ 548.0, 77.0  ],
    [ 548.0, 10.0  ],
    [ 490.0, 0.0   ],
    [ 402.0, 82.0  ],
    [ 323.0, 24.0  ],
    [ 219.0, 0.0   ],
    [ 89.0,  44.0  ],
    [ 43.0,  158.0 ],
    [ 138.0, 301.0 ],
    [ 233.0, 324.0 ],
    [ 310.0, 333.0 ],
    [ 381.0, 353.0 ],
    [ 399.0, 392.0 ],
    [ 399.0, 414.0 ],
    [ 362.0, 476.0 ],
    [ 278.0, 494.0 ],
    [ 153.0, 400.0 ],
    [ 67.0,  400.0 ],
    [ 104.0, 512.0 ],
    [ 282.0, 576.0 ],
    [ 444.0, 529.0 ],
    [ 484.0, 430.0 ],
    [ 484.0, 117.0 ],
    [ 530.0, 75.0  ],
    [ 399.0, 289.0 ],
    [ 349.0, 273.0 ],
    [ 261.0, 258.0 ],
    [ 165.0, 228.0 ],
    [ 132.0, 161.0 ],
    [ 157.0, 101.0 ],
    [ 238.0, 75.0  ],
    [ 365.0, 124.0 ],
    [ 396.0, 169.0 ],
    [ 399.0, 193.0 ],
];

static EXPECTED_GLYPH_CONTROL_POINTS: [[f32; 2]; 29] = [
    [ 512.0, 0.0 ],
    [ 410.0, 0.0 ],
    [ 362.0, 43.0 ],
    [ 276.0, 0.0 ],
    [ 137.0, 0.0 ],
    [ 43.0, 86.0 ],
    [ 43.0, 262.0 ],
    [ 169.0, 314.0 ],
    [ 241.0, 325.0 ],
    [ 365.0, 340.0 ],
    [ 398.0, 366.0 ],
    [ 399.0, 457.0 ],
    [ 330.0, 494.0 ],
    [ 163.0, 494.0 ],
    [ 70.0, 474.0 ],
    [ 160.0, 576.0 ],
    [ 394.0, 576.0 ],
    [ 484.0, 493.0 ],
    [ 484.0, 75.0 ],
    [ 537.0, 75.0 ],
    [ 377.0, 278.0 ],
    [ 325.0, 268.0 ],
    [ 195.0, 249.0 ],
    [ 132.0, 205.0 ],
    [ 132.0, 125.0 ],
    [ 183.0, 75.0 ],
    [ 313.0, 75.0 ],
    [ 390.0, 147.0 ],
    [ 399.0, 177.0 ],
];

static EXPECTED_GLYPH_SUBPATHS: [Subpath; 2] = [
    Subpath {
        first_endpoint_index: 0,
        last_endpoint_index: 24,
    },
    Subpath {
        first_endpoint_index: 24,
        last_endpoint_index: 34,
    },
];

#[test]
fn test_font_context_glyph_dimensions() {
    let mut font_context = FontContext::new().unwrap();

    let font_key = FontKey::new();
    let mut bytes = vec![];
    File::open(TEST_FONT_PATH).unwrap().read_to_end(&mut bytes).unwrap();
    font_context.add_font_from_memory(&font_key, Arc::new(bytes), 0).unwrap();

    let font_instance = FontInstance::new(&font_key, TEST_FONT_SIZE);
    let glyph_key = GlyphKey::new(TEST_GLYPH_ID, SubpixelOffset(0));
    let glyph_dimensions = font_context.glyph_dimensions(&font_instance, &glyph_key).unwrap();

    assert_eq!(glyph_dimensions, GlyphDimensions {
        origin: EXPECTED_GLYPH_ORIGIN.into(),
        size: Size2D::new(EXPECTED_GLYPH_SIZE[0], EXPECTED_GLYPH_SIZE[1]),
        advance: EXPECTED_GLYPH_ADVANCE,
    })
}

#[test]
fn test_font_context_glyph_outline() {
    drop(env_logger::init());

    let mut font_context = FontContext::new().unwrap();

    let font_key = FontKey::new();
    let mut bytes = vec![];
    File::open(TEST_FONT_PATH).unwrap().read_to_end(&mut bytes).unwrap();
    font_context.add_font_from_memory(&font_key, Arc::new(bytes), 0).unwrap();

    let font_instance = FontInstance::new(&font_key, TEST_FONT_SIZE);
    let glyph_key = GlyphKey::new(TEST_GLYPH_ID, SubpixelOffset(0));
    let glyph_outline = font_context.glyph_outline(&font_instance, &glyph_key).unwrap();
    let mut glyph_outline_buffer = PathBuffer::new();
    glyph_outline_buffer.add_stream(glyph_outline);

    info!("endpoints: {:#?}", glyph_outline_buffer.endpoints);
    info!("control points: {:#?}", glyph_outline_buffer.control_points);

    assert_eq!(glyph_outline_buffer.endpoints.len(), EXPECTED_GLYPH_ENDPOINTS.len());
    for (expected_position, endpoint) in
            EXPECTED_GLYPH_ENDPOINTS.iter().zip(glyph_outline_buffer.endpoints.iter()) {
        let actual_position = endpoint.position;
        info!("expected endpoint: {:?} actual endpoint: {:?}", expected_position, actual_position);
        assert!(expected_position[0].approx_eq(&actual_position.x) &&
                expected_position[1].approx_eq(&actual_position.y));
    }

    assert_eq!(glyph_outline_buffer.control_points.len(), EXPECTED_GLYPH_CONTROL_POINTS.len());
    for (expected_position, actual_position) in
            EXPECTED_GLYPH_CONTROL_POINTS.iter().zip(glyph_outline_buffer.control_points.iter()) {
        info!("expected control point: {:?} actual control point: {:?}",
              expected_position,
              actual_position);
        assert!(expected_position[0].approx_eq(&actual_position.x) &&
                expected_position[1].approx_eq(&actual_position.y));
    }

    assert_eq!(glyph_outline_buffer.subpaths.len(), EXPECTED_GLYPH_SUBPATHS.len());
    for (expected_subpath, actual_subpath) in
            EXPECTED_GLYPH_SUBPATHS.iter().zip(glyph_outline_buffer.subpaths.iter()) {
        assert_eq!(expected_subpath.first_endpoint_index, actual_subpath.first_endpoint_index);
        assert_eq!(expected_subpath.last_endpoint_index, actual_subpath.last_endpoint_index);
    }
}
