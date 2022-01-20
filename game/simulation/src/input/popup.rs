/// Single right click context menu
#[derive(Default)]
pub struct UiPopup {
    popup: Option<PopupContent>,
    open: bool,
}

pub struct PreparedUiPopup<'a>(&'a mut UiPopup);

#[derive(Debug, Clone)]
pub enum PopupContent {
    // TODO actual popup content
    Test(String),
}

impl UiPopup {
    /// Opened at mouse position
    pub fn open(&mut self, content: PopupContent) {
        self.popup = Some(content);
        self.open = true;
    }

    fn on_close(&mut self) {
        self.popup = None;
    }

    /// Called once per frame by render system
    pub fn prepare(&mut self) -> PreparedUiPopup {
        PreparedUiPopup(self)
    }
}

impl PreparedUiPopup<'_> {
    /// (popup, should open this frame)
    pub fn iter_all(&self) -> impl Iterator<Item = (&PopupContent, bool)> + '_ {
        self.0
            .popup
            .as_ref()
            .into_iter()
            .map(move |content| (content, self.0.open))
    }

    pub fn on_close(&mut self) {
        self.0.on_close()
    }
}

impl Drop for PreparedUiPopup<'_> {
    fn drop(&mut self) {
        if self.0.open {
            self.0.open = false;
        }
    }
}
