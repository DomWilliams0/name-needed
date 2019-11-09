use std::cell::RefCell;
use std::rc::Rc;

use glium::index::PrimitiveType;
use glium::{implement_vertex, Surface};
use glium::{uniform, Frame};
use glium_sdl2::SDL2Facade;

use simulation::{Physical, Position, Renderer};

use crate::render::debug::{DebugShape, DebugShapes};
use crate::render::{draw_params, load_program};
use world::WorldPoint;

#[derive(Copy, Clone)]
struct EntityVertex {
    v_pos: [f32; 3],
}

implement_vertex!(EntityVertex, v_pos);

#[derive(Copy, Clone, Default)]
struct EntityInstanceAttributes {
    e_pos: [f32; 3],
    e_color: [f32; 3],
}

implement_vertex!(EntityInstanceAttributes, e_pos, e_color);

pub struct SimulationRenderer {
    program: glium::Program,
    entity_instances: Vec<(Position, Physical)>,
    entity_vertex_buf: glium::VertexBuffer<EntityInstanceAttributes>,
    entity_geometry: (glium::VertexBuffer<EntityVertex>, glium::IndexBuffer<u32>),

    // per frame
    // Option because unset until ``init`` is called each frame
    target: Option<Rc<RefCell<<Self as Renderer>::Target>>>,

    // debug
    debug_shapes: DebugShapes,
}

impl SimulationRenderer {
    pub fn new(display: &SDL2Facade) -> Self {
        let program = load_program(display, "entity").unwrap();

        // TODO entity count? maybe use "arraylist" vbos with big chunks e.g. 64
        let entity_instances = Vec::with_capacity(64);

        let entity_vertex_buf =
            glium::VertexBuffer::empty_dynamic(display, entity_instances.capacity()).unwrap();

        // simple square
        let entity_geometry = {
            let vertices = vec![
                EntityVertex {
                    v_pos: [0.0, 0.0, 0.0],
                },
                EntityVertex {
                    v_pos: [scale::HUMAN, 0.0, 0.0],
                },
                EntityVertex {
                    v_pos: [scale::HUMAN, scale::HUMAN, 0.0],
                },
                EntityVertex {
                    v_pos: [0.0, scale::HUMAN, 0.0],
                },
            ];

            let indices = vec![0, 1, 2, 2, 3, 0];

            (
                glium::VertexBuffer::new(display, &vertices).unwrap(),
                glium::IndexBuffer::new(display, PrimitiveType::TrianglesList, &indices).unwrap(),
            )
        };

        Self {
            program,
            entity_instances,
            entity_vertex_buf,
            entity_geometry,
            target: None,
            debug_shapes: DebugShapes::new(display),
        }
    }
}

pub struct FrameTarget {
    pub frame: Frame,
    pub projection: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
}

impl Renderer for SimulationRenderer {
    type Target = FrameTarget;

    fn init(&mut self, target: Rc<RefCell<Self::Target>>) {
        self.target = Some(target);
    }

    fn start(&mut self) {
        self.entity_instances.clear();
    }

    fn entity(&mut self, pos: &Position, physical: &Physical) {
        // TODO for safety until it can be expanded
        assert!(self.entity_instances.len() < self.entity_instances.capacity());
        self.entity_instances.push((*pos, *physical));
    }

    fn finish(&mut self) {
        {
            let mut target = self.target
                .as_ref()
                .expect("init was not called")
                .borrow_mut();

            // update instance attributes
            {
                let mut mapping = self.entity_vertex_buf.map();
                for (src, dest) in self.entity_instances.iter().zip(mapping.iter_mut()) {
                    // scale to camera space
                    dest.e_pos = [
                        src.0.x * scale::BLOCK_DIAMETER,
                        src.0.y * scale::BLOCK_DIAMETER,
                        src.0.z as f32 * scale::BLOCK_DIAMETER,
                    ];

                    let (r, g, b) = src.1.color;
                    dest.e_color = [
                        f32::from(r) / 255.0,
                        f32::from(g) / 255.0,
                        f32::from(b) / 255.0,
                    ];
                }
            }

            // render instances
            let uniforms = uniform! {
                proj: target.projection,
                view: target.view,
                instance_count: self.entity_instances.len() as i32,
            };

            let (verts, indices) = &self.entity_geometry;

            target
                .frame
                .draw(
                    (
                        verts,
                        self.entity_vertex_buf
                            .per_instance()
                            .expect("instancing unsupported"),
                    ),
                    indices,
                    &self.program,
                    &uniforms,
                    &draw_params(),
                )
                .unwrap();
        }

        self.target = None;
    }

    fn debug_add_line(&mut self, from: WorldPoint, to: WorldPoint, color: (u8, u8, u8)) {
        self.debug_shapes.shapes.push(DebugShape::Line {
            points: [from, to],
            color,
        })
    }

    fn debug_add_tri(&mut self, points: [WorldPoint; 3], color: (u8, u8, u8)) {
        self.debug_shapes
            .shapes
            .push(DebugShape::Tri { points, color })
    }

    fn debug_finish(&mut self) {
        let mut target = self.target
            .as_ref()
            .expect("init was not called")
            .borrow_mut();

        self.debug_shapes.draw(&mut target);
    }
}
