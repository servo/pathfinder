use std::borrow::Cow;
use std::io::{Error as IOError, ErrorKind};
use pathfinder_gpu::resources::ResourceLoader;
use phf::Map;

pub struct EmbeddedResourceLoader;
static RESOURCES: Map<&'static str, &'static [u8]> = include!(concat!(env!("OUT_DIR"), "/", "resources_data.rs"));

impl ResourceLoader for EmbeddedResourceLoader {
    fn slurp(&self, virtual_path: &str) -> Result<Cow<'static, [u8]>, IOError> {
        match RESOURCES.get(virtual_path) {
            Some(&data) => Ok(data.into()),
            None => Err(ErrorKind::NotFound.into())
        }
    }
}