#![no_std]
#![no_main]

// use userlib::*;

#[export_name = "main"]
fn main() -> ! {
    // enable_led_pins();

    // // Handle messages.
    // let mut incoming = [0u8; idl::INCOMING_SIZE];
    // let mut serverimpl = ServerImpl;
    loop {
        // idol_runtime::dispatch(&mut incoming, &mut serverimpl);
    }
}
