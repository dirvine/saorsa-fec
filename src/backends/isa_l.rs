// Copyright 2024 Saorsa Labs
// SPDX-License-Identifier: AGPL-3.0-or-later

//! ISA-L hardware-accelerated backend for x86_64 platforms

#[cfg(feature = "isa-l")]
use crate::{FecBackend, FecParams, Result};

/// ISA-L hardware-accelerated backend
#[cfg(feature = "isa-l")]
pub struct IsaLBackend;

#[cfg(feature = "isa-l")]
impl FecBackend for IsaLBackend {
    fn new() -> Self {
        Self
    }

    fn encode_blocks(
        &self,
        _data: &[&[u8]],
        _parity: &mut [Vec<u8>],
        _params: FecParams,
    ) -> Result<()> {
        anyhow::bail!("ISA-L backend not yet implemented - use pure-rust backend instead")
    }

    fn decode_blocks(&self, _shares: &mut [Option<Vec<u8>>], _params: FecParams) -> Result<()> {
        anyhow::bail!("ISA-L backend not yet implemented - use pure-rust backend instead")
    }

    fn generate_matrix(&self, _k: usize, _m: usize) -> Vec<Vec<u8>> {
        // Return empty matrix as placeholder - this will cause calling code to use default
        Vec::new()
    }

    fn name(&self) -> &'static str {
        "isa-l"
    }
}

#[cfg(not(feature = "isa-l"))]
pub struct IsaLBackend;

#[cfg(not(feature = "isa-l"))]
impl IsaLBackend {
    pub fn new() -> Result<Self, &'static str> {
        Err("ISA-L backend not available - enable 'isa-l' feature")
    }
}
