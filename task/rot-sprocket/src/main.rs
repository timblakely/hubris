// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![no_main]

// use idol_runtime::{ClientError, Leased, LenLimit, RequestError, R, W};
use idol_runtime::{ClientError, Leased, RequestError, R, W};
use ringbuf::*;
use task_rot_sprocket_api::SprocketsError;
use userlib::*;

use sprockets::msgs::{RotRequest, RotResponse, SerializedSize};
use sprockets::rot::{RotConfig, RotSprocket};

#[derive(Copy, Clone, PartialEq, Debug)]
enum LogMsg {
    Init,
    BootstrapConfig,
    CreateSprocket,
    HandledRequest,
    GetEndorsementsCalled(usize, usize),
    AddHostMeasurementsCalled(usize, usize),
    GetMeasurementsCalled(usize, usize),
}
ringbuf!(LogMsg, 4, LogMsg::Init);

#[export_name = "main"]
fn main() -> ! {
    let mut buffer = [0; idl::INCOMING_SIZE];
    let mut server = ServerImpl::new();

    loop {
        idol_runtime::dispatch(&mut buffer, &mut server);
    }
}

struct ServerImpl {
    sprocket: RotSprocket,
}

impl ServerImpl {
    fn new() -> ServerImpl {
        ringbuf_entry!(LogMsg::Init);
        let config = RotConfig::bootstrap_for_testing();
        ringbuf_entry!(LogMsg::BootstrapConfig);
        let mut sprocket = RotSprocket::new(config);
        ringbuf_entry!(LogMsg::CreateSprocket);
        ServerImpl { sprocket }
    }

    fn handle_request(
        &mut self,
        request: Leased<R, [u8]>,
        response: Leased<W, [u8]>,
    ) -> Result<usize, RequestError<SprocketsError>> {
        let mut req = [0u8; RotRequest::MAX_SIZE];
        let mut rsp = [0u8; RotResponse::MAX_SIZE];

        // Read the entire message into our address space.
        request
            .read_range(0..request.len(), &mut req)
            .map_err(|_| SprocketsError::FailedToReadSource)?;

        let pos = self
            .sprocket
            .handle(&req, &mut rsp)
            .map_err(|_| SprocketsError::FailedToHandleRequest)?;

        ringbuf_entry!(LogMsg::HandledRequest);

        response
            .write_range(0..pos, &rsp[..pos])
            .map_err(|_| SprocketsError::FailedToWriteResponse)?;

        Ok(pos)
    }
}

impl idl::InOrderRotSprocketImpl for ServerImpl {
    fn get_endorsements(
        &mut self,
        _: &RecvMessage,
        request: Leased<R, [u8]>,
        response: Leased<W, [u8]>,
    ) -> Result<usize, RequestError<SprocketsError>> {
        ringbuf_entry!(LogMsg::GetEndorsementsCalled(
            request.len(),
            response.len()
        ));
        self.handle_request(request, response)
    }

    fn add_host_measurements(
        &mut self,
        _: &RecvMessage,
        request: Leased<R, [u8]>,
        response: Leased<W, [u8]>,
    ) -> Result<usize, RequestError<SprocketsError>> {
        ringbuf_entry!(LogMsg::AddHostMeasurementsCalled(
            request.len(),
            response.len()
        ));
        self.handle_request(request, response)
    }

    fn get_measurements(
        &mut self,
        _: &RecvMessage,
        request: Leased<R, [u8]>,
        response: Leased<W, [u8]>,
    ) -> Result<usize, RequestError<SprocketsError>> {
        ringbuf_entry!(LogMsg::GetMeasurementsCalled(
            request.len(),
            response.len()
        ));
        self.handle_request(request, response)
    }
}

mod idl {
    use super::SprocketsError;

    include!(concat!(env!("OUT_DIR"), "/server_stub.rs"));
}
