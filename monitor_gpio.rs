mod array_set;
use crate::array_set::PinSet;

mod rpi_constants;
use crate::rpi_constants::*;

//mod ioctl_constants;
//use crate::ioctl_constants::*;

use core::pin::Pin;

use kernel::{
    c_str,
    device::Device,
    fs::File,
    ioctl::{_IOC_SIZE, _IOWR, _IOR, _IOW},
    miscdevice::{MiscDevice, MiscDeviceOptions, MiscDeviceRegistration},
    new_mutex,
    prelude::*,
    sync::Mutex,
    types::ARef,
    uaccess::{UserSlice, UserSliceReader, UserSliceWriter},
    bindings::{ioremap, iounmap, readl},
};

// TO DO:
//
// - Figure out how to make input and output arrays global: 
//   Will require making the reference to PinSet static and global.
//   At the moment only one application may call driver as arrays get reinitialised.
//   Potentially use this struct and initialise in fn init:
// struct SharedInner {
//     outputa: PinSet,
//     inputa: PinSet,
// }
//
// - Figure out how to not need unsafe to call ioctl in userspace
//   Alternatively, can wait for other ways to access /dev in the future when further
//   Rust-For-Linux updates are released or through own abstractions
//
//
// - Add other configuartion options for example, pull down resister
// - Check BCM2837 peripheral sheet for other functions available.
//
//
// 
// - LOW PRIORITY: Remove all unsafe calls and replace with Rust Abstractions
// - May have to wait for further Rust updates in Rust-For-Linux



// Constants to hold variables which are passed/returned to functions
const RUST_MISC_DEV_GPIO_READ: u32 = _IOWR::<i32>('|' as u32, 0x80);
const RUST_MISC_DEV_GET_OUTPUTS: u32 = _IOR::<[i32; 28]>('|' as u32, 0x81);
const RUST_MISC_DEV_GET_INPUTS: u32 = _IOR::<[i32; 28]>('|' as u32, 0x82);
const RUST_MISC_DEV_SET_OUTPUT: u32 = _IOW::<[i32; 2]>('|' as u32, 0x83);
const RUST_MISC_DEV_SET_MODE: u32 = _IOW::<[i32; 2]>('|' as u32, 0x84);



// Register device in kernel
module! {
    type: RustMiscDeviceModule,
    name: "monitor_gpio",
    authors: ["Kazi Intisar"],
    description: "Device to monitor GPIO",
    license: "GPL",
}



// Register device in /dev menu
#[pin_data]
struct RustMiscDeviceModule {
    #[pin]
    _miscdev: MiscDeviceRegistration<RustMiscDevice>,
}

impl kernel::InPlaceModule for RustMiscDeviceModule {
    fn init(_module: &'static ThisModule) -> impl PinInit<Self, Error> {
        pr_info!("Initialising monitor_gpoio driver\n");

        let options = MiscDeviceOptions {
            name: c_str!("monitor_gpio"),       // /dev/monitor_gpio
        };

        try_pin_init!(Self {
            _miscdev <- MiscDeviceRegistration::register(options),
        })
    }
}

fn cpu_relax_delay() {
    // Crude ~150 cycle delay according to datasheet for Raspberry Pi
    for _ in 0..150 {
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

// Struct for holding values when opening /dev file
// Values in this struct are muteable and in a Mutex lock
// This struct is used in RustMiscDevice
struct Inner {
    value: i32,
    output: PinSet, // Holds output pins
    input: PinSet,  // Holds input pins
    alternate: PinSet,
}

// All variables using in open menu
#[pin_data(PinnedDrop)]
struct RustMiscDevice {
    #[pin]
    inner: Mutex<Inner>,    // Struct variables in Mutex
    dev: ARef<Device>,      // Device file for /dev menu

    // Map GPIO base during intialisation to save resources (saved 0.1-0.2 (25%) ms)
    // Alternative is to intiialise everytime an ioctl function is called
    gpio_base: *mut u8, // pointer returned by ioremap
}

// Declare safety for thread usage as u8 (raw pointer) type is typically not included in RustMiscDevice
// Potentially unsafe - look in the future
unsafe impl Send for RustMiscDevice {}
unsafe impl Sync for RustMiscDevice {}



// Implementation to Implement MiscDevice trait for RustMiscDevice
// Relates to system calls 
#[vtable]
impl MiscDevice for RustMiscDevice {
    type Ptr = Pin<KBox<Self>>;

    // Run when /dev/device is opened first (initialisation function)
    fn open(_file: &File, misc: &MiscDeviceRegistration<Self>) -> Result<Pin<KBox<Self>>> {
        let dev = ARef::from(misc.device());

        dev_info!(dev, "Opening gpio-monitor\n");

        // Memory map physical address for GPIO Pins to be accessible using C wrapper (no Rust abstract at time of writing)
        let gpio_base = unsafe { ioremap(GPIO_BASE, GPIO_BLOCK_SIZE) };     // *** FUTURE: USE RUST ABSTRACTIONS IF AVAILABLE OR IMPLEMENT OWN ***
        if gpio_base.is_null() {    // Checks if successful
            dev_err!(dev, "Failed to ioremap GPIO base\n");
            return Err(ENOMEM);
        }

        // Pin device variables for device into memory (heap)
        let device = KBox::try_pin_init(
            try_pin_init! {
                RustMiscDevice {
                    inner <- new_mutex!(Inner {
                        value: 0,
                        output: PinSet::new(),
                        input: PinSet::new(),
                        alternate: PinSet::new(),
                    }),
                    dev: dev,
                    gpio_base: gpio_base.cast(),
                }
            },
            GFP_KERNEL,
        )?;

        // Function to intiialise pins and sort them to correct arrays according to current state defined by memory (see BCM2835/2837 Documentation for RPi3)
        {
            let dev_ref = device.as_ref();
            let mut inner = dev_ref.inner.lock();
            for pin in 0..28 {
                let fsel_index = pin / 10;          // Finds function register dependent on tenth digit
                let bit_offset = (pin % 10) * 3;    // Finds offset dependent on oneth digit

                // Get Address according to index
                let fsel_addr = unsafe {dev_ref.gpio_base.add(fsel_index as usize * 4) as *const u32 }; // *** FUTURE: USE RUST ABSTRACTIONS IF AVAILABLE OR IMPLEMENT OWN ***
                let val = unsafe { readl(fsel_addr.cast()) };                                           // *** FUTURE: USE RUST ABSTRACTIONS IF AVAILABLE OR IMPLEMENT OWN ***
                let mode = (val >> bit_offset) & 0b111; // Gets 3 bits for specific pin, using offset

                // Turn pull up resister off
                let gppud = unsafe { dev_ref.gpio_base.add(GPPUD_OFFSET) as *mut u32 };
                let gppudclk0 = unsafe { dev_ref.gpio_base.add(GPPUDCLK0_OFFSET) as *mut u32 };
                unsafe {
                    gppud.write_volatile(0b01); // 0b01 = Pull-down
                    cpu_relax_delay();
                    gppudclk0.write_volatile(1 << pin);
                    cpu_relax_delay();
                    gppud.write_volatile(0);
                    gppudclk0.write_volatile(0);
                }

                match mode {
                    0b000 => {  // Input Mode
                        inner.input.add(pin);

                    },
                    0b001 => {  // Output Mode
                        inner.output.add(pin);
                    },
                    _ => {      // Anything else (should be none - for future modes such as UART, I2C)
                        inner.alternate.add(pin);
                    }
                }

            }
        }
        Ok(device)
    }



    // This function is called when a function is called from userspace, using IOCTL
    // This function will match that function from userspace, and link it to the function defined in this code.
    fn ioctl(me: Pin<&RustMiscDevice>, _file: &File, cmd: u32, arg: usize) -> Result<isize> {
        dev_info!(me.dev, "IOCTLing gpio-monitor\n");

        let size = _IOC_SIZE(cmd);
        
        match cmd {
            RUST_MISC_DEV_GPIO_READ => {
                let slice = UserSlice::new(arg, size);
                let (mut reader, mut writer) = slice.reader_writer();
                let pin = reader.read::<i32>()? as u32;
                me.read_gpio(pin, &mut writer)?;
            },
            RUST_MISC_DEV_GET_OUTPUTS => {
                let slice = UserSlice::new(arg, size);
                let mut writer = slice.writer();
                me.get_output_pins(&mut writer)?;
            },
            RUST_MISC_DEV_GET_INPUTS => {
                let slice = UserSlice::new(arg, size);
                let mut writer = slice.writer();
                me.get_input_pins(&mut writer)?;
            },
            RUST_MISC_DEV_SET_OUTPUT => {
                let slice = UserSlice::new(arg, size);
                let mut reader = slice.reader();
                let vals: [i32; 2] = reader.read()?;
                let pin = vals[0] as u32;
                let value = vals[1] != 0;
                me.set_gpio_output(pin, value)?;
            },
            RUST_MISC_DEV_SET_MODE => {
                let slice = UserSlice::new(arg, size);
                let mut reader = slice.reader();
                let vals: [i32; 2] = reader.read()?;
                let pin = vals[0] as u32;
                let is_output = vals[1] != 0;
                me.set_gpio_mode(pin, is_output)?;
            },
            _ => {
                dev_err!(me.dev, "-> IOCTL not recognised: {}\n", cmd);
                return Err(ENOTTY);
            }
        }
        Ok(0)
    }
}



// Implement PinnedDrop trait for RustMiscDevice
// Function called when rmmod is called (mod is removed from kernel)
#[pinned_drop]
impl PinnedDrop for RustMiscDevice {
    fn drop(self: Pin<&mut Self>) {
        dev_info!(self.dev, "Exiting the monitor-gpio driver\n");
        if !self.gpio_base.is_null() {
            unsafe { iounmap(self.gpio_base.cast()) }; // .cast() turns into a void type which is required when passing to
            // *** FUTURE: USE RUST ABSTRACTIONS IF AVAILABLE OR IMPLEMENT OWN *** C
        }
    }
}



// Implementation block for functions in IOCTL function
impl RustMiscDevice {

    // Userspace IO
    // Input: [u32] pin number (i32 from userspace is fine)
    // Output: [isize] Integer (0 or 1)
    // Will read gpio value ONLY if Pin is in 'Input' state
    fn read_gpio(&self, pin: u32, writer: &mut UserSliceWriter) -> Result<isize> {
        let inner = self.inner.lock();

        if !inner.input.contains(pin) {
            dev_err!(self.dev, "Access denied to GPIO pin: {}\n", pin);
            return Err(EACCES);
        }

        // Calculate register address
        let gplev0 = unsafe { self.gpio_base.add(GPLEV0_OFFSET) as *const u32 };
        let level = unsafe { readl(gplev0.cast()) };

        // Log raw register and address
        dev_info!(
            self.dev,
            "GPIO read for pin {}: GPLEV0 addr = 0x{:p}, raw = {:#034b}",
            pin,
            gplev0,
            level
        );

        let is_high = (level & (1 << pin)) != 0;
        let result: i32 = if is_high { 1 } else { 0 };

        dev_info!(
            self.dev,
            "Masking: (1 << {}) = {:#034b}, result bit = {}, level = {}",
            pin,
            1 << pin,
            result,
            if is_high { "HIGH" } else { "LOW" }
        );

        writer.write(&result)?;
        Ok(0)
    }


    // Userspace IO
    // Input: [u32] pin number (i32 from userspace is fine)
    // Output: [isize] Integer (0 or 1)
    // Will set gpio value ONLY if Pin is in 'Output' state
    fn set_gpio_output(&self, pin: u32, value: bool) -> Result {

        let inner = self.inner.lock();

        if !inner.output.contains(pin) {
            dev_err!(self.dev, "Attempt to set GPIO {} not allowed: not configured as output\n", pin);
            return Err(EACCES);
        }

        // One register (32bit) controls allpins
        let reg_offset = if value { GPSET0_OFFSET } else { GPCLR0_OFFSET };
        let reg = unsafe { self.gpio_base.add(reg_offset) as *mut u32 };

        unsafe {
            reg.write_volatile(1 << pin);
        }

        dev_info!(self.dev, "GPIO {} set to {}\n", pin, if value { "HIGH" } else { "LOW" });
        Ok(())
    }

    // Userspace IO
    // Input: [u32] pin number (i32 from userspace is fine)
    // Output: [isize] Integer (0 or 1)
    // Set GPIO Pin State (Input 0 or Output 1)
    fn set_gpio_mode(&self, pin: u32, is_output: bool) -> Result {
        if pin > 27 {
            dev_err!(self.dev, "Invalid GPIO pin number: {}\n", pin);
            return Err(EINVAL);
        }

        // Different registers for different mode (see BCM 2835 datasheet)
        let fsel_index = (pin / 10) as usize;        // Register index      RPI Exclusive
        let bit_offset = (pin % 10) * 3;             // Bit offset in register

        let fsel = unsafe {
            self.gpio_base.add(fsel_index * 4) as *mut u32
        };

        let mut val = unsafe { fsel.read_volatile() };

        // Clear the existing 3 bits
        val &= !(0b111 << bit_offset);

        // Set bits: 001 = output, 000 = input
        if is_output {
            val |= 0b001 << bit_offset;
        }

        unsafe {
            fsel.write_volatile(val);
        }

        // Update PinSet lists
        let mut inner = self.inner.lock();
        if is_output {
            inner.output.add(pin);
            inner.input.remove(pin);
            inner.alternate.remove(pin);
        } else {
            inner.input.add(pin);
            inner.output.remove(pin);
            inner.alternate.remove(pin);
        }

        dev_info!(self.dev, "GPIO {} configured as {}\n", pin, if is_output { "OUTPUT" } else { "INPUT" });
        Ok(())
    }


    // Returns Pin Information for Specific Pin - Useless? Or for future maybe useful but get all info does the same functionally

    // Returns Information for output Pins
    fn get_output_pins(&self, writer: &mut UserSliceWriter) -> Result<isize> {
        let inner = self.inner.lock();

        let pins = inner.output.as_slice(); // e.g. &[2, 4, 17]
        let valid_len = pins.len().min(28); // max cap is 28

        // Prepare a fixed-size buffer
        let mut buffer = [0i32; 28];
        for (i, pin) in pins.iter().enumerate().take(valid_len) {
            buffer[i] = *pin as i32;
        }

        // Convert buffer into a byte slice
        let bytes: &[u8] = unsafe {
            core::slice::from_raw_parts(
                buffer.as_ptr() as *const u8,
                valid_len * core::mem::size_of::<i32>(),
            )
        };

        // Write to userspace
        writer.write_slice(&bytes)?;

        Ok(0)
    }

    // Returns Information for input Pins
    fn get_input_pins(&self, writer: &mut UserSliceWriter) -> Result<isize> {
        let inner = self.inner.lock();

        let pins = inner.input.as_slice(); // e.g. &[2, 4, 17]
        let valid_len = pins.len().min(28); // max cap is 28

        // Prepare a fixed-size buffer
        let mut buffer = [0i32; 28];
        for (i, pin) in pins.iter().enumerate().take(valid_len) {
            buffer[i] = *pin as i32;
        }

        // Convert buffer into a byte slice
        let bytes: &[u8] = unsafe {
            core::slice::from_raw_parts(
                buffer.as_ptr() as *const u8,
                valid_len * core::mem::size_of::<i32>(),
            )
        };

        // Write to userspace
        writer.write_slice(&bytes)?;

        Ok(0)
    }
}