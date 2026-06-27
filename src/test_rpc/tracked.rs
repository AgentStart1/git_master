use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gpui::*;

pub type BoundsRegistry = Arc<Mutex<HashMap<String, Bounds<Pixels>>>>;

pub struct Tracked {
    track_id: String,
    inner: AnyElement,
    registry: BoundsRegistry,
}

pub fn tracked(
    id: &str,
    element: impl IntoElement,
    registry: &BoundsRegistry,
) -> Tracked {
    Tracked {
        track_id: id.to_string(),
        inner: element.into_any_element(),
        registry: registry.clone(),
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
        if let Ok(mut reg) = self.registry.lock() {
            reg.insert(self.track_id.clone(), bounds);
        }
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
    }
}
