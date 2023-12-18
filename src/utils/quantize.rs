use candle_core::quantized::{gguf_file, k_quants, QTensor};
use candle_core::{Device, Result, Tensor};
use clap::ValueEnum;
use rayon::prelude::*;

fn run_quantize_safetensors(
    in_files: &[std::path::PathBuf],
    out_file: std::path::PathBuf,
    q: Quantization,
) -> Result<()> {
    let mut out_file = std::fs::File::create(out_file)?;
    let mut tensors = std::collections::HashMap::new();
    for in_file in in_files.iter() {
        let in_tensors = candle_core::safetensors::load(in_file, &Device::Cpu)?;
        tensors.extend(in_tensors)
    }
    println!("tensors: {}", tensors.len());

    let quantize_fn = match q {
        Quantization::Q4_0 => QTensor::quantize::<k_quants::BlockQ4_0>,
        Quantization::Q4_1 => QTensor::quantize::<k_quants::BlockQ4_1>,
        Quantization::Q5_0 => QTensor::quantize::<k_quants::BlockQ5_0>,
        Quantization::Q5_1 => QTensor::quantize::<k_quants::BlockQ5_1>,
        Quantization::Q8_0 => QTensor::quantize::<k_quants::BlockQ8_0>,
        Quantization::Q8_1 => QTensor::quantize::<k_quants::BlockQ8_1>,
        Quantization::Q2k => QTensor::quantize::<k_quants::BlockQ2K>,
        Quantization::Q3k => QTensor::quantize::<k_quants::BlockQ3K>,
        Quantization::Q4k => QTensor::quantize::<k_quants::BlockQ4K>,
        Quantization::Q5k => QTensor::quantize::<k_quants::BlockQ5K>,
        Quantization::Q6k => QTensor::quantize::<k_quants::BlockQ6K>,
        Quantization::Q8k => QTensor::quantize::<k_quants::BlockQ8K>,
        Quantization::F16 => QTensor::quantize::<half::f16>,
        Quantization::F32 => QTensor::quantize::<f32>,
    };
    let block_size = match q {
        Quantization::Q4_0 => k_quants::QK4_0,
        Quantization::Q4_1 => k_quants::QK4_1,
        Quantization::Q5_0 => k_quants::QK5_0,
        Quantization::Q5_1 => k_quants::QK5_1,
        Quantization::Q8_0 => k_quants::QK8_0,
        Quantization::Q8_1 => k_quants::QK8_1,
        Quantization::Q2k
        | Quantization::Q3k
        | Quantization::Q4k
        | Quantization::Q5k
        | Quantization::Q6k
        | Quantization::Q8k => k_quants::QK_K,
        Quantization::F16 | Quantization::F32 => 1,
    };

    let qtensors = tensors
        .into_par_iter()
        .map(|(name, tensor)| {
            let should_quantize = tensor.rank() == 2 && tensor.dim(1)? % block_size == 0;
            println!("  quantizing {name} {tensor:?} {should_quantize}");
            let tensor = if should_quantize {
                quantize_fn(&tensor)?
            } else {
                QTensor::quantize::<f32>(&tensor)?
            };
            Ok((name, tensor))
        })
        .collect::<Result<Vec<_>>>()?;
    let qtensors = qtensors
        .iter()
        .map(|(k, v)| (k.as_str(), v))
        .collect::<Vec<_>>();
    gguf_file::write(&mut out_file, &[], &qtensors)?;
    Ok(())
}

pub fn run_quantize(
    in_files: &[std::path::PathBuf],
    out_file: std::path::PathBuf,
    q: Quantization,
    qmode: QuantizationMode,
) -> Result<()> {
    if in_files.is_empty() {
        candle_core::bail!("no specified input files")
    }
    if let Some(extension) = out_file.extension() {
        if extension == "safetensors" {
            candle_core::bail!("the generated file cannot use the safetensors extension")
        }
    }
    if let Some(extension) = in_files[0].extension() {
        if extension == "safetensors" {
            return run_quantize_safetensors(in_files, out_file, q);
        }
    }

    if in_files.len() != 1 {
        candle_core::bail!("only a single in-file can be used when quantizing gguf files")
    }

    // Open the out file early so as to fail directly on missing directories etc.
    let mut out_file = std::fs::File::create(out_file)?;
    let mut in_ = std::fs::File::open(&in_files[0])?;
    let content = gguf_file::Content::read(&mut in_)?;
    println!("tensors: {}", content.tensor_infos.len());

    let quantize_fn = match q {
        Quantization::Q4_0 => QTensor::quantize::<k_quants::BlockQ4_0>,
        Quantization::Q4_1 => QTensor::quantize::<k_quants::BlockQ4_1>,
        Quantization::Q5_0 => QTensor::quantize::<k_quants::BlockQ5_0>,
        Quantization::Q5_1 => QTensor::quantize::<k_quants::BlockQ5_1>,
        Quantization::Q8_0 => QTensor::quantize::<k_quants::BlockQ8_0>,
        Quantization::Q8_1 => QTensor::quantize::<k_quants::BlockQ8_1>,
        Quantization::Q2k => QTensor::quantize::<k_quants::BlockQ2K>,
        Quantization::Q3k => QTensor::quantize::<k_quants::BlockQ3K>,
        Quantization::Q4k => QTensor::quantize::<k_quants::BlockQ4K>,
        Quantization::Q5k => QTensor::quantize::<k_quants::BlockQ5K>,
        Quantization::Q6k => QTensor::quantize::<k_quants::BlockQ6K>,
        Quantization::Q8k => QTensor::quantize::<k_quants::BlockQ8K>,
        Quantization::F16 => QTensor::quantize::<half::f16>,
        Quantization::F32 => QTensor::quantize::<f32>,
    };

    let qtensors = content
        .tensor_infos
        .par_iter()
        .map(|(name, _)| {
            println!("  quantizing {name}");
            let mut in_file = std::fs::File::open(&in_files[0])?;
            let tensor = content.tensor(&mut in_file, name)?;
            let tensor = qmode.quantize(name, tensor, quantize_fn)?;
            Ok((name, tensor))
        })
        .collect::<Result<Vec<_>>>()?;
    let qtensors = qtensors
        .iter()
        .map(|(k, v)| (k.as_str(), v))
        .collect::<Vec<_>>();

    let metadata = content
        .metadata
        .iter()
        .map(|(k, v)| (k.as_str(), v))
        .collect::<Vec<_>>();
    gguf_file::write(&mut out_file, metadata.as_slice(), &qtensors)?;
    Ok(())
}

#[derive(ValueEnum, Debug, Clone)]
pub enum QuantizationMode {
    /// The default quantization includes all 2d tensors, except the output tensor which always
    /// uses Q6_K.
    Llama,
}

impl QuantizationMode {
    fn quantize(
        &self,
        name: &str,
        tensor: QTensor,
        default: fn(&Tensor) -> Result<QTensor>,
    ) -> Result<QTensor> {
        match self {
            Self::Llama => {
                // Same behavior as the llama.cpp quantization.
                let should_quantize = name.ends_with(".weight") && tensor.rank() == 2;
                if should_quantize {
                    let tensor = tensor.dequantize(&Device::Cpu)?;
                    if name == "output.weight" {
                        QTensor::quantize::<k_quants::BlockQ6K>(&tensor)
                    } else {
                        default(&tensor)
                    }
                } else {
                    Ok(tensor)
                }
            }
        }
    }
}

#[derive(ValueEnum, Debug, Clone)]
pub enum Quantization {
    #[value(name = "q4_0")]
    Q4_0,
    #[value(name = "q4_1")]
    Q4_1,
    #[value(name = "q5_0")]
    Q5_0,
    #[value(name = "q5_1")]
    Q5_1,
    #[value(name = "q8_0")]
    Q8_0,
    #[value(name = "q8_1")]
    Q8_1,
    Q2k,
    Q3k,
    Q4k,
    Q5k,
    Q6k,
    Q8k,
    F16,
    F32,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum Format {
    Safetensors,
    Npz,
    Ggml,
    Gguf,
    Pth,
    Pickle,
}

impl Format {
    pub fn infer<P: AsRef<std::path::Path>>(p: P) -> Option<Self> {
        p.as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .and_then(|e| match e {
                // We don't infer any format for .bin as it can be used for ggml/gguf or pytorch.
                "safetensors" | "safetensor" => Some(Self::Safetensors),
                "npz" => Some(Self::Npz),
                "pth" | "pt" => Some(Self::Pth),
                "ggml" => Some(Self::Ggml),
                "gguf" => Some(Self::Gguf),
                _ => None,
            })
    }
}