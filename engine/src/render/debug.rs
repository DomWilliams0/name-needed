use std::cmp::{max, min};
use std::ops::Range;

use glium::index::{NoIndices, PrimitiveType};
use glium::uniforms::{AsUniformValue, Uniforms, UniformsStorage};
use glium::{implement_vertex, uniform, DrawParameters, PolygonMode, Surface};
use glium_sdl2::SDL2Facade;
use log::warn;

use world::WorldPoint;

use crate::render::{load_program, FrameTarget};

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
    pub fn color(&self) -> [f32; 3] {
        let (r, g, b) = *match self {
            DebugShape::Line { color, .. } => color,
            DebugShape::Tri { color, .. } => color,
        };

        // TODO rgb!
        [
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
        ]
    }

    pub fn points(&self) -> &[WorldPoint] {
        match self {
            DebugShape::Line { points, .. } => points,
            DebugShape::Tri { points, .. } => points,
        }
    }
}

#[derive(Copy, Clone)]
pub struct DebugShapeVertex {
    v_pos: [f32; 3],
    v_color: [f32; 3],
}

impl Default for DebugShapeVertex {
    fn default() -> Self {
        Self {
            v_pos: [0.0, 0.0, -100.0], // out of sight out of mind
            v_color: [0.0, 0.0, 0.0],
        }
    }
}

implement_vertex!(DebugShapeVertex, v_pos, v_color);

pub struct DebugShapes {
    pub shapes: Vec<DebugShape>,

    geometry: glium::VertexBuffer<DebugShapeVertex>,
    program: glium::Program,
    last_written_n: usize,
}

impl DebugShapes {
    const MAX_VERTICES: usize = 8192;

    pub fn new(display: &SDL2Facade) -> Self {
        let geometry =
            glium::VertexBuffer::empty_dynamic(display, DebugShapes::MAX_VERTICES).unwrap();
        Self {
            shapes: Vec::with_capacity(DebugShapes::MAX_VERTICES),
            geometry,
            program: load_program(display, "debug").unwrap(),
            last_written_n: 0,
        }
    }

    pub fn draw(&mut self, target: &mut FrameTarget) {
        assert_eq!(
            self.shapes.capacity(),
            DebugShapes::MAX_VERTICES,
            "either bump MAX_VERTICES or dont render so many shapes"
        );

        // no shapes, don't bother continuing
        if self.shapes.is_empty() {
            return;
        }

        let uniforms = uniform! {
            proj: target.projection,
            view: target.view,
        };

        // separate lines and tris
        let (first_tri, last_tri) = {
            // sort by shape
            // unstable sort to not allocate
            self.shapes.sort_unstable_by_key(|s| match s {
                DebugShape::Line { .. } => 0,
                DebugShape::Tri { .. } => 1,
            });

            let last_tri = max(self.shapes.len(), 1);

            let first_tri = self.shapes
                .iter()
                .position(|s| {
                    if let DebugShape::Tri { .. } = s {
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(last_tri);

            (first_tri, last_tri)
        };

        let mut n = self.last_written_n;
        n = self.draw_shapes(target, &uniforms, 0..first_tri, PrimitiveType::LinesList, n);

        // some triangles should be present
        if first_tri != last_tri {
            n = self.draw_shapes(
                target,
                &uniforms,
                first_tri..last_tri,
                PrimitiveType::TrianglesList,
                n,
            );
        }
        self.last_written_n = n;

        self.shapes.clear();
    }

    fn draw_shapes<T: AsUniformValue, R: Uniforms>(
        &mut self,
        target: &mut FrameTarget,
        uniforms: &UniformsStorage<T, R>,
        shape_range: Range<usize>,
        primitive: PrimitiveType,
        n_to_clear: usize,
    ) -> usize {
        self.geometry.invalidate();

        let n = {
            let mut buf = self.geometry.map_write();
            let mut buf_offset = 0usize;

            // wipe last write's data because we can't tell glDrawArrays to not use the full buffer size
            let n_to_clear = {
                // round up to nearest multiple of 6 (2x3), because the buffer is originally zeroed
                // but when we overwrite we use Default::default() which will set z=-100 to be off
                // screen...if this number isnt a multiple of 6 we will get black lines from 0,0,0
                // to this hidden off screen location.
                const MULTIPLE: usize = 6;
                let rounded = ((n_to_clear + MULTIPLE - 1) / MULTIPLE) * MULTIPLE;

                // cap at buf size of course
                min(rounded, DebugShapes::MAX_VERTICES)
            };
            for i in 0..n_to_clear {
                buf.set(i, Default::default());
            }

            let shapes = &self.shapes[shape_range];

            'done: for shape in shapes {
                for vertex in shape.points().iter().map(|p| {
                    let WorldPoint(x, y, z) = *p;
                    DebugShapeVertex {
                        v_pos: [
                            x * scale::BLOCK_DIAMETER,
                            y * scale::BLOCK_DIAMETER,
                            z * scale::BLOCK_DIAMETER,
                        ],
                        v_color: shape.color(),
                    }
                }) {
                    if buf_offset >= DebugShapes::MAX_VERTICES {
                        warn!(
                            "exceeded max number of debug vertices ({})",
                            DebugShapes::MAX_VERTICES
                        );
                        break 'done;
                    }

                    buf.set(buf_offset, vertex);
                    buf_offset += 1;
                }
            }

            buf_offset
            // mapping dropped here
        };

        // render
        let params = DrawParameters {
            polygon_mode: PolygonMode::Line,
            ..Default::default()
        };
        target
            .frame
            .draw(
                &self.geometry,
                &NoIndices(primitive),
                &self.program,
                uniforms,
                &params,
            )
            .unwrap();

        n
    }
}
