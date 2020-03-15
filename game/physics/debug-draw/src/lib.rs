use std::convert::{TryFrom, TryInto};
use std::os::raw::c_void;

use color::ColorRgb;
use unit::world::WorldPoint;

pub struct DebugDrawer;

pub type FnDrawLine = unsafe extern "C" fn(
    frame_blob: *mut c_void,
    from: *const f32,
    to: *const f32,
    color: *const f32,
);

/// Holds references to closures that use the current frame's render state
/// Is passed through to C and back to rust (wild ride)
pub struct FrameBlob<'a> {
    pub draw_line: &'a mut dyn FnMut(WorldPoint, WorldPoint, ColorRgb),
}

/// # Safety
/// Called by C
pub unsafe extern "C" fn raw_draw_line(
    frame_blob: *mut c_void,
    from: *const f32,
    to: *const f32,
    color: *const f32,
) {
    let from = WorldPoint::try_from(std::slice::from_raw_parts(from, 3)).unwrap();
    let to = WorldPoint::try_from(std::slice::from_raw_parts(to, 3)).unwrap();
    let color = std::slice::from_raw_parts(color, 3)
        .try_into()
        .expect("color should be valid");

    let frame_blob: *mut FrameBlob = frame_blob.cast();
    let frame_blob: &mut FrameBlob = frame_blob.as_mut().unwrap();
    (frame_blob.draw_line)(from, to, color);
}
