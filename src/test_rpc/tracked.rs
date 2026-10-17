use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::*;

pub type BoundsRegistry = Rc<RefCell<HashMap<String, Bounds<Pixels>>>>;

pub struct Tracked {
    track_id: String,
    inner: AnyElement,
    registry: BoundsRegistry,
    after_paint: Option<Box<dyn FnOnce(&mut App)>>,
}

pub fn tracked(id: &str, element: impl IntoElement, registry: &BoundsRegistry) -> Tracked {
    Tracked {
        track_id: id.to_string(),
        inner: element.into_any_element(),
        registry: registry.clone(),
        after_paint: None,
    }
}

impl Tracked {
    pub fn after_paint(mut self, callback: impl FnOnce(&mut App) + 'static) -> Self {
        self.after_paint = Some(Box::new(callback));
        self
    }
}

impl IntoElement for Tracked {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Tracked {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.inner.request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.registry
            .borrow_mut()
            .insert(self.track_id.clone(), bounds);
        self.inner.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.paint(window, cx);
        if let Some(callback) = self.after_paint.take() {
            cx.defer(callback);
        }
    }
}
