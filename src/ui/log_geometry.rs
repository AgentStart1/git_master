use gpui::*;
use std::cell::Cell;
use std::rc::Rc;

pub(super) type RowBounds = Rc<Cell<Option<Bounds<Pixels>>>>;

pub(super) struct Observed {
    inner: AnyElement,
    bounds: Option<RowBounds>,
    painted: Option<Box<dyn FnOnce(&mut App)>>,
}

// The owner clears the cell each frame so virtualized, unpainted rows have no bounds.
pub(super) fn row_bounds(element: impl IntoElement, bounds: RowBounds) -> Observed {
    Observed {
        inner: element.into_any_element(),
        bounds: Some(bounds),
        painted: None,
    }
}

pub(super) fn after_paint(
    element: impl IntoElement,
    painted: impl FnOnce(&mut App) + 'static,
) -> Observed {
    Observed {
        inner: element.into_any_element(),
        bounds: None,
        painted: Some(Box::new(painted)),
    }
}

impl IntoElement for Observed {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Observed {
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
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.inner.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(target) = &self.bounds {
            target.set(Some(bounds));
        }
        self.inner.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.paint(window, cx);
        if let Some(painted) = self.painted.take() {
            cx.defer(painted);
        }
    }
}
