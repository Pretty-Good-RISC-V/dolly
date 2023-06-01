#[no_mangle]
pub unsafe extern "C" fn bdpi_function(value: u8) -> u32 {
    return (value as u32) + 0x100;
}
