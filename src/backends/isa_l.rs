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
        // TODO: Implement ISA-L hardware acceleration
        unimplemented!("ISA-L backend not yet implemented")
    }

    fn decode_blocks(&self, _shares: &mut [Option<Vec<u8>>], _params: FecParams) -> Result<()> {
        // TODO: Implement ISA-L hardware acceleration
        unimplemented!("ISA-L backend not yet implemented")
    }

    fn generate_matrix(&self, _k: usize, _m: usize) -> Vec<Vec<u8>> {
        // TODO: Implement ISA-L matrix generation
        unimplemented!("ISA-L backend not yet implemented")
    }

    fn name(&self) -> &'static str {
        "isa-l"
    }
}

#[cfg(not(feature = "isa-l"))]
pub struct IsaLBackend;

#[cfg(not(feature = "isa-l"))]
impl IsaLBackend {
    pub fn new() -> Self {
        panic!("ISA-L backend not available - enable 'isa-l' feature")
    }
}
