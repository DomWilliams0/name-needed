use sdl2::keyboard::Mod;
use sdl2::mouse::MouseButton;

use simulation::input::{InputEvent, InputModifier, SelectType, SelectionProgress, WorldColumn};

#[derive(Copy, Clone)]
enum MouseState {
    Unpressed,
    Down(SelectType),
    Dragging {
        select: SelectType,
        start: WorldColumn,
    },
}

pub struct Selection(MouseState);

impl Selection {
    pub fn mouse_down(&mut self, select: SelectType, _pos: WorldColumn) {
        // dont bother about multiple buttons being held down at once
        if let MouseState::Unpressed = self.0 {
            self.0 = MouseState::Down(select)
        }
    }

    pub fn mouse_up(
        &mut self,
        select: SelectType,
        pos: WorldColumn,
        modifiers: Mod,
    ) -> Option<InputEvent> {
        let evt = match self.0 {
            MouseState::Down(prev_select) if select == prev_select => {
                // single selection at the mouse up location, ignoring the down location
                Some(InputEvent::Click(select, pos, parse_modifiers(modifiers)))
            }
            MouseState::Dragging {
                select: prev_select,
                start,
            } if select == prev_select => {
                // region selection from original mouse down location
                Some(InputEvent::Select {
                    select,
                    from: start,
                    to: pos,
                    progress: SelectionProgress::Complete,
                    modifiers: parse_modifiers(modifiers),
                })
            }
            _ => None,
        };

        if evt.is_some() {
            // consume mouse press
            self.0 = MouseState::Unpressed;
        }

        evt
    }

    pub fn mouse_move(
        &mut self,
        select: SelectType,
        pos: WorldColumn,
        modifiers: Mod,
    ) -> Option<InputEvent> {
        match self.0 {
            MouseState::Down(prev_select) if prev_select == select => {
                // start dragging
                self.0 = MouseState::Dragging { select, start: pos };
                None
            }
            MouseState::Dragging {
                select: prev_select,
                start,
            } if prev_select == select => Some(InputEvent::Select {
                select,
                from: start,
                to: pos,
                progress: SelectionProgress::InProgress,
                modifiers: parse_modifiers(modifiers),
            }),
            _ => None,
        }
    }

    pub fn select_type(btn: MouseButton) -> Option<SelectType> {
        match btn {
            MouseButton::Left => Some(SelectType::Left),
            MouseButton::Right => Some(SelectType::Right),
            _ => None,
        }
    }
}

impl Default for Selection {
    fn default() -> Self {
        Selection(MouseState::Unpressed)
    }
}

fn parse_modifiers(modifiers: Mod) -> InputModifier {
    let mut out = InputModifier::empty();

    if modifiers.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) {
        out.insert(InputModifier::CTRL);
    }

    if modifiers.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
        out.insert(InputModifier::SHIFT);
    }

    if modifiers.intersects(Mod::LALTMOD | Mod::RALTMOD) {
        out.insert(InputModifier::ALT);
    }

    out
}
