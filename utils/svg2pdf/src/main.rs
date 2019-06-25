use std::fs::File;
use std::io::{Read, BufWriter};
use std::error::Error;
use pathfinder_svg::BuiltSVG;
use pathfinder_pdf::make_pdf;
use usvg::{Tree, Options};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("no input given");
    let output = args.next().expect("no output given");
    
    let mut data = Vec::new();
    File::open(input)?.read_to_end(&mut data)?;
    let svg = BuiltSVG::from_tree(Tree::from_data(&data, &Options::default()).unwrap());

    make_pdf(BufWriter::new(File::create(output)?), &svg.scene);
    
    Ok(())
}
