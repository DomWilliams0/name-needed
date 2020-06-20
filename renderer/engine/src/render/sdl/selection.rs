use sdl2::mouse::MouseButton;

use simulation::input::{InputEvent, SelectType, WorldColumn};

#[derive(Copy, Clone)]
enum MouseState {
    Unpressed,
    Down(SelectType, WorldColumn),
    Dragging(SelectType, WorldColumn),
}

pub struct Selection {
    state: MouseState,
}

impl Selection {
    pub fn mouse_down(&mut self, select: SelectType, pos: WorldColumn) {
        // dont bother about multiple buttons being held down at once
        if let MouseState::Unpressed = self.state {
            self.state = MouseState::Down(select, pos);
        }
    }
    pub fn mouse_up(&mut self, select: SelectType, pos: WorldColumn) -> Option<InputEvent> {
        let evt = match self.state {
            MouseState::Down(prev_select, _) if select == prev_select => {
                // single selection at the mouse up location, ignoring the down location
                let evt = InputEvent::Click(select, pos);
                Some(evt)
            }
            MouseState::Dragging(prev_select, start) if select == prev_select => {
                // region selection from original mouse down location
                let evt = InputEvent::Select(select, start, pos);
                Some(evt)
            }
            _ => None,
        };

        if evt.is_some() {
            // consume mouse press
            self.state = MouseState::Unpressed;
        }

        evt
    }

    pub fn mouse_move(&mut self, select: SelectType, pos: WorldColumn) {
        match self.state {
            MouseState::Down(prev_select, _) if prev_select == select => {
                // start dragging
                self.state = MouseState::Dragging(select, pos);
            }
            _ => {}
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
        Self {
            state: MouseState::Unpressed,
        }
    }
}
