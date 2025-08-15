// Copyright 2024 Saorsa Labs
// SPDX-License-Identifier: AGPL-3.0-or-later

//! FEC backend implementations

use crate::{FecBackend, Result};

pub mod pure_rust;

#[cfg(all(target_arch = "x86_64", feature = "isa-l"))]
pub mod isa_l;

/// Create the best available backend for the current platform
pub fn create_backend() -> Result<Box<dyn FecBackend>> {
    #[cfg(all(target_arch = "x86_64", feature = "isa-l"))]
    {
        if is_x86_feature_detected!("avx2") {
            return Ok(Box::new(isa_l::IsaLBackend::new()?));
        }
    }
    
    Ok(Box::new(pure_rust::PureRustBackend::new()))
}