use color::ColorRgb;
use common::*;
use unit;

use crate::chunk::slab::{Slab, SLAB_SIZE};
use crate::chunk::Chunk;
use crate::coordinate::world::SliceBlock;
use crate::viewer::SliceRange;
use crate::CHUNK_SIZE;

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

    fn with_color(x: f32, y: f32, z: f32, rgb: ColorRgb) -> Self {
        Self {
            v_pos: [x, y, z],
            v_color: rgb.into(),
        }
    }
}

const VERTICES_PER_BLOCK: usize = 6;

// for ease of declaration. /2 for radius as this is based around the center of the block
const X: f32 = unit::BLOCK_DIAMETER / 2.0;

const BLOCK_VERTICES: [Vertex; 36] = [
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

pub fn make_render_mesh(chunk: &Chunk, slice_range: SliceRange) -> Vec<Vertex> {
    let mut vertices = Vec::new(); // TODO reuse/calculate needed capacity first
    for (slice_index, slice) in chunk.slice_range(slice_range) {
        // TODO skip if slice knows it is empty

        for (block_pos, block) in slice.non_air_blocks() {
            let height = block.block_height();

            let (bx, by, bz) = {
                let SliceBlock(x, y) = block_pos;
                let z = {
                    let z = slice_index.0 as f32;

                    // blocks that aren't full would be floating around the center, so lower to
                    // the bottom of the block
                    z - height.offset_from_center()
                };
                (
                    // +0.5 to render in the center of the block, which is the block mesh's origin
                    f32::from(x) + 0.5,
                    f32::from(y) + 0.5,
                    z,
                )
            };

            let color = block.block_type().color();
            let height = height.height();

            for face in 0..FACE_COUNT {
                let face_verts = {
                    let offset = 6 * face; // 6 vertices per face
                    &BLOCK_VERTICES[offset..offset + 6]
                };

                for vertex in face_verts.iter() {
                    let [fx, fy, fz] = vertex.v_pos;
                    vertices.push(Vertex::with_color(
                        fx + bx * unit::BLOCK_DIAMETER,
                        fy + by * unit::BLOCK_DIAMETER,
                        (fz * height) + bz * unit::BLOCK_DIAMETER,
                        color,
                    ));
                }
            }
        }
    }

    vertices
}

/// Compile time `min`...
const fn min_const(a: usize, b: usize) -> usize {
    [a, b][(a > b) as usize]
}

#[allow(clippy::many_single_char_names)]
/// Based off this[0] and its insane javascript implementation[1]. An attempt was made to make it
/// more idiomatic and less dense but it stops working in subtle ways so I'm leaving it at this :^)
///  - [0] https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/
///  - [1] https://github.com/mikolalysenko/mikolalysenko.github.com/blob/master/MinecraftMeshes/js/greedy.js
pub(crate) fn make_collision_mesh(
    slab: &Slab,
    out_vertices: &mut Vec<f32>,
    out_indices: &mut Vec<u32>,
) {
    let is_solid = |coord: &[i32; 3]| {
        let coord = [coord[0] as i32, coord[1] as i32, coord[2] as i32];
        slab.grid()[&coord].solid()
    };

    let mut add_vertex = |x: i32, y: i32, z: i32| {
        let old_size = out_vertices.len();
        out_vertices.extend(&[x as f32, y as f32, z as f32]);
        old_size
    };

    // TODO half blocks

    let dims = [CHUNK_SIZE.as_i32(), CHUNK_SIZE.as_i32(), SLAB_SIZE.as_i32()];
    let mut mask = {
        // reuse the same array for each mask, so calculate the min size it needs to be
        const CHUNK_SZ: usize = CHUNK_SIZE.as_usize();
        const SLAB_SZ: usize = SLAB_SIZE.as_usize();
        const FULL_COUNT: usize = CHUNK_SZ * CHUNK_SZ * SLAB_SZ;
        const MIN_DIM: usize = min_const(CHUNK_SZ, SLAB_SZ);
        [false; FULL_COUNT / MIN_DIM]
    };

    for d in 0..3 {
        let u = (d + 1) % 3;
        let v = (d + 2) % 3;

        let mask_size = dims[u] * dims[v];

        // unit vector from current direction
        let mut q = [0; 3];
        q[d] = 1;

        // iterate in slices in dimension direction
        let mut x = [0; 3];
        let mut xd = -1i32;
        while xd < dims[d] {
            x[d] = xd;

            // compute mask
            let mut n = 0;
            for xv in 0..dims[v] {
                x[v] = xv;

                for xu in 0..dims[u] {
                    x[u] = xu;
                    let solid_this = if xd >= 0 { is_solid(&x) } else { false };
                    let solid_other = if xd < dims[d] - 1 {
                        is_solid(&[x[0] + q[0], x[1] + q[1], x[2] + q[2]])
                    } else {
                        false
                    };
                    mask[n] = solid_this != solid_other;
                    n += 1;
                }
            }

            x[d] += 1;
            xd += 1;

            // generate mesh
            n = 0;
            for j in 0..dims[v] {
                let mut i = 0;
                while i < dims[u] {
                    if mask[n] {
                        // width
                        let mut w = 1i32;
                        while mask[n + w as usize] && i + w < dims[u] {
                            w += 1;
                        }

                        // height
                        let mut h = 1;
                        let mut done = false;
                        while j + h < dims[v] {
                            for k in 0..w {
                                if !mask[n + k as usize + (h * dims[u]) as usize] {
                                    done = true;
                                    break;
                                }
                            }

                            if done {
                                break;
                            }

                            h += 1;
                        }

                        // create quad
                        {
                            let (b, du, dv) = {
                                let mut quad_pos = x;
                                quad_pos[u] = i;
                                quad_pos[v] = j;

                                let mut quad_width = [0i32; 3];
                                quad_width[u] = w as i32;

                                let mut quad_height = [0i32; 3];
                                quad_height[v] = h;

                                trace!(
                                    "adding quad at {:?} of size {:?}x{:?}",
                                    quad_pos,
                                    quad_width,
                                    quad_height
                                );

                                (quad_pos, quad_width, quad_height)
                            };

                            // add quad vertices
                            let idx = add_vertex(b[0], b[1], b[2]);
                            add_vertex(b[0] + du[0], b[1] + du[1], b[2] + du[2]);
                            add_vertex(
                                b[0] + du[0] + dv[0],
                                b[1] + du[1] + dv[1],
                                b[2] + du[2] + dv[2],
                            );
                            add_vertex(b[0] + dv[0], b[1] + dv[1], b[2] + dv[2]);

                            // add indices
                            let vs = idx as u32 / 3;
                            let indices = [vs + 0, vs + 1, vs + 2, vs + 2, vs + 3, vs + 0];
                            out_indices.extend_from_slice(&indices);
                        }

                        // __partly__ zero mask
                        for l in 0..h {
                            for k in 0..w {
                                mask[n + k as usize + (l * dims[u]) as usize] = false;
                            }
                        }
                        i += w;
                        n += w as usize;
                    } else {
                        i += 1;
                        n += 1;
                    }
                }
            }
        }

        // fully zero mask for next dimension
        mask.iter_mut().for_each(|i| *i = false);
    }
}

#[cfg(test)]
mod tests {
    use crate::block::BlockType;
    use crate::chunk::slab::Slab;
    use crate::mesh::make_collision_mesh;

    #[test]
    fn greedy_single_block() {
        let slab = {
            let mut slab = Slab::empty(0);
            slab.slice_mut(0).set_block((0, 0), BlockType::Stone);
            slab
        };

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        make_collision_mesh(&slab, &mut vertices, &mut indices);

        assert_eq!(
            vertices.len(),
            6 /* 6 quads */ * 4 /* 4 verts per quad */ * 3 /* x,y,z per vert */
        );
        assert_eq!(
            indices.len(),
            6 /* 6 quads */ * 6 /* 6 indices per quad */
        );
    }

    #[test]
    fn greedy_column() {
        let slab = {
            let mut slab = Slab::empty(0);
            slab.slice_mut(1).set_block((1, 1), BlockType::Stone);
            slab.slice_mut(2).set_block((1, 1), BlockType::Stone);
            slab
        };

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        make_collision_mesh(&slab, &mut vertices, &mut indices);

        // same as single block above
        assert_eq!(vertices.len(), 6 * 4 * 3);
        assert_eq!(indices.len(), 6 * 6);
    }

    #[test]
    fn greedy_plane() {
        let slab = {
            let mut slab = Slab::empty(0);
            slab.slice_mut(0).fill(BlockType::Stone);
            slab.slice_mut(1).set_block((1, 1), BlockType::Grass);
            slab
        };

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        make_collision_mesh(&slab, &mut vertices, &mut indices);
        assert_eq!(vertices.len(), 168); // more of a regression test
        assert_eq!(indices.len(), 84);
    }
}
