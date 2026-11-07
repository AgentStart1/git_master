use gpui::*;
use std::borrow::Cow;
pub struct Icons;
impl AssetSource for Icons {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        let bytes: &[u8] = match path {
            "trash" => include_bytes!("../../assets/icons/trash.svg"),
            "info-circle" => include_bytes!("../../assets/icons/info-circle.svg"),
            "alert-triangle" => include_bytes!("../../assets/icons/alert-triangle.svg"),
            "alert-circle" => include_bytes!("../../assets/icons/alert-circle.svg"),
            "circle-check" => include_bytes!("../../assets/icons/circle-check.svg"),
            "arrows-exchange" => include_bytes!("../../assets/icons/arrows-exchange.svg"),
            "terminal" => include_bytes!("../../assets/icons/terminal.svg"),
            "player-play" => include_bytes!("../../assets/icons/player-play.svg"),
            "player-pause" => include_bytes!("../../assets/icons/player-pause.svg"),
            "copy" => include_bytes!("../../assets/icons/copy.svg"),
            "calendar" => include_bytes!("../../assets/icons/calendar.svg"),
            "check" => include_bytes!("../../assets/icons/check.svg"),
            "chevron-down" => include_bytes!("../../assets/icons/chevron-down.svg"),
            "chevron-right" => include_bytes!("../../assets/icons/chevron-right.svg"),
            "cloud" => include_bytes!("../../assets/icons/cloud.svg"),
            "code" => include_bytes!("../../assets/icons/code.svg"),
            "download" => include_bytes!("../../assets/icons/download.svg"),
            "file" => include_bytes!("../../assets/icons/file.svg"),
            "folder" => include_bytes!("../../assets/icons/folder.svg"),
            "git-branch" => include_bytes!("../../assets/icons/git-branch.svg"),
            "git-fork" => include_bytes!("../../assets/icons/git-fork.svg"),
            "list" => include_bytes!("../../assets/icons/list.svg"),
            "refresh" => include_bytes!("../../assets/icons/refresh.svg"),
            "search" => include_bytes!("../../assets/icons/search.svg"),
            "stack-2" => include_bytes!("../../assets/icons/stack-2.svg"),
            "upload" => include_bytes!("../../assets/icons/upload.svg"),
            "user" => include_bytes!("../../assets/icons/user.svg"),
            "x" => include_bytes!("../../assets/icons/x.svg"),
            "minus" => include_bytes!("../../assets/icons/minus.svg"),
            "square" => include_bytes!("../../assets/icons/square.svg"),
            _ => return Ok(None),
        };
        Ok(Some(Cow::Borrowed(bytes)))
    }
    fn list(&self, _: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
pub fn icon(name: &'static str, size: f32) -> Svg {
    svg()
        .path(name)
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(super::theme::palette().text_primary))
}
