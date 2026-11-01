use super::tracked::BoundsRegistry;
use crate::app_state::GitMasterApp;
use gpui::WeakEntity;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, PartialEq)]
pub struct Description {
    pub message: String,
    pub detail: String,
    pub answers: Vec<String>,
    pub focused_answer: Option<usize>,
    pub detail_scroll_y: f32,
}
pub type PromptState = Rc<RefCell<Option<Description>>>;

#[derive(Clone)]
pub struct Observation {
    pub bounds: BoundsRegistry,
    pub state: PromptState,
    pub owner: WeakEntity<GitMasterApp>,
}
