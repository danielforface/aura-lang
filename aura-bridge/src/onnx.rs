#![forbid(unsafe_code)]

use std::{fs, path::Path};

use miette::IntoDiagnostic;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnnxIoShapes {
    pub input: Vec<OnnxDim>,
    pub output: Vec<OnnxDim>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OnnxDim {
    Known(i64),
    Symbolic(String),
}

pub fn read_onnx_io_shapes(path: &Path) -> miette::Result<OnnxIoShapes> {
    let bytes = fs::read(path).into_diagnostic()?;
    let graph = find_len_delimited_field(&bytes, 7)
        .ok_or_else(|| miette::miette!("ONNX model missing graph field"))?;

    let input_vi = find_first_repeated_len_field(graph, 11)
        .ok_or_else(|| miette::miette!("ONNX graph missing input"))?;
    let output_vi = find_first_repeated_len_field(graph, 12)
        .ok_or_else(|| miette::miette!("ONNX graph missing output"))?;

    let input = extract_value_info_shape(input_vi)
        .ok_or_else(|| miette::miette!("ONNX input type missing tensor shape"))?;
    let output = extract_value_info_shape(output_vi)
        .ok_or_else(|| miette::miette!("ONNX output type missing tensor shape"))?;

    Ok(OnnxIoShapes { input, output })
}

fn extract_value_info_shape(value_info: &[u8]) -> Option<Vec<OnnxDim>> {
    // ValueInfoProto.type = field 2 (TypeProto)
    let type_proto = find_len_delimited_field(value_info, 2)?;

    // TypeProto.tensor_type = field 1 (TensorTypeAndShapeProto)
    let tensor_type = find_len_delimited_field(type_proto, 1)?;

    // TensorTypeAndShapeProto.shape = field 2 (TensorShapeProto)
    let shape = find_len_delimited_field(tensor_type, 2)?;

    // TensorShapeProto.dim = repeated field 1 (Dimension)
    let mut dims = Vec::new();
    let mut cursor = shape;
    while let Some((field, wire, val, rest)) = next_field(cursor) {
        cursor = rest;
        if field != 1 || wire != WireType::Len {
            continue;
        }
        let dim_msg = val;
        if let Some(d) = parse_dimension(dim_msg) {
            dims.push(d);
        }
    }

    Some(dims)
}

fn parse_dimension(dim: &[u8]) -> Option<OnnxDim> {
    let mut cursor = dim;
    let mut dim_value: Option<i64> = None;
    let mut dim_param: Option<String> = None;

    while let Some((field, wire, val, rest)) = next_field(cursor) {
        cursor = rest;
        match (field, wire) {
            (1, WireType::Varint) => {
                let v = decode_varint(val)?;
                dim_value = Some(v as i64);
            }
            (2, WireType::Len) => {
                let s = std::str::from_utf8(val).ok()?.to_string();
                dim_param = Some(s);
            }
            _ => {}
        }
    }

    if let Some(v) = dim_value {
        return Some(OnnxDim::Known(v));
    }
    dim_param.map(OnnxDim::Symbolic)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WireType {
    Varint,
    Len,
    Other(u8),
}

fn next_field(input: &[u8]) -> Option<(u32, WireType, &[u8], &[u8])> {
    let (key, rest) = read_varint(input)?;
    let field = (key >> 3) as u32;
    let wt = (key & 0x07) as u8;

    match wt {
        0 => {
            let (_v, rest2) = read_varint(rest)?;
            // Return the varint bytes as slice by re-encoding boundary: easiest is to pass value via 8 bytes,
            // but to keep this parser simple, we pass an empty slice and have caller re-decode from `v`.
            // Instead, we return a synthetic slice that contains the encoded varint by taking from `rest`.
            let consumed = rest.len() - rest2.len();
            Some((field, WireType::Varint, &rest[..consumed], rest2))
        }
        2 => {
            let (len, rest2) = read_varint(rest)?;
            let len = usize::try_from(len).ok()?;
            if rest2.len() < len {
                return None;
            }
            let (val, tail) = rest2.split_at(len);
            Some((field, WireType::Len, val, tail))
        }
        other => {
            let wt = WireType::Other(other);
            let rest2 = skip_unknown(rest, other)?;
            Some((field, wt, &[], rest2))
        }
    }
}

fn skip_unknown(input: &[u8], wire: u8) -> Option<&[u8]> {
    match wire {
        0 => {
            let (_v, rest) = read_varint(input)?;
            Some(rest)
        }
        1 => {
            if input.len() < 8 {
                None
            } else {
                Some(&input[8..])
            }
        }
        2 => {
            let (len, rest) = read_varint(input)?;
            let len = usize::try_from(len).ok()?;
            if rest.len() < len {
                None
            } else {
                Some(&rest[len..])
            }
        }
        5 => {
            if input.len() < 4 {
                None
            } else {
                Some(&input[4..])
            }
        }
        _ => None,
    }
}

fn find_len_delimited_field<'a>(msg: &'a [u8], field_num: u32) -> Option<&'a [u8]> {
    let mut cursor = msg;
    while let Some((field, wire, val, rest)) = next_field(cursor) {
        if field == field_num {
            if let WireType::Len = wire {
                return Some(val);
            }
        }
        cursor = rest;
    }
    None
}

fn find_first_repeated_len_field<'a>(msg: &'a [u8], field_num: u32) -> Option<&'a [u8]> {
    // Same as find_len_delimited_field, but GraphProto uses repeated length-delimited messages.
    find_len_delimited_field(msg, field_num)
}

fn read_varint(input: &[u8]) -> Option<(u64, &[u8])> {
    let mut value: u64 = 0;
    let mut shift = 0;
    let mut i = 0;
    while i < input.len() {
        let b = input[i];
        let low = (b & 0x7F) as u64;
        value |= low << shift;
        i += 1;
        if (b & 0x80) == 0 {
            return Some((value, &input[i..]));
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
    None
}

fn decode_varint(bytes: &[u8]) -> Option<u64> {
    let (v, rest) = read_varint(bytes)?;
    if rest.is_empty() {
        Some(v)
    } else {
        // Caller passed only the varint bytes, so rest should be empty.
        None
    }
}
