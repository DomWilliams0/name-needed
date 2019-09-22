use glium::index::{NoIndices, PrimitiveType};
use glium::{implement_vertex, uniform, Surface};
use glium_sdl2::SDL2Facade;

use simulation::Position;

use crate::render::{load_program, FrameTarget};

pub struct DebugLine {
    pub from: Position,
    pub to: Position,
    pub color: (u8, u8, u8),
}

#[derive(Copy, Clone)]
pub struct DebugLineVertex {
    v_pos: [f32; 3],
    v_color: [f32; 3],
}

implement_vertex!(DebugLineVertex, v_pos, v_color);

pub struct DebugLines {
    pub lines: Vec<DebugLine>,

    geometry: glium::VertexBuffer<DebugLineVertex>,
    program: glium::Program,
}

impl DebugLines {
    pub fn new(display: &SDL2Facade) -> Self {
        Self {
            lines: Vec::new(),
            geometry: glium::VertexBuffer::empty_dynamic(display, 2).unwrap(),
            program: load_program(display, "lines").unwrap(),
        }
    }

    pub fn draw(&mut self, target: &mut FrameTarget) {
        let uniforms = uniform! {
            proj: target.projection,
            view: target.view,
        };

        // TODO instancing!

        for line in self.lines.drain(..) {
            let color = [
                // TODO color type Into f32x3
                f32::from(line.color.0) / 255.0,
                f32::from(line.color.1) / 255.0,
                f32::from(line.color.2) / 255.0,
            ];
            let vertices = [
                DebugLineVertex {
                    v_pos: [
                        line.from.x * scale::BLOCK,
                        line.from.y * scale::BLOCK,
                        line.from.z as f32 * scale::BLOCK,
                    ], // TODO Into f32x3
                    v_color: color,
                },
                DebugLineVertex {
                    v_pos: [
                        line.to.x * scale::BLOCK,
                        line.to.y * scale::BLOCK,
                        line.to.z as f32 * scale::BLOCK,
                    ],
                    v_color: color,
                },
            ];
            self.geometry.write(&vertices);
            target
                .frame
                .draw(
                    &self.geometry,
                    &NoIndices(PrimitiveType::LinesList),
                    &self.program,
                    &uniforms,
                    &Default::default(),
                )
                .unwrap();
        }
    }
}
