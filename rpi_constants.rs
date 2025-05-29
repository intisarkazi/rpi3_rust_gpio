// Raspberry Pi 3 specific constants relating to addresses and pins

pub const GPIO_BASE: u64 = 0x3F200000;      // Base address for GPIO Pins in Rasbpberry Pi 3
pub const GPIO_BLOCK_SIZE: usize = 4096;    // Size of GPIO block       

// ---- SET PULL-DOWN RESISTOR ----
pub const GPPUD_OFFSET: usize = 0x94;
pub const GPPUDCLK0_OFFSET: usize = 0x98;

pub const GPSET0_OFFSET: usize = 28; // Adds onto base to get to output set register (pins 0-32 with u32)
pub const GPCLR0_OFFSET: usize = 40; // Adds onto base to get to output clear register (pins 0-32 with u32)
pub const GPLEV0_OFFSET: usize = 52; // Adds onto base to get to pin level register (pins 0-32 with u32)