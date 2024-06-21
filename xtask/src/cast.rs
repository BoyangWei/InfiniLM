﻿use std::{fs, path::PathBuf, time::Instant};

use digit_layout::types::{BF16, F16, F32};

#[derive(Args, Default)]
pub(crate) struct CastArgs {
    /// Original model directory.
    #[clap(short, long)]
    model: String,
    /// Target model directory.
    #[clap(short, long)]
    target: Option<String>,
    /// Target model type.
    #[clap(long)]
    dt: Option<String>,
}

impl CastArgs {
    pub fn invode(self) {
        let ty = match self.dt.as_deref() {
            Some("f32") | Some("float") | Some("float32") | None => F32,
            Some("f16") | Some("half") | Some("float16") => F16,
            Some("bf16") | Some("bfloat16") => BF16,
            Some(ty) => panic!("Unknown data type: \"{ty}\""),
        };
        let model_dir = PathBuf::from(self.model);

        let time = Instant::now();
        let model = llama::Storage::load_safetensors(&model_dir).unwrap();
        println!("load model ... {:?}", time.elapsed());

        let target = self.target.map(PathBuf::from).unwrap_or_else(|| {
            model_dir.parent().unwrap().join(format!(
                "{}_{ty:?}",
                model_dir.file_name().unwrap().to_str().unwrap()
            ))
        });
        fs::create_dir_all(&target).unwrap();

        let time = Instant::now();
        let model = model.cast(ty);
        println!("cast data type ... {:?}", time.elapsed());

        let time = Instant::now();
        model.save(&target).unwrap();
        println!("save model ... {:?}", time.elapsed());

        let copy_file = |name: &str| {
            let src = model_dir.join(name);
            if src.is_file() {
                let time = Instant::now();
                fs::copy(&src, target.join(name)).unwrap();
                println!("copy {name} ... {:?}", time.elapsed());
            }
        };

        copy_file("tokenizer.model");
        copy_file("vocabs.txt");
    }
}
