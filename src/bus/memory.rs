pub fn read_u16_le(memory: &[u8], offset: usize) -> u16 {
    if let Some(bytes) = memory.get(offset..offset + 2) {
        return u16::from_le_bytes([bytes[0], bytes[1]]);
    }

    let low = memory.get(offset).copied().unwrap_or(0);

    let high = memory.get(offset + 1).copied().unwrap_or(0);

    u16::from_le_bytes([low, high])
}

pub fn read_u32_le(memory: &[u8], offset: usize) -> u32 {
    if let Some(bytes) = memory.get(offset..offset + 4) {
        return u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }

    let b0 = memory.get(offset).copied().unwrap_or(0);

    let b1 = memory.get(offset + 1).copied().unwrap_or(0);

    let b2 = memory.get(offset + 2).copied().unwrap_or(0);

    let b3 = memory.get(offset + 3).copied().unwrap_or(0);

    u32::from_le_bytes([b0, b1, b2, b3])
}

pub fn write_u16_le(memory: &mut [u8], offset: usize, value: u16) {
    let [low, high] = value.to_le_bytes();

    if let Some(bytes) = memory.get_mut(offset..offset + 2) {
        bytes.copy_from_slice(&[low, high]);
        return;
    }

    if let Some(byte) = memory.get_mut(offset) {
        *byte = low;
    }

    if let Some(byte) = memory.get_mut(offset + 1) {
        *byte = high;
    }
}

pub fn write_u32_le(memory: &mut [u8], offset: usize, value: u32) {
    if let Some(bytes) = memory.get_mut(offset..offset + 4) {
        bytes.copy_from_slice(&value.to_le_bytes());
        return;
    }

    for (index, byte) in value.to_le_bytes().into_iter().enumerate() {
        if let Some(destination) = memory.get_mut(offset + index) {
            *destination = byte;
        }
    }
}
