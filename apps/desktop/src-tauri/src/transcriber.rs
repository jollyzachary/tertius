use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use transcribe_rs::{
    OrtAccelerator,
    onnx::{
        Quantization,
        parakeet::{ParakeetModel, ParakeetParams, TimestampGranularity},
    },
    set_ort_accelerator,
};

struct LoadedModel {
    path: std::path::PathBuf,
    model: ParakeetModel,
}

pub struct Transcriber {
    model: Mutex<Option<LoadedModel>>,
}

impl Transcriber {
    pub fn new() -> Self {
        let accelerator = if cfg!(feature = "directml") {
            OrtAccelerator::DirectMl
        } else if cfg!(feature = "webgpu") {
            OrtAccelerator::WebGpu
        } else {
            OrtAccelerator::Auto
        };
        set_ort_accelerator(accelerator);
        Self {
            model: Mutex::new(None),
        }
    }

    pub fn transcribe(
        &self,
        model_path: &Path,
        samples: &[f32],
        cancel: Arc<AtomicBool>,
    ) -> Result<String> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(String::new());
        }
        let mut loaded = self.model.lock();
        let reload = loaded.as_ref().is_none_or(|model| model.path != model_path);
        if reload {
            let model = ParakeetModel::load(model_path, &Quantization::Int8)
                .with_context(|| format!("could not load speech model {}", model_path.display()))?;
            *loaded = Some(LoadedModel {
                path: model_path.to_owned(),
                model,
            });
        }
        let model = &mut loaded.as_mut().expect("model loaded above").model;
        let result = model.transcribe_with(
            samples,
            &ParakeetParams {
                timestamp_granularity: Some(TimestampGranularity::Segment),
                ..Default::default()
            },
        )?;
        if cancel.load(Ordering::Relaxed) {
            return Ok(String::new());
        }
        Ok(result.text.trim().to_owned())
    }
}
