// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! API crate for the RoT Sprocket Server

#![no_std]

use userlib::*;

/// Errors that can be produced from the Rot Sprocket server API.
#[derive(Copy, Clone, Debug, FromPrimitive)]
#[repr(u32)]
pub enum SprocketsError {
    FailedToReadSource = 1,
    FailedToHandleRequest = 2,
    FailedToWriteResponse = 3,
}

impl From<u32> for SprocketsError {
    fn from(e: u32) -> Self {
        match e {
            1 => SprocketsError::FailedToReadSource,
            2 => SprocketsError::FailedToHandleRequest,
            3 => SprocketsError::FailedToWriteResponse,
            _ => panic!(),
        }
    }
}

impl From<SprocketsError> for u16 {
    fn from(rc: SprocketsError) -> Self {
        rc as u16
    }
}

impl From<SprocketsError> for u32 {
    fn from(rc: SprocketsError) -> Self {
        rc as u32
    }
}

include!(concat!(env!("OUT_DIR"), "/client_stub.rs"));
