use std::path::PathBuf;

use glium::{BackfaceCullingMode, Depth, DepthTest, DrawParameters, PolygonMode};
use glium_sdl2::SDL2Facade;

pub use self::renderer::GliumRenderer;
pub use self::simulation::{FrameTarget, SimulationRenderer};

mod debug;
mod renderer;
mod simulation;

fn load_program(
    display: &SDL2Facade,
    key: &str,
) -> Result<glium::Program, glium::ProgramCreationError> {
    let root = {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("shaders");
        p.push(format!("{}.nop", key)); // ext will be substituted
        p
    };

    let v = std::fs::read_to_string(root.with_extension("glslv"))
        .expect("Failed to read vertex shader");
    let f = std::fs::read_to_string(root.with_extension("glslf"))
        .expect("Failed to read fragment shader");

    glium::Program::from_source(display, &v, &f, None)
}

static mut WIREFRAME_WORLD: bool = false;

pub unsafe fn wireframe_world_toggle() -> bool {
    WIREFRAME_WORLD = !WIREFRAME_WORLD;
    WIREFRAME_WORLD
}

#[derive(Copy, Clone)]
enum DrawParamType {
    World,
    Entity,
}

fn draw_params<'a>(draw_type: DrawParamType) -> DrawParameters<'a> {
    let polygon_mode = match draw_type {
        DrawParamType::World if unsafe { WIREFRAME_WORLD } => PolygonMode::Line,
        _ => PolygonMode::Fill,
    };

    DrawParameters {
        depth: Depth {
            test: DepthTest::IfLess,
            write: true,
            ..Default::default()
        },
        polygon_mode,
        backface_culling: BackfaceCullingMode::CullClockwise,
        ..Default::default()
    }
}
