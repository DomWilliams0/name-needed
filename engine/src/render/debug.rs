use glium::index::{NoIndices, PrimitiveType};
use glium::{implement_vertex, uniform, DrawParameters, PolygonMode, Surface};
use glium_sdl2::SDL2Facade;

use crate::render::{load_program, FrameTarget};
use world::WorldPoint;

pub enum DebugShape {
    Line {
        points: [WorldPoint; 2],
        color: (u8, u8, u8),
    },
    Tri {
        points: [WorldPoint; 3],
        color: (u8, u8, u8),
    },
}

impl DebugShape {
    pub fn color(&self) -> &(u8, u8, u8) {
        match self {
            DebugShape::Line { color, .. } => color,
            DebugShape::Tri { color, .. } => color,
        }
    }

    pub fn points(&self) -> &[WorldPoint] {
        match self {
            DebugShape::Line { points, .. } => points,
            DebugShape::Tri { points, .. } => points,
        }
    }

    pub fn primitive(&self) -> PrimitiveType {
        match self {
            DebugShape::Line { .. } => PrimitiveType::LinesList,
            DebugShape::Tri { .. } => PrimitiveType::TrianglesList,
        }
    }

    pub fn vertex_count(&self) -> usize {
        match self {
            DebugShape::Line { .. } => 2,
            DebugShape::Tri { .. } => 3,
        }
    }
}

#[derive(Copy, Clone)]
pub struct DebugLineVertex {
    v_pos: [f32; 3],
    v_color: [f32; 3],
}

implement_vertex!(DebugLineVertex, v_pos, v_color);

pub struct DebugShapes {
    pub shapes: Vec<DebugShape>,

    geometry: glium::VertexBuffer<DebugLineVertex>,
    program: glium::Program,
    vertex_buf: Vec<DebugLineVertex>,
}

impl DebugShapes {
    pub fn new(display: &SDL2Facade) -> Self {
        Self {
            shapes: Vec::new(),
            geometry: glium::VertexBuffer::empty_dynamic(display, 3).unwrap(),
            program: load_program(display, "lines").unwrap(),
            vertex_buf: Vec::new(),
        }
    }

    pub fn draw(&mut self, target: &mut FrameTarget) {
        let uniforms = uniform! {
            proj: target.projection,
            view: target.view,
        };

        // TODO instancing!

        for shape in self.shapes.iter() {
            let color = {
                let color = shape.color();
                [
                    // TODO color type Into f32x3
                    f32::from(color.0) / 255.0,
                    f32::from(color.1) / 255.0,
                    f32::from(color.2) / 255.0,
                ]
            };

            self.vertex_buf.clear();
            self.vertex_buf.extend(shape.points().iter().map(|p| {
                let WorldPoint(x, y, z) = p;
                DebugLineVertex {
                    v_pos: [x * scale::BLOCK, y * scale::BLOCK, z * scale::BLOCK], // TODO Into f32x3
                    v_color: color,
                }
            }));

            self.geometry
                .slice_mut(0..shape.vertex_count())
                .unwrap()
                .write(&self.vertex_buf);

            let params = DrawParameters {
                polygon_mode: PolygonMode::Line,
                ..Default::default()
            };
            target
                .frame
                .draw(
                    &self.geometry,
                    &NoIndices(shape.primitive()),
                    &self.program,
                    &uniforms,
                    &params,
                )
                .unwrap();
        }

        self.shapes.clear();
    }
}
