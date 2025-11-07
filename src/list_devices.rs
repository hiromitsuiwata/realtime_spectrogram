use cpal::traits::{DeviceTrait, HostTrait};

fn main() {
    let host = cpal::default_host();
    println!("Available input devices:");
    for device in host.input_devices().unwrap() {
        println!(" - {}", device.name().unwrap());
    }

    match host.default_input_device() {
        Some(dev) => println!("Default input: {}", dev.name().unwrap()),
        None => println!("No default input device."),
    }
}