use crate::input::popup::content::{Button, EntityContent, TileSelectionContent};
use crate::{EcsWorld, Entity};

/// Single right click context menu
#[derive(Default)]
pub struct UiPopup {
    popup: Option<PopupContent>,
}

pub struct PreparedUiPopup<'a>(&'a mut UiPopup);

#[derive(Copy, Clone)]
pub enum PopupContentType {
    TileSelection,
    Entity(Entity),
}

pub struct PopupContent {
    ty: PopupContentType,
    content: Option<RenderedPopupContent>,
}

// TODO bump alloc
pub struct RenderedPopupContent {
    title: String,
    buttons: Vec<Button>,
}

trait RenderablePopup {
    fn prepare(&mut self, world: &EcsWorld) -> RenderedPopupContent;
}

impl UiPopup {
    /// Opened at mouse position
    pub fn open(&mut self, content: PopupContentType) {
        self.popup = Some(PopupContent {
            ty: content,
            content: None,
        });
    }

    fn on_close(&mut self) {
        self.popup = None;
    }

    /// Returns true if closed
    pub fn close(&mut self) -> bool {
        if self.popup.is_some() {
            self.popup = None;
            true
        } else {
            false
        }
    }

    /// Called once per frame by render system
    pub fn prepare(&mut self) -> PreparedUiPopup {
        PreparedUiPopup(self)
    }
}

impl RenderedPopupContent {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn buttons(&self) -> impl Iterator<Item = &Button> {
        self.buttons.iter()
    }
}

impl PopupContentType {
    fn prepare(&self, world: &EcsWorld) -> RenderedPopupContent {
        match self {
            PopupContentType::TileSelection => TileSelectionContent.prepare(world),
            PopupContentType::Entity(e) => EntityContent(*e).prepare(world),
        }
    }
}

impl PopupContent {
    pub fn as_renderable(&mut self, world: &EcsWorld) -> (&RenderedPopupContent, bool) {
        let open = if self.content.is_none() {
            // prepare for rendering
            self.content = Some(self.ty.prepare(world));
            true
        } else {
            false
        };

        debug_assert!(self.content.is_some());
        // safety: unconditionally set above
        let content = unsafe { self.content.as_ref().unwrap_unchecked() };

        (content, open)
    }
}

impl PreparedUiPopup<'_> {
    pub fn iter_all(&mut self) -> impl Iterator<Item = &mut PopupContent> + '_ {
        self.0.popup.as_mut().into_iter()
    }

    pub fn on_close(&mut self) {
        self.0.on_close()
    }
}

mod content {
    use unit::world::WorldPosition;

    use crate::ecs::*;
    use crate::input::popup::{RenderablePopup, RenderedPopupContent};

    pub enum ButtonType {
        GoTo(WorldPosition),
        CancelJob,
        HaulToSocietyStore,
    }

    pub enum ButtonState {
        Active,
        Disabled,
    }

    pub struct Button {
        ty: ButtonType,
        state: ButtonState,
    }

    pub struct EntityContent(pub Entity);
    pub struct TileSelectionContent;

    impl RenderablePopup for EntityContent {
        fn prepare(&mut self, world: &EcsWorld) -> RenderedPopupContent {
            let title = {
                let name = world.component::<NameComponent>(self.0);
                let kind = world.component::<KindComponent>(self.0);

                match (name, kind) {
                    (Ok(name), Ok(kind)) => format!("{} ({})", name, kind),
                    (Ok(name), Err(_)) => format!("{}", name),
                    (Err(_), Ok(kind)) => format!("{}", kind),
                    _ => format!("{}", self.0),
                }
            };

            let buttons = { Vec::new() };

            RenderedPopupContent { title, buttons }
        }
    }

    impl RenderablePopup for TileSelectionContent {
        fn prepare(&mut self, world: &EcsWorld) -> RenderedPopupContent {
            // TODO
            RenderedPopupContent {
                title: "Selection".to_string(),
                buttons: Vec::new(),
            }
        }
    }
}
