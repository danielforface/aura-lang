//! Deterministic, renderer-independent Lumina measure -> arrange pass.
//!
//! This is a bounded geometry pipeline for the existing MVP nodes; it is NOT
//! a responsive flexbox engine, clipping engine, DPI converter or accessibility tree.
//! Every frame gets a fresh tree. No Raylib, OS handles or compiler APIs are used.

use aura_nexus::UiNode;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Bounds {
    pub(crate) fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        let size = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        Self { x: if x.is_finite() { x } else { 0.0 },
               y: if y.is_finite() { y } else { 0.0 },
               width: size(width), height: size(height) }
    }

    pub(crate) fn contains(self, x: f32, y: f32) -> bool {
        self.width > 0.0 && self.height > 0.0
            && x >= self.x && x <= self.x + self.width
            && y >= self.y && y <= self.y + self.height
    }
}

/// Child slots preserve UiNode child indices, including skipped out-of-range
/// Grid children or ignored extra Box children. The renderer uses these slots
/// without computing geometry or measuring during paint.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayoutNode {
    pub bounds: Bounds,
    pub children: Vec<Option<LayoutNode>>,
}

impl LayoutNode {
    #[cfg(test)]
    fn child(&self, index: usize) -> &LayoutNode {
        self.children[index].as_ref().expect("child should have layout")
    }
}

fn prop<'a>(node: &'a UiNode, key: &str) -> Option<&'a str> {
    node.props.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}
fn int(node: &UiNode, key: &str) -> Option<i32> {
    prop(node, key).and_then(|v| v.parse::<i32>().ok())
}
fn positive(node: &UiNode, key: &str, default: f32) -> f32 {
    int(node, key).map(|v| v.max(0) as f32).unwrap_or(default)
}
fn spacing(node: &UiNode, key: &str) -> f32 {
    positive(node, key, 0.0)
}

fn padding(node: &UiNode) -> (f32, f32, f32, f32) {
    let p = spacing(node, "padding");
    let px = int(node, "padding_x").map(|n| n.max(0) as f32).unwrap_or(p);
    let py = int(node, "padding_y").map(|n| n.max(0) as f32).unwrap_or(p);
    (
        int(node, "padding_top").map(|n| n.max(0) as f32).unwrap_or(py),
        int(node, "padding_right").map(|n| n.max(0) as f32).unwrap_or(px),
        int(node, "padding_bottom").map(|n| n.max(0) as f32).unwrap_or(py),
        int(node, "padding_left").map(|n| n.max(0) as f32).unwrap_or(px),
    )
}

fn grid_dims(node: &UiNode) -> (usize, usize) {
    let cols = int(node, "cols").or_else(|| int(node, "columns")).unwrap_or(1).max(1) as usize;
    let mut rows = int(node, "rows").or_else(|| int(node, "row_count")).unwrap_or(0).max(0) as usize;
    if rows == 0 {
        for child in &node.children {
            let r = int(child, "row").unwrap_or(0).max(0) as usize;
            let span = int(child, "row_span").unwrap_or(1).max(1) as usize;
            rows = rows.max(r.saturating_add(span));
        }
        rows = rows.max(1);
    }
    (cols, rows)
}

/// One bottom-up measurement record per source node; paint never measures.
#[derive(Clone, Debug)]
struct MeasuredNode {
    size: (f32, f32),
    children: Vec<MeasuredNode>,
}

fn measure_tree(node: &UiNode) -> MeasuredNode {
    let children: Vec<_> = node.children.iter().map(measure_tree).collect();
    let size = intrinsic(node, &children);
    MeasuredNode { size, children }
}

#[cfg(test)]
fn measure(node: &UiNode) -> (f32, f32) {
    measure_tree(node).size
}

/// Intrinsic estimate only. Exact text/font shaping is not claimed.
fn intrinsic(node: &UiNode, children: &[MeasuredNode]) -> (f32, f32) {
    match node.kind.as_str() {
        "Box" => {
            let (t, r, b, l) = padding(node);
            let (cw, ch) = children.first().map(|m| m.size).unwrap_or((0.0, 0.0));
            (positive(node, "width", cw + l + r), positive(node, "height", ch + t + b))
        }
        "Grid" => {
            let (cols, rows) = grid_dims(node);
            let (t, r, b, l) = padding(node);
            let gap = spacing(node, "gap");
            let gx = int(node, "gap_x").map(|v| v.max(0) as f32).unwrap_or(gap);
            let gy = int(node, "gap_y").map(|v| v.max(0) as f32).unwrap_or(gap);
            let (cw, ch) = children.iter().map(|m| m.size).fold((0.0_f32, 0.0_f32),
                |(w, h), (x, y)| (w.max(x), h.max(y)));
            (positive(node, "width", cols as f32 * cw + (cols - 1) as f32 * gx + l + r),
             positive(node, "height", rows as f32 * ch + (rows - 1) as f32 * gy + t + b))
        }
        "VStack" => {
            let p = spacing(node, "padding");
            let gap = int(node, "spacing").unwrap_or(0).max(0) as f32;
            let sizes: Vec<_> = children.iter().map(|m| m.size).collect();
            let w = sizes.iter().fold(0.0_f32, |a, b| a.max(b.0));
            let h: f32 = sizes.iter().map(|s| s.1).sum();
            (positive(node, "width", w + p * 2.0),
             positive(node, "height", h + p * 2.0 + gap * sizes.len().saturating_sub(1) as f32))
        }
        "HStack" => {
            let p = spacing(node, "padding");
            let gap = int(node, "spacing").unwrap_or(0).max(0) as f32;
            let sizes: Vec<_> = children.iter().map(|m| m.size).collect();
            let w: f32 = sizes.iter().map(|s| s.0).sum();
            let h = sizes.iter().fold(0.0_f32, |a, b| a.max(b.1));
            (positive(node, "width", w + p * 2.0 + gap * sizes.len().saturating_sub(1) as f32),
             positive(node, "height", h + p * 2.0))
        }
        "Button" => (positive(node, "width", 200.0), positive(node, "height", 50.0)),
        "Rect" => (positive(node, "width", 100.0), positive(node, "height", 100.0)),
        "TextInput" => (positive(node, "width", 360.0), positive(node, "height", 46.0)),
        "Image" => (positive(node, "width", 256.0), positive(node, "height", 256.0)),
        "Text" => {
            let size = positive(node, "size", 20.0);
            let label = prop(node, "text").or_else(|| prop(node, "content")).unwrap_or("");
            (size * 0.6 * label.chars().count() as f32, size)
        }
        _ => (0.0, 0.0),
    }
}

fn with_abs(node: &UiNode, mut b: Bounds) -> Bounds {
    if let Some(x) = int(node, "x") { b.x = x as f32; }
    if let Some(y) = int(node, "y") { b.y = y as f32; }
    b
}

fn content_rect(b: Bounds, pad: (f32, f32, f32, f32)) -> Bounds {
    let (t, r, bottom, l) = pad;
    Bounds::new(b.x + l, b.y + t, (b.width - l - r).max(0.0),
                (b.height - t - bottom).max(0.0))
}

pub(crate) fn arrange(node: &UiNode, incoming: Bounds) -> LayoutNode {
    let measurements = measure_tree(node);
    arrange_measured(node, &measurements, incoming)
}
fn arrange_measured(node: &UiNode, measured: &MeasuredNode, incoming: Bounds) -> LayoutNode {
    let bounds = with_abs(node, incoming);
    let mut children: Vec<Option<LayoutNode>> = (0..node.children.len()).map(|_| None).collect();
    match node.kind.as_str() {
        "Box" => {
            let rect = Bounds::new(bounds.x, bounds.y,
                positive(node, "width", bounds.width).max(1.0),
                positive(node, "height", bounds.height).max(1.0));
            if let Some(child) = node.children.first() {
                let content = content_rect(rect, padding(node));
                children[0] = Some(arrange_measured(child, &measured.children[0], content));
            }
            LayoutNode { bounds: rect, children }
        }
        "Grid" => {
            let rect = Bounds::new(bounds.x, bounds.y,
                positive(node, "width", bounds.width).max(1.0),
                positive(node, "height", bounds.height).max(1.0));
            let content = content_rect(rect, padding(node));
            let (cols, rows) = grid_dims(node);
            let g = spacing(node, "gap");
            let gx = int(node, "gap_x").map(|v| v.max(0) as f32).unwrap_or(g);
            let gy = int(node, "gap_y").map(|v| v.max(0) as f32).unwrap_or(g);
            let cw = ((content.width - (cols - 1) as f32 * gx) / cols as f32).max(0.0);
            let ch = ((content.height - (rows - 1) as f32 * gy) / rows as f32).max(0.0);
            for (i, child) in node.children.iter().enumerate() {
                let col = int(child, "col").unwrap_or(0).max(0) as usize;
                let row = int(child, "row").unwrap_or(0).max(0) as usize;
                if col >= cols || row >= rows { continue; }
                let cs = (int(child, "col_span").unwrap_or(1).max(1) as usize).min(cols - col);
                let rs = (int(child, "row_span").unwrap_or(1).max(1) as usize).min(rows - row);
                let x = content.x + col as f32 * (cw + gx);
                let y = content.y + row as f32 * (ch + gy);
                let w = (cw * cs as f32 + gx * (cs - 1) as f32).min((content.x + content.width - x).max(0.0));
                let h = (ch * rs as f32 + gy * (rs - 1) as f32).min((content.y + content.height - y).max(0.0));
                children[i] = Some(arrange_measured(child, &measured.children[i], Bounds::new(x, y, w, h)));
            }
            LayoutNode { bounds: rect, children }
        }
        "VStack" => {
            let p = spacing(node, "padding");
            let gap = int(node, "spacing").unwrap_or(0).max(0) as f32;
            let content = content_rect(bounds, (p,p,p,p));
            let centered = prop(node, "alignment") == Some("center");
            let mut y = content.y;
            for (i, child) in node.children.iter().enumerate() {
                let (w, h) = measured.children[i].size;
                let x = if centered && w > 0.0 { content.x + (content.width - w)/2.0 } else { content.x };
                let width = if w > 0.0 { w } else { content.width };
                children[i] = Some(arrange_measured(child, &measured.children[i], Bounds::new(x, y, width, h)));
                y += h + gap;
            }
            LayoutNode { bounds, children }
        }
        "HStack" => {
            let p = spacing(node, "padding");
            let gap = int(node, "spacing").unwrap_or(0).max(0) as f32;
            let content = content_rect(bounds, (p,p,p,p));
            let mut x = content.x;
            for (i, child) in node.children.iter().enumerate() {
                let (w, h) = measured.children[i].size;
                children[i] = Some(arrange_measured(child, &measured.children[i], Bounds::new(x, content.y, w, h)));
                x += w + gap;
            }
            LayoutNode { bounds, children }
        }
        "App" => {
            for (i, child) in node.children.iter().enumerate() {
                children[i] = Some(arrange_measured(child, &measured.children[i], bounds));
            }
            LayoutNode { bounds, children }
        }
        "Button" | "TextInput" => {
            let (w,h) = measured.size;
            LayoutNode { bounds: Bounds::new(bounds.x, bounds.y, w, h), children }
        }
        "Image" | "Rect" => {
            LayoutNode { bounds: Bounds::new(bounds.x, bounds.y,
                positive(node, "width", bounds.width).max(1.0),
                positive(node, "height", bounds.height).max(1.0)), children }
        }
        "Text" => LayoutNode { bounds, children },
        _ => {
            for (i, child) in node.children.iter().enumerate() {
                children[i] = Some(arrange_measured(child, &measured.children[i], bounds));
            }
            LayoutNode { bounds, children }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(kind: &str, props: &[(&str, &str)], children: Vec<UiNode>) -> UiNode {
        let mut node = UiNode::new(kind);
        for &(k,v) in props { node.set_prop(k,v); }
        node.children = children;
        node
    }
    fn b(x:f32,y:f32,w:f32,h:f32)->Bounds { Bounds::new(x,y,w,h) }

    #[test]
    fn nested_stack_measurement_prevents_zero_height_overlap() {
        let text = n("Text", &[("text","hi"),("size","20")],vec![]);
        let nested = n("HStack", &[("padding","2"),("spacing","3")],
            vec![n("Button",&[("width","40"),("height","30")],vec![]),text]);
        let root = n("VStack",&[("padding","10"),("spacing","5")],
            vec![nested,n("Button",&[("width","60"),("height","20")],vec![])]);
        assert_eq!(measure(&root), (91.0, 79.0));
        let plan = arrange(&root, b(0.0,0.0,300.0,200.0));
        assert_eq!(plan.child(0).bounds, b(10.0,10.0,71.0,34.0));
        assert_eq!(plan.child(1).bounds, b(10.0,49.0,60.0,20.0));
        assert_eq!(plan.child(0).child(1).bounds.x, 55.0);
    }

    #[test]
    fn box_padding_and_extra_child_are_not_double_rendered() {
        let root = n("Box",&[("padding_left","6"),("padding_top","8"),
          ("padding_right","9"),("padding_bottom","10")],
          vec![n("TextInput",&[("width","70")],vec![]), n("Button",&[],vec![])]);
        let plan=arrange(&root,b(20.0,30.0,200.0,110.0));
        assert_eq!(plan.child(0).bounds,b(26.0,38.0,70.0,46.0));
        assert!(plan.children[1].is_none());
    }

    #[test]
    fn center_align_uses_available_inner_width_after_padding() {
        let root=n("VStack",&[("padding","10"),("alignment","center")],
            vec![n("Button",&[("width","60")],vec![])]);
        let plan=arrange(&root,b(20.0,0.0,200.0,120.0));
        assert_eq!(plan.child(0).bounds,b(90.0,10.0,60.0,50.0));
    }

    #[test]
    fn grid_spans_and_invalid_cell_are_stable_slots() {
        let child=||n("Box",&[],vec![]);
        let root=n("Grid",&[("cols","3"),("rows","2"),("gap","10"),("padding","5")],
          vec![
            n("Box",&[("col","0"),("row","0"),("col_span","2")],vec![]),
            n("Box",&[("col","2"),("row","1")],vec![]),
            n("Box",&[("col","9")],vec![]),
            child()
          ]);
        let plan=arrange(&root,b(0.0,0.0,330.0,150.0));
        assert_eq!(plan.child(0).bounds,b(5.0,5.0,210.0,65.0));
        assert_eq!(plan.child(1).bounds,b(225.0,80.0,100.0,65.0));
        assert!(plan.children[2].is_none());
        assert_eq!(plan.child(3).bounds,b(5.0,5.0,100.0,65.0));
    }

    #[test]
    fn resize_reflows_grid_and_keeps_click_geometry_in_step() {
        let input=n("TextInput",&[("width","100"),("height","40"),("col","1")],vec![]);
        let root=n("Grid",&[("cols","2"),("rows","1"),("padding","10")],
            vec![input]);
        let widths=[300.0,600.0,240.0];
        let positions:Vec<_>=widths.into_iter().map(|w|{
            let plan=arrange(&root,b(0.0,0.0,w,120.0));
            let rect=plan.child(0).bounds;
            assert!(rect.contains(rect.x+1.0,rect.y+1.0));
            assert!(!rect.contains(rect.x-1.0,rect.y+1.0));
            rect.x
        }).collect();
        assert_eq!(positions,vec![150.0,300.0,120.0]);
    }

    #[test]
    fn absolute_override_is_preserved_for_child_hit_rectangles() {
        let root=n("App",&[],vec![n("Button",
            &[("x","140"),("y","80"),("width","50"),("height","30")],vec![])]);
        let plan=arrange(&root,b(0.0,0.0,600.0,400.0));
        assert_eq!(plan.child(0).bounds,b(140.0,80.0,50.0,30.0));
        assert!(plan.child(0).bounds.contains(150.0,90.0));
        assert!(!plan.child(0).bounds.contains(130.0,90.0));
    }

    #[test]
    fn zero_negative_and_non_finite_geometry_never_create_negative_rectangles() {
        assert_eq!(b(0.0,0.0,-100.0, f32::NAN).width,0.0);
        let root=n("Grid",&[("cols","3"),("rows","1"),("gap","500")],
            vec![n("Rect",&[],vec![])]);
        let plan=arrange(&root,b(0.0,0.0,200.0,0.0));
        assert!(plan.child(0).bounds.width >= 0.0);
        assert!(plan.child(0).bounds.height >= 0.0);
        assert!(plan.child(0).bounds.x.is_finite());
    }

    #[test]
    fn grid_row_inference_and_clamped_spans() {
        let root=n("Grid",&[("cols","2"),("gap","2")],
            vec![n("Box",&[("col","1"),("row","2"),("row_span","3"),
                ("col_span","200")],vec![])]);
        let plan=arrange(&root,b(0.0,0.0,202.0,100.0));
        assert_eq!(plan.children.len(),1);
        assert!(plan.child(0).bounds.height > 0.0);
        assert_eq!(plan.child(0).bounds.width,100.0);
    }

    #[test]
    fn each_node_has_exactly_one_bottom_up_measurement_record() {
        let mut node = n("Button", &[("width","30")], vec![]);
        for _ in 0..64 {
            node = n("VStack", &[("padding","1")], vec![node]);
        }
        fn count(m: &MeasuredNode) -> usize {
            1 + m.children.iter().map(count).sum::<usize>()
        }
        let measurements = measure_tree(&node);
        assert_eq!(count(&measurements),65);
        assert_eq!(measurements.size.0,158.0);
        let planned = arrange(&node, b(0.0,0.0,400.0,800.0));
        assert_eq!(planned.children.len(),1);
    }

    #[test]
    fn same_tree_same_viewport_produces_identical_plan_without_global_state() {
        let root=n("VStack",&[],vec![n("Box",&[],vec![n("Button",&[],vec![])])]);
        assert_eq!(arrange(&root,b(0.0,0.0,800.0,600.0)),
                   arrange(&root,b(0.0,0.0,800.0,600.0)));
    }
}
