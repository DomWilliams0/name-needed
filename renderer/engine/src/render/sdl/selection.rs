use sdl2::mouse::MouseButton;

use simulation::input::{InputEvent, SelectType, SelectionProgress, WorldColumn};

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

    pub fn mouse_up(&mut self, select: SelectType, pos: WorldColumn) -> Option<InputEvent> {
        let evt = match self.0 {
            MouseState::Down(prev_select) if select == prev_select => {
                // single selection at the mouse up location, ignoring the down location
                Some(InputEvent::Click(select, pos))
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

    pub fn mouse_move(&mut self, select: SelectType, pos: WorldColumn) -> Option<InputEvent> {
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
