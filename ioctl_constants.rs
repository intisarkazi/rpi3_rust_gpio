use kernel::ioctl::{_IOWR, _IOR, _IOW};

pub const RUST_MISC_DEV_GPIO_READ: u32 = _IOWR::<i32>('|' as u32, 0x80);
pub const RUST_MISC_DEV_GET_OUTPUTS: u32 = _IOR::<[i32; 28]>('|' as u32, 0x81);
pub const RUST_MISC_DEV_GET_INPUTS: u32 = _IOR::<[i32; 28]>('|' as u32, 0x82);
pub const RUST_MISC_DEV_SET_OUTPUT: u32 = _IOW::<[i32; 2]>('|' as u32, 0x83);
pub const RUST_MISC_DEV_SET_MODE: u32 = _IOW::<[i32; 2]>('|' as u32, 0x84);