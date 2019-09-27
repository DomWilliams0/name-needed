use scale;

use crate::chunk::Chunk;
use crate::coordinate::world::{BlockCoord, SliceBlock, SliceIndex};
use crate::viewer::SliceRange;
use crate::BLOCK_COUNT_CHUNK;

#[derive(Copy, Clone)]
pub struct Vertex {
    pub v_pos: [f32; 3],
    pub v_color: [f32; 3],
}

impl Vertex {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            v_pos: [x, y, z],
            v_color: [0.0, 0.0, 0.0],
        }
    }

    fn with_color(x: f32, y: f32, z: f32, rgb: (f32, f32, f32)) -> Self {
        Self {
            v_pos: [x, y, z],
            v_color: [rgb.0, rgb.1, rgb.2],
        }
    }
}

const VERTICES_PER_BLOCK: usize = 6;
pub const VERTICES_PER_CHUNK: usize = VERTICES_PER_BLOCK * BLOCK_COUNT_CHUNK;

// for ease of declaration. /2 as based around the center of the block
const X: f32 = scale::BLOCK / 2.0;

pub const BLOCK_VERTICES: [Vertex; 36] = [
    // front
    Vertex::new(-X, -X, -X),
    Vertex::new(-X, -X, X),
    Vertex::new(-X, X, X),
    Vertex::new(-X, X, X),
    Vertex::new(-X, X, -X),
    Vertex::new(-X, -X, -X),
    // left
    Vertex::new(X, -X, -X),
    Vertex::new(-X, -X, -X),
    Vertex::new(-X, X, -X),
    Vertex::new(-X, X, -X),
    Vertex::new(X, X, -X),
    Vertex::new(X, -X, -X),
    // right
    Vertex::new(-X, -X, X),
    Vertex::new(X, -X, X),
    Vertex::new(X, X, X),
    Vertex::new(X, X, X),
    Vertex::new(-X, X, X),
    Vertex::new(-X, -X, X),
    // top
    Vertex::new(-X, X, -X),
    Vertex::new(-X, X, X),
    Vertex::new(X, X, X),
    Vertex::new(X, X, X),
    Vertex::new(X, X, -X),
    Vertex::new(-X, X, -X),
    // bottom
    Vertex::new(X, -X, -X),
    Vertex::new(X, -X, X),
    Vertex::new(-X, -X, X),
    Vertex::new(-X, -X, X),
    Vertex::new(-X, -X, -X),
    Vertex::new(X, -X, -X),
    // back
    Vertex::new(X, X, -X),
    Vertex::new(X, X, X),
    Vertex::new(X, -X, X),
    Vertex::new(X, -X, X),
    Vertex::new(X, -X, -X),
    Vertex::new(X, X, -X),
];

#[allow(dead_code)]
#[derive(Copy, Clone)]
enum Face {
    Front,
    Left,
    Right,
    Top,
    Bottom,
    Back,
}

const FACE_COUNT: usize = 6;

/// For iteration
#[allow(dead_code)]
const FACES: [Face; FACE_COUNT] = [
    Face::Front,
    Face::Left,
    Face::Right,
    Face::Top,
    Face::Bottom,
    Face::Back,
];

pub fn make_mesh(chunk: &Chunk, slice_range: SliceRange) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(VERTICES_PER_CHUNK); // TODO reuse/calculate needed capacity first
    for slice_index in slice_range.into_iter() {
        let slice = chunk.slice(slice_index);
        // TODO skip if slice knows it is empty

        for (block_pos, block) in slice.non_air_blocks() {
            let (bx, by, bz) = {
                let SliceBlock(BlockCoord(x), BlockCoord(y)) = block_pos;
                let SliceIndex(slice_index) = slice_index;
                (
                    // +0.5 to render in the center of the block, which is the block mesh's origin
                    f32::from(x) + 0.5,
                    f32::from(y) + 0.5,
                    slice_index as f32,
                )
            };

            let color = block.color_as_f32();

            for face in 0..FACE_COUNT {
                let face_verts = {
                    let offset = 6 * face; // 6 vertices per face
                    &BLOCK_VERTICES[offset..offset + 6]
                };

                for vertex in face_verts.iter() {
                    let [fx, fy, fz] = vertex.v_pos;
                    vertices.push(Vertex::with_color(
                        fx + bx * scale::BLOCK,
                        fy + by * scale::BLOCK,
                        fz + bz * scale::BLOCK,
                        color,
                    ));
                }
            }
        }
    }

    vertices
}
