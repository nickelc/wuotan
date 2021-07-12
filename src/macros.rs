macro_rules! usb_debug {
    ($device:expr, $msg:expr) => {
        tracing::debug!(
            concat!("USB(bus={},addr={}): ", $msg),
            $device.bus_number(),
            $device.address(),
        );
    };
    ($device:expr, $fmt:expr, $($arg:tt)+) => {
        tracing::debug!(
            concat!("USB(bus={},addr={}): ", $fmt),
            $device.bus_number(),
            $device.address(),
            $($arg)+
        );
    };
}
