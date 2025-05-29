// Maximum number of allowed pins in the set
pub const MAX_PINS: usize = 28;

/// Fixed-size GPIO pin set optimized for kernel use (no heap, no dynamic alloc).
pub struct PinSet {
    pins: [u32; MAX_PINS],
    count: usize,
}

impl PinSet {
    /// Create an empty PinSet
    pub const fn new() -> Self {
        Self {
            pins: [0; MAX_PINS],
            count: 0,
        }
    }

    /// Initialize a PinSet from a slice of u32s
    pub fn init_with(pins: &[u32]) -> Self {
        let mut new_set = Self::new();

        for &pin in pins.iter() {
            if new_set.count < MAX_PINS && !new_set.contains(pin) {
                new_set.pins[new_set.count] = pin;
                new_set.count += 1;
            }
        }

        new_set
    }

    /// Check if the set contains a specific pin
    pub fn contains(&self, pin: u32) -> bool {
        self.pins[..self.count].contains(&pin)
    }

    /// Add a pin to the set (if not already present and space allows)
    pub fn add(&mut self, pin: u32) -> bool {
        if self.count >= MAX_PINS || self.contains(pin) {
            return false;
        }

        self.pins[self.count] = pin;
        self.count += 1;
        true
    }

    /// Remove a pin from the set (O(1) unordered removal)
    pub fn remove(&mut self, pin: u32) -> bool {
        if let Some(pos) = self.pins[..self.count].iter().position(|&p| p == pin) {
            self.pins[pos] = self.pins[self.count - 1];
            self.count -= 1;
            return true;
        }
        false
    }

    /// Return number of valid pins in the set
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn as_slice(&self) -> &[u32] {
        &self.pins[..self.count]
    }
}
