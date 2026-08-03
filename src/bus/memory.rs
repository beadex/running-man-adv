pub fn read_u16_le(memory: &[u8], offset: usize) -> u16 {
    let low = memory.get(offset).copied().unwrap_or(0);

    let high = memory.get(offset + 1).copied().unwrap_or(0);

    u16::from_le_bytes([low, high])
}

pub fn read_u32_le(memory: &[u8], offset: usize) -> u32 {
    let b0 = memory.get(offset).copied().unwrap_or(0);

    let b1 = memory.get(offset + 1).copied().unwrap_or(0);

    let b2 = memory.get(offset + 2).copied().unwrap_or(0);

    let b3 = memory.get(offset + 3).copied().unwrap_or(0);

    u32::from_le_bytes([b0, b1, b2, b3])
}

pub fn write_u16_le(memory: &mut [u8], offset: usize, value: u16) {
    let [low, high] = value.to_le_bytes();

    if let Some(byte) = memory.get_mut(offset) {
        *byte = low;
    }

    if let Some(byte) = memory.get_mut(offset + 1) {
        *byte = high;
    }
}

pub fn write_u32_le(memory: &mut [u8], offset: usize, value: u32) {
    for (index, byte) in value.to_le_bytes().into_iter().enumerate() {
        if let Some(destination) = memory.get_mut(offset + index) {
            *destination = byte;
        }
    }
}
