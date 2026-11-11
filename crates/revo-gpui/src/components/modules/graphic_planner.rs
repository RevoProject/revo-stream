use revo_core::planner::{
    centered_position, fit_scale, parse_resolution, stretch_scale, DEFAULT_SCENE_RESOLUTION,
};

use gpui::{div, prelude::*, px, Context, IntoElement, Render, SharedString, Window};

use crate::theme::RevoTheme;

const MIN_SIZE: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlannerOp {
    Fit,
    Stretch,
    CenterH,
    CenterV,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlannerTransform {
    pub pos_x: f32,
    pub pos_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation: f32,
}

impl Default for PlannerTransform {
    fn default() -> Self {
        Self {
            pos_x: 100.0,
            pos_y: 100.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        }
    }
}

pub fn normalize_angle(angle: f32) -> f32 {
    let mut next = angle % 360.0;
    if next < 0.0 {
        next += 360.0;
    }
    next
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlannerSource {
    pub id: String,
    pub name: String,
    pub base_width: u32,
    pub base_height: u32,
    pub transform: PlannerTransform,
}

impl PlannerSource {
    pub fn item_width(&self) -> f32 {
        self.base_width as f32 * self.transform.scale_x
    }

    pub fn item_height(&self) -> f32 {
        self.base_height as f32 * self.transform.scale_y
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannerApplyItem {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub rotation: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlannerEvent {
    Select(String),
    Op { id: String, op: PlannerOp },
    Apply { items: Vec<PlannerApplyItem> },
    Cancel,
}

pub struct GraphicPlannerPanel {
    pub sources: Vec<PlannerSource>,
    pub selected_id: Option<String>,
    pub canvas: String,
    pub open: bool,
    pub theme: RevoTheme,
}

impl GraphicPlannerPanel {
    pub fn canvas_resolution(&self) -> (u32, u32) {
        parse_resolution(&self.canvas).unwrap_or(DEFAULT_SCENE_RESOLUTION)
    }

    pub fn set_canvas(&mut self, canvas: impl Into<String>) {
        self.canvas = canvas.into();
    }

    pub fn select(&mut self, id: &str) -> Option<PlannerEvent> {
        if !self.sources.iter().any(|source| source.id == id) {
            return None;
        }
        self.selected_id = Some(id.to_owned());
        Some(PlannerEvent::Select(id.to_owned()))
    }

    pub fn fit_to_canvas(&mut self, id: &str) -> Option<PlannerEvent> {
        let (canvas_width, canvas_height) = self.canvas_resolution();
        let source = self.source_mut(id)?;
        let scale = fit_scale(
            source.base_width,
            source.base_height,
            canvas_width,
            canvas_height,
        )?;
        source.transform.scale_x = scale;
        source.transform.scale_y = scale;
        let (x, y) = centered_position(
            canvas_width,
            canvas_height,
            source.item_width(),
            source.item_height(),
        );
        source.transform.pos_x = x;
        source.transform.pos_y = y;
        Some(PlannerEvent::Op {
            id: id.to_owned(),
            op: PlannerOp::Fit,
        })
    }

    pub fn stretch_to_canvas(&mut self, id: &str) -> Option<PlannerEvent> {
        let (canvas_width, canvas_height) = self.canvas_resolution();
        let source = self.source_mut(id)?;
        let (scale_x, scale_y) = stretch_scale(
            source.base_width,
            source.base_height,
            canvas_width,
            canvas_height,
        )?;
        source.transform.scale_x = scale_x;
        source.transform.scale_y = scale_y;
        let (x, y) = centered_position(
            canvas_width,
            canvas_height,
            source.item_width(),
            source.item_height(),
        );
        source.transform.pos_x = x;
        source.transform.pos_y = y;
        Some(PlannerEvent::Op {
            id: id.to_owned(),
            op: PlannerOp::Stretch,
        })
    }

    pub fn center_horizontal(&mut self, id: &str) -> Option<PlannerEvent> {
        let (canvas_width, _) = self.canvas_resolution();
        let source = self.source_mut(id)?;
        let (x, _) = centered_position(canvas_width, 1, source.item_width(), 1.0);
        source.transform.pos_x = x;
        Some(PlannerEvent::Op {
            id: id.to_owned(),
            op: PlannerOp::CenterH,
        })
    }

    pub fn center_vertical(&mut self, id: &str) -> Option<PlannerEvent> {
        let (_, canvas_height) = self.canvas_resolution();
        let source = self.source_mut(id)?;
        let (_, y) = centered_position(1, canvas_height, 1.0, source.item_height());
        source.transform.pos_y = y;
        Some(PlannerEvent::Op {
            id: id.to_owned(),
            op: PlannerOp::CenterV,
        })
    }

    pub fn apply(&mut self) -> PlannerEvent {
        let (canvas_width, canvas_height) = self.canvas_resolution();
        let max_width = canvas_width as f32;
        let max_height = canvas_height as f32;
        let items = self
            .sources
            .iter()
            .map(|source| {
                let mut width = source.item_width().clamp(MIN_SIZE, max_width);
                let mut height = source.item_height().clamp(MIN_SIZE, max_height);
                let mut x = source
                    .transform
                    .pos_x
                    .clamp(0.0, (max_width - width).max(0.0));
                let mut y = source
                    .transform
                    .pos_y
                    .clamp(0.0, (max_height - height).max(0.0));
                let rotation = normalize_angle(source.transform.rotation);
                if x.is_nan() {
                    x = 0.0;
                }
                if y.is_nan() {
                    y = 0.0;
                }
                if width.is_nan() {
                    width = MIN_SIZE;
                }
                if height.is_nan() {
                    height = MIN_SIZE;
                }
                PlannerApplyItem {
                    id: source.id.clone(),
                    x: x.round() as i32,
                    y: y.round() as i32,
                    width: width.round() as i32,
                    height: height.round() as i32,
                    rotation: rotation.round() as i32,
                }
            })
            .collect();
        PlannerEvent::Apply { items }
    }

    pub fn source(&self, id: &str) -> Option<&PlannerSource> {
        self.sources.iter().find(|source| source.id == id)
    }

    fn source_mut(&mut self, id: &str) -> Option<&mut PlannerSource> {
        self.sources.iter_mut().find(|source| source.id == id)
    }
}

impl gpui::EventEmitter<PlannerEvent> for GraphicPlannerPanel {}

impl Render for GraphicPlannerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let (canvas_width, canvas_height) = self.canvas_resolution();
        let toolbar = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_2()
            .p_2()
            .border_1()
            .border_color(self.theme.border)
            .rounded_lg()
            .bg(self.theme.surface)
            .child("Graphic Planner")
            .child(
                div()
                    .id("planner-fit")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(self.theme.surface_active)
                    .child("Fit to canvas")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let id = this.selected_id.clone();
                        if let Some(id) = id {
                            if let Some(event) = this.fit_to_canvas(&id) {
                                cx.emit(event);
                            }
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("planner-stretch")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(self.theme.surface_active)
                    .child("Stretch to canvas")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let id = this.selected_id.clone();
                        if let Some(id) = id {
                            if let Some(event) = this.stretch_to_canvas(&id) {
                                cx.emit(event);
                            }
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("planner-center-h")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(self.theme.surface_active)
                    .child("Center H")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let id = this.selected_id.clone();
                        if let Some(id) = id {
                            if let Some(event) = this.center_horizontal(&id) {
                                cx.emit(event);
                            }
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("planner-center-v")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(self.theme.surface_active)
                    .child("Center V")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let id = this.selected_id.clone();
                        if let Some(id) = id {
                            if let Some(event) = this.center_vertical(&id) {
                                cx.emit(event);
                            }
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("planner-save")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(self.theme.accent)
                    .text_color(self.theme.text_strong)
                    .child("Save")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let event = this.apply();
                        cx.emit(event);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("planner-close")
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(30.))
                    .h(px(30.))
                    .rounded_md()
                    .bg(self.theme.surface_active)
                    .hover(|style| style.bg(self.theme.danger))
                    .child(crate::components::icon::icon(self.theme.text, "close", 14.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open = false;
                        cx.emit(PlannerEvent::Cancel);
                        cx.notify();
                    })),
            );

        let mut boxes = div()
            .gap_2()
            .child(div().text_color(self.theme.text_muted).child(format!(
                "Canvas: {}x{} ({})",
                canvas_width,
                canvas_height,
                if parse_resolution(&self.canvas).is_some() {
                    "configured"
                } else {
                    "default"
                }
            )));
        for source in self.sources.clone() {
            let selected = self.selected_id.as_deref() == Some(&source.id);
            boxes = boxes.child(
                div()
                    .id(SharedString::from(format!("planner-source-{}", source.id)))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .p_2()
                    .rounded_md()
                    .bg(if selected {
                        self.theme.surface_selected
                    } else {
                        self.theme.surface_elevated
                    })
                    .border_1()
                    .border_color(if selected {
                        self.theme.border_selected
                    } else {
                        self.theme.border
                    })
                    .child(div().child(source.name.clone()))
                    .child(div().text_color(self.theme.text_muted).child(format!(
                        "x={} y={} scale=({:.2}, {:.2}) rot={}",
                        source.transform.pos_x.round(),
                        source.transform.pos_y.round(),
                        source.transform.scale_x,
                        source.transform.scale_y,
                        normalize_angle(source.transform.rotation),
                    )))
                    .on_click(cx.listener({
                        let id = source.id.clone();
                        move |this, _, _, cx| {
                            if let Some(event) = this.select(&id) {
                                cx.emit(event);
                            }
                            cx.notify();
                        }
                    })),
            );
        }

        crate::components::panel(self.theme)
            .child(toolbar)
            .child(boxes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_640x480() -> PlannerSource {
        PlannerSource {
            id: "camera".into(),
            name: "Camera".into(),
            base_width: 640,
            base_height: 480,
            transform: PlannerTransform::default(),
        }
    }

    fn panel() -> GraphicPlannerPanel {
        GraphicPlannerPanel {
            sources: vec![source_640x480()],
            selected_id: Some("camera".into()),
            canvas: "1920x1080".into(),
            open: true,
            theme: RevoTheme::future(),
        }
    }

    #[test]
    fn fit_uses_core_fit_scale_and_centering() {
        let mut planner = panel();
        let event = planner.fit_to_canvas("camera");
        assert_eq!(
            event,
            Some(PlannerEvent::Op {
                id: "camera".into(),
                op: PlannerOp::Fit,
            })
        );
        let source = planner.source("camera").unwrap();
        assert_eq!(fit_scale(640, 480, 1920, 1080), Some(1.0),);
        assert_eq!(source.transform.scale_x, 1.0);
        assert_eq!(source.transform.scale_y, 1.0);
        assert_eq!(source.transform.pos_x, 640.0);
        assert_eq!(source.transform.pos_y, 300.0);
    }

    #[test]
    fn fit_downscales_oversized_sources() {
        let mut planner = panel();
        planner.canvas = "320x240".into();
        planner.fit_to_canvas("camera");
        assert_eq!(planner.source("camera").unwrap().transform.scale_x, 0.5);
        assert_eq!(planner.source("camera").unwrap().transform.pos_x, 0.0);
        assert_eq!(planner.source("camera").unwrap().transform.pos_y, 0.0);
    }

    #[test]
    fn stretch_uses_core_stretch_scale() {
        let mut planner = panel();
        let event = planner.stretch_to_canvas("camera");
        assert_eq!(
            event,
            Some(PlannerEvent::Op {
                id: "camera".into(),
                op: PlannerOp::Stretch,
            })
        );
        let source = planner.source("camera").unwrap();
        assert_eq!(source.transform.scale_x, 3.0);
        assert_eq!(source.transform.scale_y, 2.25);
        assert_eq!(source.transform.pos_x, 0.0);
        assert_eq!(source.transform.pos_y, 0.0);
    }

    #[test]
    fn center_horizontal_and_vertical_use_core_centering() {
        let mut planner = panel();
        planner.center_horizontal("camera");
        planner.center_vertical("camera");
        let source = planner.source("camera").unwrap();
        assert_eq!(source.transform.pos_x, 640.0);
        assert_eq!(source.transform.pos_y, 300.0);
    }

    #[test]
    fn canvas_resolution_parses_and_rejects_with_default_fallback() {
        let mut planner = panel();
        assert_eq!(planner.canvas_resolution(), (1920, 1080));
        planner.canvas = "1280x720".into();
        assert_eq!(planner.canvas_resolution(), (1280, 720));
        planner.canvas = "bogus".into();
        assert_eq!(planner.canvas_resolution(), DEFAULT_SCENE_RESOLUTION);

        assert_eq!(parse_resolution("0x1080"), None);
        assert_eq!(parse_resolution("1920x"), None);
        assert_eq!(parse_resolution("axb"), None);
    }

    #[test]
    fn apply_rounds_and_clamps_like_svelte_payload() {
        let mut planner = panel();
        planner.sources[0].transform.scale_x = 3.0;
        planner.sources[0].transform.scale_y = 2.25;
        planner.sources[0].transform.pos_x = 10.4;
        planner.sources[0].transform.pos_y = 20.6;
        planner.sources[0].transform.rotation = -370.0;
        let event = planner.apply();
        assert_eq!(
            event,
            PlannerEvent::Apply {
                items: vec![PlannerApplyItem {
                    id: "camera".into(),
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                    rotation: 350,
                }],
            }
        );
    }

    #[test]
    fn undersized_sources_clamp_to_the_svelte_minimum() {
        let mut planner = panel();
        planner.sources[0].transform.scale_x = 0.0005;
        planner.sources[0].transform.scale_y = 0.0005;
        let event = planner.apply();
        let PlannerEvent::Apply { items } = event else {
            panic!("apply emits");
        };
        assert_eq!(items[0].width, 16);
        assert_eq!(items[0].height, 16);
    }

    #[test]
    fn selection_is_typed_and_rejects_unknown_sources() {
        let mut planner = panel();
        assert_eq!(
            planner.select("camera"),
            Some(PlannerEvent::Select("camera".into()))
        );
        assert_eq!(planner.select("missing"), None);
    }
}
