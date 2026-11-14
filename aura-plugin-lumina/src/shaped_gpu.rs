//! Native opt-in GPU path: actual OpenType GIDs -> fontdue indexed rasters
//! -> tightly packed Raylib texture -> GPOS positioned DrawTexturePro.
//! Never reinterpret glyph ID as a Unicode codepoint. CPU/GPU allocations
//! are bounded. GPU Texture2D MUST drop before RaylibHandle (struct order).
use std::collections::{HashMap, HashSet};
use raylib::prelude::*;
use crate::{font_atlas, text_shaping};
use text_shaping::{ParagraphDirection, ShapedText};

pub(crate) const MAX_GLYPHS: usize = 384;
pub(crate) const MAX_ATLAS_SIDE: i32 = 1024;
pub(crate) const MAX_ATLAS_PIXELS: usize = (MAX_ATLAS_SIDE as usize)*(MAX_ATLAS_SIDE as usize);
pub(crate) const MAX_CACHE_ENTRIES: usize = 12;
pub(crate) const MAX_RENDER_TEXT_BYTES: usize = 8192;
pub(crate) const MAX_FONT_CHAIN: usize = text_shaping::MAX_FONT_CHAIN;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cell {
    pub(crate) src: Rectangle,
    pub(crate) xmin: i32,
    pub(crate) ymin: i32,
    pub(crate) width: usize,
    pub(crate) height: usize,
}
pub(crate) struct PreparedGlyphText {
    // GPU resource must be dropped before the window context closes.
    pub(crate) atlas: Texture2D,
    pub(crate) cells: HashMap<(usize,u16),Cell>,
    pub(crate) shaped: ShapedText,
    pub(crate) ascent: f32,
}

pub(crate) fn is_opted(node:&aura_nexus::UiNode)->bool {
    node.props.iter().any(|(k,v)|k=="shaping" && v=="opentype")
}
fn prop<'a>(node:&'a aura_nexus::UiNode,key:&str)->Option<&'a str>{
    node.props.iter().find(|(k,_)|k==key).map(|(_,v)|v.as_str())
}
pub(crate) fn entry_key(node:&aura_nexus::UiNode,text:&str,size:i32)->Option<String> {
    if !is_opted(node) || text.len()>MAX_RENDER_TEXT_BYTES {return None;}
    let path=prop(node,"font_path")?;
    font_atlas::font_key(path,size)?;
    let fallback_font_path=prop(node,"fallback_font_path");
    if let Some(fallback)=fallback_font_path {
        if fallback==path{return None;}
        font_atlas::font_key(fallback,size)?;
    }
    let dir=prop(node,"direction").unwrap_or("auto");
    ParagraphDirection::parse(Some(dir))?;
    Some(format!("{size}|{path}\0{}\0{dir}\0{text}",fallback_font_path.unwrap_or("")))
}
/// One prepared shaped texture per visible (font,size,direction,text) identity.
/// No work for legacy widgets or invalid shapes. Retention drops unused GPU
/// textures; a transient invalid shape is not rendered as misleading RTL.
pub(crate) fn prepare(
    rl:&mut RaylibHandle,thread:&RaylibThread,cache:&mut HashMap<String,PreparedGlyphText>,
    desired:&[(String,String,Option<String>,i32,String,String)]
) {
    let wanted:HashSet<&str>=desired.iter().map(|(key,_,_,_,_,_)|key.as_str()).collect();
    cache.retain(|k,_|wanted.contains(k.as_str()));
    for (key,path,fallback,size,dir,text) in desired {
        if cache.contains_key(key) || cache.len()>=MAX_CACHE_ENTRIES {continue;}
        if font_atlas::font_key(path,*size).is_none() ||
            text.len()>MAX_RENDER_TEXT_BYTES {continue;}
        let mut paths=vec![path.as_str()];
        if let Some(fallback)=fallback.as_deref(){
            if fallback==path || font_atlas::font_key(fallback,*size).is_none(){continue;}
            paths.push(fallback);
        }
        if paths.len()>MAX_FONT_CHAIN {continue;}
        // Bounded exact bytes and metadata are checked for every file, never
        // loading an unrestricted system fallback chain.
        let mut files=Vec::with_capacity(paths.len());
        let mut valid=true;
        for p in paths {
            match std::fs::metadata(p) {
                Ok(m) if m.is_file()&&m.len()>0&&
                    m.len()<=text_shaping::MAX_SHAPE_FONT_BYTES as u64 => {
                    if let Ok(bytes)=std::fs::read(p) {
                        if bytes.len()<=text_shaping::MAX_SHAPE_FONT_BYTES {
                            files.push(bytes);continue;
                        }
                    }
                }
                _=>{}
            }
            valid=false;break;
        }
        if !valid {continue;}
        let Some(direction)=ParagraphDirection::parse(Some(dir)) else{continue;};
        let refs:Vec<&[u8]>=files.iter().map(|v|v.as_slice()).collect();
        let Some(shaped)=text_shaping::shape_text_with_fonts(text,&refs,*size as f32,direction)
            else{continue;};
        let mut fonts=Vec::with_capacity(files.len());
        for bytes in files {
            let Ok(f)=fontdue::Font::from_bytes(bytes,fontdue::FontSettings::default())
                else{valid=false;break;};
            fonts.push(f);
        }
        if !valid {continue;}
        if let Some(atlas)=rasterize_atlas(rl,thread,&fonts,&shaped,*size as f32){
            cache.insert(key.clone(),atlas);
        }
    }
}
fn rasterize_atlas(
    rl:&mut RaylibHandle,thread:&RaylibThread,fonts:&[fontdue::Font],
    shaped:&ShapedText,size:f32
)->Option<PreparedGlyphText> {
    let ids:std::collections::BTreeSet<(usize,u16)>=shaped.visual_runs.iter()
        .flat_map(|r|r.glyphs.iter().map(|g|(g.font_index,g.glyph_id))).collect();
    if ids.len()>MAX_GLYPHS {return None;}
    // Packed, single texture with 1px bleed-safety gutter per glyph;
    // rejected before GPU upload if dimensions exceed 1024x1024.
    let mut cell_map=HashMap::with_capacity(ids.len());
    let mut all=Vec::with_capacity(ids.len());
    let mut x=1i32;
    let mut y=1i32;
    let mut row_h=0i32;
    for id in ids {
        let font=fonts.get(id.0)?;
        let (metrics, bitmap)=font.rasterize_indexed(id.1,size);
        if metrics.width>MAX_ATLAS_SIDE as usize ||
           metrics.height>MAX_ATLAS_SIDE as usize ||
           bitmap.len()!=metrics.width.checked_mul(metrics.height)? {return None;}
        let w=i32::try_from(metrics.width).ok()?;
        let h=i32::try_from(metrics.height).ok()?;
        let rw=w.max(1)+2;
        let rh=h.max(1)+2;
        if x.checked_add(rw)?>MAX_ATLAS_SIDE {
            x=1; y=y.checked_add(row_h)?; row_h=0;
        }
        if y.checked_add(rh)?>MAX_ATLAS_SIDE {return None;}
        let cell=Cell {src:Rectangle::new(x as f32,y as f32,w as f32,h as f32),
            xmin:metrics.xmin,ymin:metrics.ymin,
            width:metrics.width,height:metrics.height};
        cell_map.insert(id,cell);
        all.push((id,bitmap));
        x=x.checked_add(rw)?;
        row_h=row_h.max(rh);
    }
    let height=(y+row_h).clamp(2,MAX_ATLAS_SIDE);
    let mut image=Image::gen_image_color(MAX_ATLAS_SIDE,height,Color::BLANK);
    // Native raylib ImageDrawPixel is CPU-side. No unsafe FFI, no GPU
    // work during drawing. Use source bitmap alpha, tint in DrawTexturePro.
    let mut visited_pixels=0usize;
    for (id,bitmap) in all {
        let cell=cell_map.get(&id)?;
        for (j,row) in bitmap.chunks_exact(cell.width.max(1)).enumerate() {
            if j>=cell.height {break;}
            for (i,&alpha) in row.iter().enumerate() {
                if alpha==0 {continue;}
                let xx=(cell.src.x as i32).checked_add(i32::try_from(i).ok()?)?;
                let yy=(cell.src.y as i32).checked_add(i32::try_from(j).ok()?)?;
                if !(0..MAX_ATLAS_SIDE).contains(&xx) || !(0..height).contains(&yy) {return None;}
                image.draw_pixel(xx,yy,Color::new(255,255,255,alpha));
                visited_pixels+=1;
                if visited_pixels>MAX_ATLAS_PIXELS{return None;}
            }
        }
    }
    let atlas=rl.load_texture_from_image(thread,&image).ok()?;
    // One common baseline for all font faces; bearings are per-font
    // metrics from fontdue. Avoid per-font top alignment jitter.
    let ascent=fonts.first()?.horizontal_line_metrics(size)
        .map(|line|line.ascent).unwrap_or(size*0.8);
    if !ascent.is_finite(){return None;}
    Some(PreparedGlyphText{atlas,cells:cell_map,shaped:shaped.clone(),ascent})
}
pub(crate) fn paint<D:RaylibDraw>(
    d:&mut D,prepared:&PreparedGlyphText,x:f32,y:f32,color:Color
){
    for run in &prepared.shaped.visual_runs {
        for glyph in &run.glyphs {
            let Some(cell)=prepared.cells.get(&(glyph.font_index,glyph.glyph_id)) else{continue;};
            if cell.width==0 || cell.height==0 {continue;}
            // Fontdue ymin is relative to baseline with +y upward. Raylib
            // uses top-left origin. GPOS y_offset is likewise positive up.
            let dst=Rectangle::new(
                x+glyph.pen_x+glyph.offset_x+cell.xmin as f32,
                y+prepared.ascent-glyph.offset_y-(cell.ymin as f32+cell.height as f32),
                cell.width as f32,cell.height as f32);
            if !dst.x.is_finite() || !dst.y.is_finite(){continue;}
            d.draw_texture_pro(&prepared.atlas,cell.src,dst,Vector2::new(0.0,0.0),0.0,color);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]fn bounded_gpu_allocation_contract(){
        assert!(MAX_ATLAS_SIDE<=1024 && MAX_GLYPHS<=384);
        assert!(MAX_CACHE_ENTRIES<=12 && MAX_RENDER_TEXT_BYTES<=8192);
    }
    #[test]fn invalid_opt_in_inputs_do_not_silently_downgrade_direction(){
        let mut n=aura_nexus::UiNode::new("Text");
        n.set_prop("shaping","opentype");n.set_prop("font_path","/tmp/example.ttf");
        n.set_prop("direction","sideways");
        assert!(entry_key(&n,"hello",24).is_none());
        n.set_prop("direction","rtl");
        assert!(entry_key(&n,"שלום",24).is_some());
        assert!(entry_key(&n,&"A".repeat(MAX_RENDER_TEXT_BYTES+1),24).is_none());
    }
}
