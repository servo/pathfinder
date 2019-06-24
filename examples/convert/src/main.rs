use std::fs::File;
use std::io::{Read, BufWriter};
use std::error::Error;
use std::path::PathBuf;
use pathfinder_svg::BuiltSVG;
use pathfinder_pdf::make_pdf;
use usvg::{Tree, Options};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = PathBuf::from(args.next().expect("no input given"));
    let output = PathBuf::from(args.next().expect("no output given"));
    
    let mut data = Vec::new();
    File::open(input)?.read_to_end(&mut data)?;
    let svg = BuiltSVG::from_tree(Tree::from_data(&data, &Options::default()).unwrap());

    let scene = &svg.scene;
    let mut writer = BufWriter::new(File::create(&output)?);
    match output.extension().and_then(|s| s.to_str()) {
        Some("pdf") => make_pdf(&mut writer, scene),
        Some("ps") => scene.write_ps(&mut writer)?,
        _ => return Err("output filename must have .ps or .pdf extension".into())
    }
    Ok(())
}
