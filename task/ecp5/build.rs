// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    build_util::expose_target_board();

    let ecp5_bitstream_name = match env::var("HUBRIS_BOARD")?.as_str() {
        "gimletlet-2" => "blinky_ecp5_evn.compressed.bit",
        //"sidecar-1" => "sidecar_main_board_controller.bit",
        "sidecar-1" => "sidecar_main.bit",
        _ => {
            println!("No FPGA image for target board");
            std::process::exit(1)
        }
    };
    let fpga_bitstream = fs::read(ecp5_bitstream_name)?;
    let compressed_fpga_bitstream = compress(&fpga_bitstream);
    let out_dir = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

    fs::write(out_dir.join("ecp5.bin.rle"), compressed_fpga_bitstream)?;

    // Make sure the app image is rebuilt if the bitstream file for this target
    // changes.
    println!("cargo:rerun-if-changed={}", ecp5_bitstream_name);

    idol::server::build_server_support(
        "../../idl/ecp5.idol",
        "server_stub.rs",
        idol::server::ServerStyle::InOrder,
    )?;

    Ok(())
}

fn compress(input: &[u8]) -> Vec<u8> {
    let mut output = vec![];

    gnarle::compress(input, |chunk| {
        output.extend_from_slice(chunk);
        Ok::<_, std::convert::Infallible>(())
    })
    .ok();

    output
}
