use std::fs::File;
use std::env;
use std::path::{PathBuf, Path};
use phf_codegen::Map;

fn add_dir(map: &mut Map<String>, root: &Path, dir: Option<&Path>) {
    println!("{:?}", dir);
    let abs_dir = match dir {
        Some(p) => root.join(p),
        None => root.into()
    };
    for entry in abs_dir.read_dir().expect("not a directory") {
        let entry = entry.unwrap();
        let typ = entry.file_type().unwrap();
        let path = match dir {
            Some(p) => p.join(entry.file_name()),
            None => entry.file_name().into()
        };
        if typ.is_file() {
            let file_path = root.join(&path);
            map.entry(path.to_str().expect("non-utf8 filename").into(), &format!("include_bytes!({:?})", file_path));
        } else if typ.is_dir() {
            add_dir(map, root, Some(&path))
        }
    }
}

fn main() {
    let resources = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent().unwrap()
        .join("resources");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", resources.to_str().expect("no-utf8 path"));
    let file_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("resources_data.rs");
    
    let mut file = File::create(file_path).unwrap();
    let mut map = Map::new();

    #[cfg(feature="gl3_shaders")]
    add_dir(&mut map, &resources.join("shaders/gl3"), None);

    #[cfg(feature="metal_shaders")]
    add_dir(&mut map, &resources.join("shaders/metal"), None);

    #[cfg(feature="fonts")]
    add_dir(&mut map, &resources.join("fonts"), None);

    #[cfg(feature="debug-fonts")]
    add_dir(&mut map, &resources.join("debug-fonts"), None);

    #[cfg(feature="svg")]
    add_dir(&mut map, &resources.join("svg"), None);

    #[cfg(feature="textures")]
    add_dir(&mut map, &resources.join("textures"), None);

    map.build(&mut file).unwrap();
}
