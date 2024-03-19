﻿fn main() {
    if cfg!(feature = "nvidia") && find_cuda_helper::find_cuda_root().is_some() {
        println!("cargo:rustc-cfg=detected_cuda");
    }
}
