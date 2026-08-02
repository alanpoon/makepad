//! Minimal safetensors reader.
//!
//! The format is a little-endian u64 header length, a JSON header mapping
//! tensor name to `{dtype, shape, data_offsets}`, then the raw tensor bytes.
//! Only the dtypes the converter emits are supported (F32 and F16).

use makepad_micro_serde::*;
use std::collections::HashMap;

use crate::NnError;

/// A tensor read out of a safetensors file, always widened to f32.
#[derive(Clone, Debug)]
pub struct WeightTensor {
    /// Row-major shape, outermost dim first (the safetensors convention).
    pub shape: Vec<usize>,
    pub data: Vec<f32>,
}

impl WeightTensor {
    pub fn elements(&self) -> usize {
        self.data.len()
    }
}

#[derive(Debug, Default)]
pub struct Weights {
    tensors: HashMap<String, WeightTensor>,
}

impl Weights {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, NnError> {
        if bytes.len() < 8 {
            return Err(NnError::Weights("file is shorter than the header length field".into()));
        }
        let header_len = u64::from_le_bytes(bytes[0..8].try_into().unwrap()) as usize;
        let header_end = 8usize
            .checked_add(header_len)
            .ok_or_else(|| NnError::Weights("header length overflows".into()))?;
        if header_end > bytes.len() {
            return Err(NnError::Weights(format!(
                "header length {} runs past the end of the {} byte file",
                header_len,
                bytes.len()
            )));
        }
        let header_str = std::str::from_utf8(&bytes[8..header_end])
            .map_err(|e| NnError::Weights(format!("header is not utf8: {e}")))?;
        let header: HashMap<String, JsonValue> = DeJson::deserialize_json(header_str)
            .map_err(|e| NnError::Weights(format!("header is not valid json: {e:?}")))?;

        let data = &bytes[header_end..];
        let mut tensors = HashMap::new();
        for (name, value) in header {
            // safetensors reserves this key for free-form string metadata
            if name == "__metadata__" {
                continue;
            }
            tensors.insert(name.clone(), read_tensor(&name, &value, data)?);
        }
        Ok(Self { tensors })
    }

    pub fn get(&self, name: &str) -> Result<&WeightTensor, NnError> {
        self.tensors
            .get(name)
            .ok_or_else(|| NnError::MissingTensor(name.to_string()))
    }

    pub fn contains(&self, name: &str) -> bool {
        self.tensors.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.tensors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tensors.is_empty()
    }

    /// Tensor names, sorted — used to report what a model file actually holds
    /// when a spec asks for something that is not there.
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tensors.keys().map(|k| k.as_str()).collect();
        names.sort_unstable();
        names
    }
}

fn read_tensor(name: &str, value: &JsonValue, data: &[u8]) -> Result<WeightTensor, NnError> {
    let entry = value
        .object()
        .ok_or_else(|| NnError::Weights(format!("entry for {name} is not an object")))?;

    let dtype = entry
        .get("dtype")
        .and_then(|v| v.string())
        .ok_or_else(|| NnError::Weights(format!("{name} has no dtype")))?
        .to_ascii_uppercase();

    let shape = json_usize_array(entry.get("shape"), name, "shape")?;
    let offsets = json_usize_array(entry.get("data_offsets"), name, "data_offsets")?;
    if offsets.len() != 2 {
        return Err(NnError::Weights(format!(
            "{name} data_offsets must hold exactly 2 values, got {}",
            offsets.len()
        )));
    }
    let (start, end) = (offsets[0], offsets[1]);
    if end < start || end > data.len() {
        return Err(NnError::Weights(format!(
            "{name} data_offsets [{start}, {end}] fall outside the {} byte data block",
            data.len()
        )));
    }
    let raw = &data[start..end];
    let elements: usize = shape.iter().product();

    let values = match dtype.as_str() {
        "F32" => {
            if raw.len() != elements * 4 {
                return Err(NnError::Weights(format!(
                    "{name} holds {} bytes but shape {:?} needs {}",
                    raw.len(),
                    shape,
                    elements * 4
                )));
            }
            raw.chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect()
        }
        "F16" => {
            if raw.len() != elements * 2 {
                return Err(NnError::Weights(format!(
                    "{name} holds {} bytes but shape {:?} needs {}",
                    raw.len(),
                    shape,
                    elements * 2
                )));
            }
            raw.chunks_exact(2)
                .map(|c| makepad_ggml::quant::f16_to_f32(u16::from_le_bytes([c[0], c[1]])))
                .collect()
        }
        other => {
            return Err(NnError::Weights(format!(
                "{name} has unsupported dtype {other}; convert the model to F32 or F16"
            )))
        }
    };

    Ok(WeightTensor {
        shape,
        data: values,
    })
}

fn json_usize_array(
    value: Option<&JsonValue>,
    name: &str,
    field: &str,
) -> Result<Vec<usize>, NnError> {
    let JsonValue::Array(items) = value
        .ok_or_else(|| NnError::Weights(format!("{name} has no {field}")))?
    else {
        return Err(NnError::Weights(format!(
            "{name} {field} is not an array"
        )));
    };
    items
        .iter()
        .map(|item| match item {
            JsonValue::U64(v) => Ok(*v as usize),
            JsonValue::I64(v) if *v >= 0 => Ok(*v as usize),
            JsonValue::F64(v) if *v >= 0.0 && v.fract() == 0.0 => Ok(*v as usize),
            other => Err(NnError::Weights(format!(
                "{name} {field} holds a non-integer value: {other:?}"
            ))),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a safetensors file in memory the way the converter would.
    fn build_file(entries: &[(&str, Vec<usize>, Vec<f32>)]) -> Vec<u8> {
        let mut header = String::from("{");
        let mut blob: Vec<u8> = Vec::new();
        for (i, (name, shape, values)) in entries.iter().enumerate() {
            let start = blob.len();
            for v in values {
                blob.extend_from_slice(&v.to_le_bytes());
            }
            let shape_json = shape
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",");
            if i > 0 {
                header.push(',');
            }
            header.push_str(&format!(
                "\"{name}\":{{\"dtype\":\"F32\",\"shape\":[{shape_json}],\"data_offsets\":[{start},{}]}}",
                blob.len()
            ));
        }
        header.push('}');

        let mut out = Vec::new();
        out.extend_from_slice(&(header.len() as u64).to_le_bytes());
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(&blob);
        out
    }

    #[test]
    fn reads_tensors_and_shapes() {
        let file = build_file(&[
            ("stem.weight", vec![2, 1, 1, 1], vec![1.0, 2.0]),
            ("stem.bias", vec![2], vec![-0.5, 0.25]),
        ]);
        let weights = Weights::from_bytes(&file).unwrap();
        assert_eq!(weights.len(), 2);

        let w = weights.get("stem.weight").unwrap();
        assert_eq!(w.shape, vec![2, 1, 1, 1]);
        assert_eq!(w.data, vec![1.0, 2.0]);

        let b = weights.get("stem.bias").unwrap();
        assert_eq!(b.shape, vec![2]);
        assert_eq!(b.data, vec![-0.5, 0.25]);
    }

    #[test]
    fn reports_missing_tensors_by_name() {
        let file = build_file(&[("a", vec![1], vec![1.0])]);
        let weights = Weights::from_bytes(&file).unwrap();
        let err = weights.get("b").unwrap_err();
        assert!(matches!(err, NnError::MissingTensor(name) if name == "b"));
        assert_eq!(weights.names(), vec!["a"]);
    }

    #[test]
    fn skips_metadata_key() {
        let mut file = Vec::new();
        let header = "{\"__metadata__\":{\"format\":\"pt\"},\"a\":{\"dtype\":\"F32\",\"shape\":[1],\"data_offsets\":[0,4]}}";
        file.extend_from_slice(&(header.len() as u64).to_le_bytes());
        file.extend_from_slice(header.as_bytes());
        file.extend_from_slice(&1.5f32.to_le_bytes());

        let weights = Weights::from_bytes(&file).unwrap();
        assert_eq!(weights.len(), 1);
        assert_eq!(weights.get("a").unwrap().data, vec![1.5]);
    }

    #[test]
    fn rejects_truncated_data() {
        let mut file = Vec::new();
        let header = "{\"a\":{\"dtype\":\"F32\",\"shape\":[4],\"data_offsets\":[0,16]}}";
        file.extend_from_slice(&(header.len() as u64).to_le_bytes());
        file.extend_from_slice(header.as_bytes());
        file.extend_from_slice(&[0u8; 8]);

        assert!(Weights::from_bytes(&file).is_err());
    }
}
