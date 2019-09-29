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
        p.push(format!("{}.nop", key));
        p
    };

    let v = std::fs::read_to_string(root.with_extension("glslv")).unwrap();
    let f = std::fs::read_to_string(root.with_extension("glslf")).unwrap();

    glium::Program::from_source(display, &v, &f, None)
}

fn draw_params<'a>() -> DrawParameters<'a> {
    DrawParameters {
        depth: Depth {
            test: DepthTest::IfLess,
            write: true,
            ..Default::default()
        },
        polygon_mode: PolygonMode::Fill,
        backface_culling: BackfaceCullingMode::CullClockwise,
        ..Default::default()
    }
}
