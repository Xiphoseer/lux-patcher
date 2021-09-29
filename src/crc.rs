const CRC_POLY: u32 = 0x04C11DB7;
const CRC_INIT: u32 = 0xFFFFFFFF;
const CRC_FXOR: u32 = 0x00000000;

fn update_crc(crc: &mut u32, b: u8) {
    *crc ^= u32::from(b) << 24; /* Move byte to MSB */
    for _i in 0..8 {
        if (*crc & 0x80000000) == 0 {
            *crc <<= 1;
        } else {
            *crc = (*crc << 1) ^ CRC_POLY;
        }
    }
}

pub fn calculate_crc(path: &[u8]) -> u32 {
    let mut crc: u32 = CRC_INIT;
    /* Process the actual string */
    for bp in path {
        let mut b = *bp;
        /* Perform some cleanup on the input */
        if b == b'/' {
            b = b'\\';
        }
        if (b'A'..=b'Z').contains(&b) {
            b += b'a' - b'A';
        }

        update_crc(&mut crc, b);
    }
    /* I have no clue why this was added */
    for _i in 0..4 {
        update_crc(&mut crc, 0);
    }
    crc ^= CRC_FXOR;
    crc
}
