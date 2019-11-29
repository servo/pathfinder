use pathfinder_gpu::resources::ResourceLoader;

/// A hacky loader that includes the resources we need in the binary.
///
/// On desktop, we can use FilesystemResourceLoader (see canvas_minimal), but
/// wasm32-unknown-unknown does not have a filesystem.
///
/// This does not have any platform dependencies, but will result in larger binaries, and slower
/// compile times, so you may wish to implement an approach with different trade-offs.
pub struct StaticResourceLoader;

impl ResourceLoader for StaticResourceLoader {
    fn slurp(&self, path: &str) -> Result<Vec<u8>, std::io::Error> {
        match path {
            "shaders/gl3/debug_solid.fs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/debug_solid.fs.glsl").to_vec())
            }
            "shaders/gl3/debug_solid.vs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/debug_solid.vs.glsl").to_vec())
            }
            "shaders/gl3/debug_texture.fs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/debug_texture.fs.glsl").to_vec())
            }
            "shaders/gl3/debug_texture.vs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/debug_texture.vs.glsl").to_vec())
            }
            "shaders/gl3/demo_ground.fs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/demo_ground.fs.glsl").to_vec())
            }
            "shaders/gl3/demo_ground.vs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/demo_ground.vs.glsl").to_vec())
            }
            "shaders/gl3/fill.fs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/fill.fs.glsl").to_vec())
            }
            "shaders/gl3/fill.vs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/fill.vs.glsl").to_vec())
            }
            "shaders/gl3/post.fs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/post.fs.glsl").to_vec())
            }
            "shaders/gl3/post.vs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/post.vs.glsl").to_vec())
            }
            "shaders/gl3/reproject.fs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/reproject.fs.glsl").to_vec())
            }
            "shaders/gl3/reproject.vs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/reproject.vs.glsl").to_vec())
            }
            "shaders/gl3/stencil.fs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/stencil.fs.glsl").to_vec())
            }
            "shaders/gl3/stencil.vs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/stencil.vs.glsl").to_vec())
            }
            "shaders/gl3/tile_alpha.fs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/tile_alpha.fs.glsl").to_vec())
            }
            "shaders/gl3/tile_alpha_monochrome.vs.glsl" => Ok(include_bytes!(
                "../../../resources/shaders/gl3/tile_alpha_monochrome.vs.glsl"
            )
            .to_vec()),
            "shaders/gl3/tile_alpha_multicolor.vs.glsl" => Ok(include_bytes!(
                "../../../resources/shaders/gl3/tile_alpha_multicolor.vs.glsl"
            )
            .to_vec()),
            "shaders/gl3/tile_solid.fs.glsl" => {
                Ok(include_bytes!("../../../resources/shaders/gl3/tile_solid.fs.glsl").to_vec())
            }
            "shaders/gl3/tile_solid_monochrome.vs.glsl" => Ok(include_bytes!(
                "../../../resources/shaders/gl3/tile_solid_monochrome.vs.glsl"
            )
            .to_vec()),
            "shaders/gl3/tile_solid_multicolor.vs.glsl" => Ok(include_bytes!(
                "../../../resources/shaders/gl3/tile_solid_multicolor.vs.glsl"
            )
            .to_vec()),

            "textures/area-lut.png" => {
                Ok(include_bytes!("../../../resources/textures/area-lut.png").to_vec())
            }
            "textures/debug-corner-fill.png" => {
                Ok(include_bytes!("../../../resources/textures/debug-corner-fill.png").to_vec())
            }
            "textures/debug-corner-outline.png" => {
                Ok(include_bytes!("../../../resources/textures/debug-corner-outline.png").to_vec())
            }
            "textures/debug-font.png" => {
                // TODO(joshuan): This is 12kb, and not needed for this demo. Make the renderer not
                // require this.
                Ok(include_bytes!("../../../resources/textures/debug-font.png").to_vec())
            }
            "textures/gamma-lut.png" => {
                Ok(include_bytes!("../../../resources/textures/gamma-lut.png").to_vec())
            }
            "debug-fonts/regular.json" => {
                Ok(include_bytes!("../../../resources/debug-fonts/regular.json").to_vec())
            }

            _ => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} is not included in this build.", path),
            )),
        }
    }
}
