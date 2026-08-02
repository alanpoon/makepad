//! Runs a TFLite-derived model spec on `makepad-ggml`.
//!
//! A model is two files, produced by an accompanying converter script:
//!   * `<model>.json`         — the layer list (topology as data, not code)
//!   * `<model>.safetensors`  — the weights, in ggml's kernel layout
//!
//! Keeping the topology in data means one Rust implementation runs any model
//! whose ops fall inside the vocabulary in [`graph`], with no per-model code.

pub mod graph;
pub mod preprocess;
pub mod spec;
pub mod weights;

pub use graph::{build, BuiltGraph};
pub use preprocess::{letterbox_rgb8, Letterbox};
pub use spec::{LayerSpec, ModelSpec};
pub use weights::{WeightTensor, Weights};

use std::fmt;

#[derive(Debug)]
pub enum NnError {
    Io(String),
    Spec(String),
    Weights(String),
    MissingTensor(String),
    Graph(String),
    Backend(String),
    Input(String),
}

impl NnError {
    /// Annotate a graph error with the layer that produced it, which is the
    /// only way to find a bad entry in a several-hundred-layer spec.
    pub fn with_layer(self, index: usize, op: &str, output: &str) -> Self {
        match self {
            Self::Graph(msg) => Self::Graph(format!("layer {index} ({op} -> {output}): {msg}")),
            Self::MissingTensor(name) => Self::Graph(format!(
                "layer {index} ({op} -> {output}): weights have no tensor named {name}"
            )),
            other => other,
        }
    }
}

impl fmt::Display for NnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(m) => write!(f, "io error: {m}"),
            Self::Spec(m) => write!(f, "model spec error: {m}"),
            Self::Weights(m) => write!(f, "weights error: {m}"),
            Self::MissingTensor(m) => write!(f, "weights have no tensor named {m}"),
            Self::Graph(m) => write!(f, "graph error: {m}"),
            Self::Backend(m) => write!(f, "backend error: {m}"),
            Self::Input(m) => write!(f, "input error: {m}"),
        }
    }
}

impl std::error::Error for NnError {}
