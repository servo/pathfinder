use pathfinder_renderer::{scene::Scene};
use pathfinder_geometry::{vector::Vector2F, rect::RectF};
use pathfinder_content::{outline::Outline, segment::{Segment, SegmentKind}, color::ColorF};
use std::io::Write;

mod pdf;
use pdf::Pdf;

pub struct PdfBuilder {
    pdf: Pdf
}

impl PdfBuilder {
    pub fn new() -> PdfBuilder {
        PdfBuilder {
            pdf: Pdf::new()
        }
    }
    
    pub fn add_scene(&mut self, scene: &Scene) {
        let view_box = scene.view_box();
        self.pdf.add_page(view_box.size());
        
        let height = view_box.size().y();
        let tr = |v: Vector2F| -> Vector2F {
            let r = v - view_box.origin();
            Vector2F::new(r.x(), height - r.y())
        };
        
        for (paint, outline) in scene.paths() {
            self.pdf.set_fill_color(paint.color);
            
            for contour in outline.contours() {
                for (segment_index, segment) in contour.iter().enumerate() {
                    if segment_index == 0 {
                        self.pdf.move_to(tr(segment.baseline.from()));
                    }

                    match segment.kind {
                        SegmentKind::None => {}
                        SegmentKind::Line => self.pdf.line_to(tr(segment.baseline.to())),
                        SegmentKind::Quadratic => {
                            let current = segment.baseline.from();
                            let c = segment.ctrl.from();
                            let p = segment.baseline.to();
                            let c1 = Vector2F::splat(2./3.) * c + Vector2F::splat(1./3.) * current;
                            let c2 = Vector2F::splat(2./3.) * c + Vector2F::splat(1./3.) * p;
                            self.pdf.cubic_to(c1, c2, p);
                        }
                        SegmentKind::Cubic => self.pdf.cubic_to(tr(segment.ctrl.from()), tr(segment.ctrl.to()), tr(segment.baseline.to()))
                    }
                }

                if contour.is_closed() {
                    self.pdf.close();
                }
            }
            
            // closes implicitly
            self.pdf.fill();
        }
    }
    
    pub fn write<W: Write>(mut self, out: W) {
        self.pdf.write_to(out);
    }
}

pub fn make_pdf<W: Write>(mut writer: W, scene: &Scene) {
    let mut pdf = PdfBuilder::new();
    pdf.add_scene(scene);
    pdf.write(writer);
}
