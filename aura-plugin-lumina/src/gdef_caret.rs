//! Strict bounded OpenType GDEF LigCaretList reader, no font synthesis.
//! Only static horizontal design-unit positions are qualified: CaretValue 1
//! and CaretValue 3 with null Device/VariationIndex. Format 2 contour
//! coordinates, hinted Device, variable-axis and malformed offsets fail closed.
//! Actual OpenType GDEF spec: offsets are table-relative, coverage-indexed.
const MAX_TABLES:usize=256;
const MAX_COVERAGE:usize=8192;
const MAX_CARETS:usize=64;
pub(crate) const MAX_COMPONENTS:usize=MAX_CARETS+1;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum CaretError {
    MalformedFont,
    MissingCaret,
    UnsupportedCaretFormat,
}
fn u16be(data:&[u8],at:usize)->Result<u16,CaretError>{
    let bytes=data.get(at..at.checked_add(2).ok_or(CaretError::MalformedFont)?)
        .ok_or(CaretError::MalformedFont)?;
    Ok(u16::from_be_bytes([bytes[0],bytes[1]]))
}
fn i16be(data:&[u8],at:usize)->Result<i16,CaretError>{
    Ok(u16be(data,at)? as i16)
}
fn u32be(data:&[u8],at:usize)->Result<u32,CaretError>{
    let bytes=data.get(at..at.checked_add(4).ok_or(CaretError::MalformedFont)?)
        .ok_or(CaretError::MalformedFont)?;
    Ok(u32::from_be_bytes([bytes[0],bytes[1],bytes[2],bytes[3]]))
}
fn at_offset(base:usize,offset:usize,len:usize)->Result<usize,CaretError>{
    let pos=base.checked_add(offset).ok_or(CaretError::MalformedFont)?;
    if pos<len {Ok(pos)}else{Err(CaretError::MalformedFont)}
}
/// Bound the table slice to the declared sfnt directory length, not the rest
/// of the file; a malicious offset must not jump into another table.
fn gdef_table(font:&[u8])->Result<&[u8],CaretError>{
    if font.len()<12{return Err(CaretError::MalformedFont);}
    // No TTC collection/WOFF/OTC interpretation; rustybuzz's fallback
    // shapes may still paint, but no internal GDEF caret is invented.
    let magic=u32be(font,0)?;
    if magic!=0x00010000 && magic!=0x4F54544F {
        return Err(CaretError::UnsupportedCaretFormat);
    }
    let count=u16be(font,4)? as usize;
    if count>MAX_TABLES{return Err(CaretError::MalformedFont);}
    let dir_end=12usize.checked_add(count.checked_mul(16)
        .ok_or(CaretError::MalformedFont)?).ok_or(CaretError::MalformedFont)?;
    if dir_end>font.len(){return Err(CaretError::MalformedFont);}
    for i in 0..count{
        let rec=12+i*16;
        if font.get(rec..rec+4)==Some(b"GDEF".as_slice()) {
            let start=u32be(font,rec+8)? as usize;
            let length=u32be(font,rec+12)? as usize;
            let end=start.checked_add(length).ok_or(CaretError::MalformedFont)?;
            if start<dir_end || end>font.len()||length<12{
                return Err(CaretError::MalformedFont);
            }
            return Ok(&font[start..end]);
        }
    }
    Err(CaretError::MissingCaret)
}
/// Map glyph ID to a zero-based Coverage index, supporting official
/// CoverageFormat1 and CoverageFormat2 with validated records and bounds.
fn coverage_index(data:&[u8],table:usize,gid:u16)->Result<Option<usize>,CaretError>{
    let format=u16be(data,table)?;
    let count=u16be(data,table+2)? as usize;
    if count>MAX_COVERAGE{return Err(CaretError::MalformedFont);}
    match format{
        1=>{
            let end=table.checked_add(4+2*count).ok_or(CaretError::MalformedFont)?;
            if end>data.len(){return Err(CaretError::MalformedFont);}
            let mut prev=None;
            let mut found=None;
            for i in 0..count {
                let id=u16be(data,table+4+2*i)?;
                if prev.is_some_and(|v|id<=v){return Err(CaretError::MalformedFont);}
                prev=Some(id);
                if id==gid{found=Some(i);}
            }
            // Validate *all* records before trusting a hit, including any
            // malformed tail after the first matching glyph.
            Ok(found)
        }
        2=>{
            let end=table.checked_add(4+6*count).ok_or(CaretError::MalformedFont)?;
            if end>data.len(){return Err(CaretError::MalformedFont);}
            let mut prev_end=None;
            let mut found=None;
            for i in 0..count{
                let a=u16be(data,table+4+6*i)?;
                let b=u16be(data,table+6+6*i)?;
                let idx=u16be(data,table+8+6*i)? as usize;
                if a>b || prev_end.is_some_and(|last|a<=last) {
                    return Err(CaretError::MalformedFont);
                }
                prev_end=Some(b);
                if gid>=a&&gid<=b {
                    found=Some(idx+(gid-a) as usize);
                }
            }
            Ok(found)
        }
        _=>Err(CaretError::UnsupportedCaretFormat)
    }
}
/// Read exactly N-1 increasing caret positions in the requested ligature
/// glyph's own design units. Never divide the advance to guess intervals.
pub(crate) fn ligature_caret_coords(
    font:&[u8],gid:u16,expected_count:usize
)->Result<Vec<i16>,CaretError>{
    if expected_count==0 || expected_count>MAX_CARETS{
        return Err(CaretError::UnsupportedCaretFormat);
    }
    let gdef=gdef_table(font)?;
    if u16be(gdef,0)?!=1 || u16be(gdef,2)?>3 {
        return Err(CaretError::UnsupportedCaretFormat);
    }
    let lig_off=u16be(gdef,8)? as usize;
    if lig_off==0{return Err(CaretError::MissingCaret);}
    let lig=at_offset(0,lig_off,gdef.len())?;
    let coverage_off=u16be(gdef,lig)? as usize;
    let count=u16be(gdef,lig+2)? as usize;
    if count>MAX_COVERAGE || coverage_off==0{return Err(CaretError::MalformedFont);}
    let cov=at_offset(lig,coverage_off,gdef.len())?;
    let Some(index)=coverage_index(gdef,cov,gid)? else{return Err(CaretError::MissingCaret);};
    if index>=count{return Err(CaretError::MalformedFont);}
    let ligand_off=u16be(gdef,lig+4+index*2)? as usize;
    if ligand_off==0{return Err(CaretError::MalformedFont);}
    let glyph=at_offset(lig,ligand_off,gdef.len())?;
    let n=u16be(gdef,glyph)? as usize;
    if n!=expected_count{return Err(CaretError::MissingCaret);}
    let mut result=Vec::with_capacity(n);
    let mut prev=0i16;
    for i in 0..n{
        let offset=u16be(gdef,glyph+2+i*2)? as usize;
        if offset==0{return Err(CaretError::MalformedFont);}
        let caret=at_offset(glyph,offset,gdef.len())?;
        let kind=u16be(gdef,caret)?;
        let coordinate=match kind{
            1=>i16be(gdef,caret+2)?,
            3=>{
                let value=i16be(gdef,caret+2)?;
                // Non-null Device/VariationIndex requires unavailable ppem
                // hinting or per-instance variations: never silently ignore.
                if u16be(gdef,caret+4)?!=0{
                    return Err(CaretError::UnsupportedCaretFormat);
                }
                value
            }
            _=>return Err(CaretError::UnsupportedCaretFormat),
        };
        if coordinate<=prev{return Err(CaretError::MalformedFont);}
        prev=coordinate;
        result.push(coordinate);
    }
    Ok(result)
}
#[cfg(test)]
mod tests{
 use super::*;
 fn set16(v:&mut[u8],p:usize,x:u16){v[p..p+2].copy_from_slice(&x.to_be_bytes());}
 fn set32(v:&mut[u8],p:usize,x:u32){v[p..p+4].copy_from_slice(&x.to_be_bytes());}
 /// Small sfnt fixture, generated in-memory; no silent dependency on
 /// external font files for positive/negative malformed table behavior.
 fn fixture(coverage:u16,caret_format:u16,device:u16)->Vec<u8>{
    let mut g=vec![0u8;64];
    set16(&mut g,0,1);set16(&mut g,8,12);
    // LigCaretList at 12: coverage at 20, one LigGlyph at 32.
    set16(&mut g,12,8);set16(&mut g,14,1);set16(&mut g,16,20);
    set16(&mut g,20,coverage);
    set16(&mut g,22,1);
    if coverage==1{set16(&mut g,24,100);}
    else {set16(&mut g,24,100);set16(&mut g,26,100);set16(&mut g,28,0);}
    set16(&mut g,32,2);
    set16(&mut g,34,6);set16(&mut g,36,12);
    set16(&mut g,38,caret_format);set16(&mut g,40,240);
    set16(&mut g,44,caret_format);set16(&mut g,46,510);
    if caret_format==3{set16(&mut g,42,device);set16(&mut g,48,device);}
    let mut f=vec![0u8;28+g.len()];
    set32(&mut f,0,0x00010000);set16(&mut f,4,1);
    f[12..16].copy_from_slice(b"GDEF");
    set32(&mut f,20,28);set32(&mut f,24,g.len() as u32);
    f[28..].copy_from_slice(&g);
    f
 }
 #[test]fn format1_gdef_coverage_1_and_2_exact_coordinates(){
    for c in [1,2]{
        let f=fixture(c,1,0);
        assert_eq!(ligature_caret_coords(&f,100,2),Ok(vec![240,510]));
        assert_eq!(ligature_caret_coords(&f,101,2),Err(CaretError::MissingCaret));
        assert_eq!(ligature_caret_coords(&f,100,1),Err(CaretError::MissingCaret));
    }
 }
 #[test]fn static_format3_allowed_but_device_and_contour_are_not(){
    assert_eq!(ligature_caret_coords(&fixture(1,3,0),100,2),Ok(vec![240,510]));
    assert_eq!(ligature_caret_coords(&fixture(1,3,8),100,2),Err(CaretError::UnsupportedCaretFormat));
    assert_eq!(ligature_caret_coords(&fixture(1,2,0),100,2),Err(CaretError::UnsupportedCaretFormat));
 }
 #[test]fn malicious_table_offsets_coverage_and_order_fail_closed(){
    let f=fixture(1,1,0);
    for i in [0,4,12,20,28,36,40,44,50,60,70,80,90]{
        let shortened=&f[..i.min(f.len())];
        assert!(ligature_caret_coords(shortened,100,2).is_err());
    }
    let mut invalid=f.clone();set32(&mut invalid,20,0xFFFF_FFFF);
    assert!(ligature_caret_coords(&invalid,100,2).is_err());
    let mut reverse=f.clone();set16(&mut reverse,28+46,220);
    assert_eq!(ligature_caret_coords(&reverse,100,2),Err(CaretError::MalformedFont));
    let mut missing=f.clone();set16(&mut missing,28+8,0);
    assert_eq!(ligature_caret_coords(&missing,100,2),Err(CaretError::MissingCaret));
 }
 #[test]fn coverage_format1_rejects_malformed_trailing_glyph_even_after_matching_first(){
    let mut f=fixture(1,1,0);
    // Two coverage entries are required to be sorted. A previously
    // vulnerable early return on first match skipped validation of the
    // malicious second glyph ID. This is a real parser acceptance bug.
    set16(&mut f,28+22,2);
    set16(&mut f,28+24,100);
    set16(&mut f,28+26,90);
    assert_eq!(ligature_caret_coords(&f,100,2),
        Err(CaretError::MalformedFont),
        "no malformed trailing CoverageFormat1 record may be ignored");
 }
 #[test]fn coverage_format2_rejects_overlapping_trailing_range_after_match(){
    let mut f=fixture(2,1,0);
    // Move the LigGlyph record to offset 48, leaving space to encode a
    // second CoverageFormat2 range without overlapping its caret records.
    f.resize(28+96,0);
    let glyph=f[28+32..28+50].to_vec();
    f[28+48..28+66].copy_from_slice(&glyph);
    f[28+32..28+48].fill(0);
    set32(&mut f,24,96);
    set16(&mut f,28+16,36);
    set16(&mut f,28+22,2);
    set16(&mut f,28+30,99);
    set16(&mut f,28+32,99);
    set16(&mut f,28+34,1);
    assert_eq!(ligature_caret_coords(&f,100,2),
        Err(CaretError::MalformedFont),
        "CoverageFormat2 overlap after an earlier match must fail");
 }
 #[test]fn authentic_linux_libertine_font_gdef_ffi_if_installed(){
    let p="/usr/share/fonts/opentype/linux-libertine/LinLibertine_R.otf";
    if !std::path::Path::new(p).is_file(){return;}
    let f=std::fs::read(p).unwrap();
    assert_eq!(ligature_caret_coords(&f,2649,2),Ok(vec![266,550]));
    assert_eq!(ligature_caret_coords(&f,2647,1),Ok(vec![277]));
 }
}
