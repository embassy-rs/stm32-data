use stm32_data_serde::chip::Memory;
use stm32_data_serde::chip::memory::{self, Access, Settings};

use crate::util::RegexMap;

struct Mem {
    name: &'static str,
    address: u32,
    size: u32,
    access: Option<Access>,
}

const fn access(access: &'static str) -> Access {
    const fn contains(access: &str, c: u8) -> bool {
        let bytes = access.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == c {
                return true;
            }
            i += 1;
        }
        false
    }

    Access {
        read: contains(access, b'r'),
        write: contains(access, b'w'),
        execute: contains(access, b'x'),
    }
}

macro_rules! mem {
    (@row $name:ident $addr:literal $size_kb:literal) => {
        Mem {
            name: stringify!($name),
            address: $addr,
            size: $size_kb*1024,
            access: None,
        }
    };
    (@row $name:ident $addr:literal $size:literal bytes) => {
        Mem {
            name: stringify!($name),
            address: $addr,
            size: $size,
            access: None,
        }
    };
    (@row $name:ident $addr:literal $size_kb:literal $access:ident) => {
        Mem {
            name: stringify!($name),
            address: $addr,
            size: $size_kb*1024,
            access: Some(access(stringify!($access))),
        }
    };
    (@row $name:ident $addr:literal $size:literal bytes $access:ident) => {
        Mem {
            name: stringify!($name),
            address: $addr,
            size: $size,
            access: Some(access(stringify!($access))),
        }
    };

    ($( $name:ident{$($row:tt)*}),*) => {
        &[
            $(mem!(@row $name $($row)*),)*
        ]
    };
}

#[rustfmt::skip]
static MEMS: RegexMap<&[&[Mem]]> = RegexMap::new(&[
    // C0
    ("STM32C01..4",                  &[mem!(BANK_1 { 0x08000000 16 }, SRAM { 0x20000000 6 })]),
    ("STM32C01..6",                  &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 6 })]),
    ("STM32C03..4",                  &[mem!(BANK_1 { 0x08000000 16 }, SRAM { 0x20000000 12 })]),
    ("STM32C03..6",                  &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 12 })]),
    ("STM32C05..6",                  &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 12 })]),
    ("STM32C05..8",                  &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 12 })]),
    ("STM32C07..8",                  &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 24 })]),
    ("STM32C07..B",                  &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 24 })]),
    ("STM32C091.B",                  &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 36 })]),
    ("STM32C091.C",                  &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 36 })]),
    ("STM32C092.B",                  &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 30 })]),
    ("STM32C092.C",                  &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 30 })]),
    // F0
    ("STM32F0...C",                  &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 32 })]),
    ("STM32F0[35]..8",               &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 8 })]),
    ("STM32F0[47]..6",               &[mem!(BANK_1 { 0x08000000 32 },  SRAM { 0x20000000 6 })]),
    ("STM32F03..4",                  &[mem!(BANK_1 { 0x08000000 16 },  SRAM { 0x20000000 4 })]),
    ("STM32F03..6",                  &[mem!(BANK_1 { 0x08000000 32 },  SRAM { 0x20000000 4 })]),
    ("STM32F04..4",                  &[mem!(BANK_1 { 0x08000000 16 },  SRAM { 0x20000000 6 })]),
    ("STM32F05..4",                  &[mem!(BANK_1 { 0x08000000 16 },  SRAM { 0x20000000 8 })]),
    ("STM32F05..6",                  &[mem!(BANK_1 { 0x08000000 32 },  SRAM { 0x20000000 8 })]),
    ("STM32F07..8",                  &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 16 })]),
    ("STM32F07..B",                  &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 16 })]),
    ("STM32F09..B",                  &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 32 })]),
    // F1
    ("STM32F1.[12].6",               &[mem!(BANK_1 { 0x08000000 32 },  SRAM { 0x20000000 6 })]),
    ("STM32F1.[12].8",               &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 10 })]),
    ("STM32F1.[12].B",               &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 16 })]),
    ("STM32F1.[57].B",               &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 64 })]),
    ("STM32F1.[57].C",               &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 64 })]),
    ("STM32F1.0.6",                  &[mem!(BANK_1 { 0x08000000 32 },  SRAM { 0x20000000 4 })]),
    ("STM32F1.0.8",                  &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 8 })]),
    ("STM32F1.0.B",                  &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 8 })]),
    ("STM32F1.0.C",                  &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 24 })]),
    ("STM32F1.0.D",                  &[mem!(BANK_1 { 0x08000000 384 }, SRAM { 0x20000000 32 })]),
    ("STM32F1.0.E",                  &[mem!(BANK_1 { 0x08000000 512 }, SRAM { 0x20000000 32 })]),
    ("STM32F1.1.C",                  &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 32 })]),
    ("STM32F1.1.D",                  &[mem!(BANK_1 { 0x08000000 384 }, SRAM { 0x20000000 48 })]),
    ("STM32F1.1.E",                  &[mem!(BANK_1 { 0x08000000 512 }, SRAM { 0x20000000 48 })]),
    ("STM32F1.1.F",                  &[mem!(BANK_1 { 0x08000000 512 }, BANK_2 { 0x08080000 256 }, SRAM { 0x20000000 80 })]),
    ("STM32F1.1.G",                  &[mem!(BANK_1 { 0x08000000 512 }, BANK_2 { 0x08080000 512 }, SRAM { 0x20000000 80 })]),
    ("STM32F1.3.6",                  &[mem!(BANK_1 { 0x08000000 32 },  SRAM { 0x20000000 10 })]),
    ("STM32F1.3.8",                  &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 20 })]),
    ("STM32F1.3.B",                  &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 20 })]),
    ("STM32F1.3.C",                  &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 48 })]),
    ("STM32F1.3.D",                  &[mem!(BANK_1 { 0x08000000 384 }, SRAM { 0x20000000 64 })]),
    ("STM32F1.3.E",                  &[mem!(BANK_1 { 0x08000000 512 }, SRAM { 0x20000000 64 })]),
    ("STM32F1.3.F",                  &[mem!(BANK_1 { 0x08000000 512 }, BANK_2 { 0x08080000 256 }, SRAM { 0x20000000 96 })]),
    ("STM32F1.3.G",                  &[mem!(BANK_1 { 0x08000000 512 }, BANK_2 { 0x08080000 512 }, SRAM { 0x20000000 96 })]),
    ("STM32F1.5.8",                  &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 64 })]),
    ("STM32F10[012].4",              &[mem!(BANK_1 { 0x08000000 16 },  SRAM { 0x20000000 4 })]),
    ("STM32F103.4",                  &[mem!(BANK_1 { 0x08000000 16 },  SRAM { 0x20000000 6 })]),
    // F2
    ("STM32F2...B",                  &[mem!(BANK_1 { 0x08000000 128 },  SRAM { 0x20000000 48 },  SRAM2 { 0x2001c000 16 })]),
    ("STM32F2...E",                  &[mem!(BANK_1 { 0x08000000 512 },  SRAM { 0x20000000 112 }, SRAM2 { 0x2001c000 16 })]),
    ("STM32F2...F",                  &[mem!(BANK_1 { 0x08000000 768 },  SRAM { 0x20000000 112 }, SRAM2 { 0x2001c000 16 })]),
    ("STM32F2...G",                  &[mem!(BANK_1 { 0x08000000 1024 }, SRAM { 0x20000000 112 }, SRAM2 { 0x2001c000 16 })]),
    ("STM32F2.5.C",                  &[mem!(BANK_1 { 0x08000000 256 },  SRAM { 0x20000000 80 },  SRAM2 { 0x2001c000 16 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F2.7.C",                  &[mem!(BANK_1 { 0x08000000 256 },  SRAM { 0x20000000 112 }, SRAM2 { 0x2001c000 16 }, OTP { 0x1fff7800 528 bytes })]),
    // F3. TODO: CCM is in both buses.
    ("STM32F3...4",                  &[mem!(BANK_1 { 0x08000000 16 },  SRAM { 0x20000000 12 }, CCMRAM { 0x10000000 4 })]),
    ("STM32F3.[12].6",               &[mem!(BANK_1 { 0x08000000 32 },  SRAM { 0x20000000 16 })]),
    ("STM32F3.[34].6",               &[mem!(BANK_1 { 0x08000000 32 },  SRAM { 0x20000000 12 }, CCMRAM { 0x10000000 4 })]),
    ("STM32F3.[38].E",               &[mem!(BANK_1 { 0x08000000 512 }, SRAM { 0x20000000 64 }, CCMRAM { 0x10000000 16 })]),
    ("STM32F3.2.D",                  &[mem!(BANK_1 { 0x08000000 384 }, SRAM { 0x20000000 64 })]),
    ("STM32F3.2.E",                  &[mem!(BANK_1 { 0x08000000 512 }, SRAM { 0x20000000 64 })]),
    ("STM32F3.3.D",                  &[mem!(BANK_1 { 0x08000000 384 }, SRAM { 0x20000000 64 }, CCMRAM { 0x10000000 16 })]),
    ("STM32F3([23]..8|03.8)",        &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 12 }, CCMRAM { 0x10000000 4 })]),
    ("STM32F3(0[12].8|[17]..8)",     &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 16 })]),
    ("STM32F3(03|58).C",             &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 40 }, CCMRAM { 0x10000000 8 })]),
    ("STM32F3(73|78).C",             &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 32 })]),
    ("STM32F302.B",                  &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 32 })]),
    ("STM32F302.C",                  &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 40 })]),
    ("STM32F303.B",                  &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 32 }, CCMRAM { 0x10000000 8 })]),
    ("STM32F373.B",                  &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 24 })]),
    // F4
    ("STM32F4...8",                  &[mem!(BANK_1 { 0x08000000 64 },   SRAM { 0x20000000 32 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F4...D",                  &[mem!(BANK_1 { 0x08000000 384 },  SRAM { 0x20000000 96 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F4...H",                  &[mem!(BANK_1 { 0x08000000 1536 }, SRAM { 0x20000000 320 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F4(05|07).E",             &[mem!(BANK_1 { 0x08000000 512 },  SRAM { 0x20000000 112 }, SRAM2 { 0x2001c000 16 }, CCMRAM { 0x10000000 64 rw }, OTP { 0x1fff7800 528 bytes } )]),
    ("STM32F4(11|46).E",             &[mem!(BANK_1 { 0x08000000 512 },  SRAM { 0x20000000 128 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F4[14]..C",               &[mem!(BANK_1 { 0x08000000 256 },  SRAM { 0x20000000 128 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F4[23]..G",               &[
                                        mem!(BANK_1 { 0x08000000 1024 },                            SRAM { 0x20000000 192 }, CCMRAM { 0x10000000 64 rw }, OTP { 0x1fff7800 528 bytes }),
                                        mem!(BANK_1 { 0x08000000 512  }, BANK_2 { 0x08080000 512 }, SRAM { 0x20000000 192 }, CCMRAM { 0x10000000 64 rw }, OTP { 0x1fff7800 528 bytes }),
                                      ]),
    ("STM32F4[23]..I",               &[mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08100000 1024 }, SRAM { 0x20000000 192 }, CCMRAM { 0x10000000 64 rw }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F4[67]..G",               &[
                                        mem!(BANK_1 { 0x08000000 1024 },                             SRAM { 0x20000000 320 }, CCMRAM { 0x10000000 64 rw }, OTP { 0x1fff7800 528 bytes }),
                                        mem!(BANK_1 { 0x08000000  512 }, BANK_2 { 0x08080000  512 }, SRAM { 0x20000000 320 }, CCMRAM { 0x10000000 64 rw }, OTP { 0x1fff7800 528 bytes }),
                                      ]),
    ("STM32F4[67]..I",               &[mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08100000 1024 }, SRAM { 0x20000000 320 }, CCMRAM { 0x10000000 64 rw }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F40..B",                  &[mem!(BANK_1 { 0x08000000 128 },  SRAM { 0x20000000 64 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F40..C",                  &[mem!(BANK_1 { 0x08000000 256 },  SRAM { 0x20000000 64 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F40..G",                  &[mem!(BANK_1 { 0x08000000 1024 }, SRAM { 0x20000000 112 },SRAM2 { 0x2001c000 16 }, CCMRAM { 0x10000000 64 rw }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F401.E",                  &[mem!(BANK_1 { 0x08000000 512 },  SRAM { 0x20000000 96 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F41..B",                  &[mem!(BANK_1 { 0x08000000 128 },  SRAM { 0x20000000 32 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F41[57].G",               &[mem!(BANK_1 { 0x08000000 1024 }, SRAM { 0x20000000 128 }, CCMRAM { 0x10000000 64 rw }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F412.E",                  &[mem!(BANK_1 { 0x08000000 512 },  SRAM { 0x20000000 256 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F412.G",                  &[mem!(BANK_1 { 0x08000000 1024 }, SRAM { 0x20000000 256 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F413.G",                  &[mem!(BANK_1 { 0x08000000 1024 }, SRAM { 0x20000000 320 }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F417.E",                  &[mem!(BANK_1 { 0x08000000 512 },  SRAM { 0x20000000 128 }, CCMRAM { 0x10000000 64 rw }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F429.E",                  &[mem!(BANK_1 { 0x08000000 512 },  SRAM { 0x20000000 192 }, CCMRAM { 0x10000000 64 rw }, OTP { 0x1fff7800 528 bytes })]),
    ("STM32F469.E",                  &[mem!(BANK_1 { 0x08000000 512 },  SRAM { 0x20000000 320 }, CCMRAM { 0x10000000 64 rw }, OTP { 0x1fff7800 528 bytes })]),
    // F7
    ("STM32F7...C",                  &[mem!(BANK_1 { 0x08000000 256 },  DTCM { 0x20000000 64 },  SRAM { 0x20010000 192 })]),
    ("STM32F7[67]..I",               &[
                                        mem!(BANK_1 { 0x08000000 2048 },                             DTCM { 0x20000000 128 }, SRAM { 0x20020000 384 }),
                                        mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08100000 1024 }, DTCM { 0x20000000 128 }, SRAM { 0x20020000 384 }),
                                      ]),
    ("STM32F7...I",                  &[mem!(BANK_1 { 0x08000000 2048 }, DTCM { 0x20000000 128 }, SRAM { 0x20020000 384 })]),
    ("STM32F7[23]..E",               &[mem!(BANK_1 { 0x08000000 512 },  DTCM { 0x20000000 64 },  SRAM { 0x20010000 192 }, OTP { 0x1ff07800 528 bytes })]),
    ("STM32F7[45]..G",               &[mem!(BANK_1 { 0x08000000 1024 }, DTCM { 0x20000000 64 },  SRAM { 0x20010000 256 }, OTP { 0x08fff000 2 })]),
    ("STM32F73..8",                  &[mem!(BANK_1 { 0x08000000 64 },   DTCM { 0x20000000 64 },  SRAM { 0x20010000 192 })]),
    ("STM32F74..E",                  &[mem!(BANK_1 { 0x08000000 512 },  DTCM { 0x20000000 64 },  SRAM { 0x20010000 256 }, OTP { 0x08fff000 2 })]),
    ("STM32F75..8",                  &[mem!(BANK_1 { 0x08000000 64 },   DTCM { 0x20000000 64 },  SRAM { 0x20010000 256 }, OTP { 0x1ff0f000 1 })]),
    ("STM32F76..G",                  &[
                                        mem!(BANK_1 { 0x08000000 1024 },                             DTCM { 0x20000000 128 }, SRAM { 0x20020000 384 }, OTP { 0x1ff0f000 1 }),
                                        mem!(BANK_1 { 0x08000000 512  }, BANK_2 { 0x08080000 512  }, DTCM { 0x20000000 128 }, SRAM { 0x20020000 384 }, OTP { 0x1ff0f000 1 }),
                                      ]),
    // G0
    ("STM32G0...4",                  &[mem!(BANK_1 { 0x08000000 16 },  SRAM { 0x20000000 8 })]),
    ("STM32G0...C",                  &[
                                        mem!(BANK_1 { 0x08000000 256 },                            SRAM { 0x20000000 144 }),
                                        mem!(BANK_1 { 0x08000000 128 }, BANK_2 { 0x08020000 128 }, SRAM { 0x20000000 144 }),
                                      ]),
    ("STM32G0...E",                  &[mem!(BANK_1 { 0x08000000 256 }, BANK_2 { 0x08040000 256 }, SRAM { 0x20000000 144 })]),
    ("STM32G0[34]..6",               &[mem!(BANK_1 { 0x08000000 32 },  SRAM { 0x20000000 8 })]),
    ("STM32G0[34]..8",               &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 8 })]),
    ("STM32G0[56]..6",               &[mem!(BANK_1 { 0x08000000 32 },  SRAM { 0x20000000 18 })]),
    ("STM32G0[56]..8",               &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 18 })]),
    ("STM32G0[78]..B",               &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 36 })]),
    ("STM32G07..6",                  &[mem!(BANK_1 { 0x08000000 32 },  SRAM { 0x20000000 36 })]),
    ("STM32G07..8",                  &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 36 })]),
    ("STM32G0B..B",                  &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 144 })]),
    // G4
    ("STM32G4...6",                  &[mem!(BANK_1 { 0x08000000 32 },  CCMRAM_ICODE { 0x10000000 10 }, SRAM1 { 0x20000000 16 }, SRAM2 { 0x20004000 6 }, CCMRAM_DCODE { 0x20005800 10 })]),
    ("STM32G4...8",                  &[mem!(BANK_1 { 0x08000000 64 },  CCMRAM_ICODE { 0x10000000 10 }, SRAM1 { 0x20000000 16 }, SRAM2 { 0x20004000 6 }, CCMRAM_DCODE { 0x20005800 10 })]),
    ("STM32G4[34]..B",               &[mem!(BANK_1 { 0x08000000 128 }, CCMRAM_ICODE { 0x10000000 10 }, SRAM1 { 0x20000000 16 }, SRAM2 { 0x20004000 6 }, CCMRAM_DCODE { 0x20005800 10 })]),
    ("STM32G49..C",                  &[mem!(BANK_1 { 0x08000000 256 }, CCMRAM_ICODE { 0x10000000 16 }, SRAM1 { 0x20000000 80 }, SRAM2 { 0x20014000 16 }, CCMRAM_DCODE { 0x20018000 16 })]),
    ("STM32G4[9A]..E",               &[mem!(BANK_1 { 0x08000000 512 }, CCMRAM_ICODE { 0x10000000 16 }, SRAM1 { 0x20000000 80 }, SRAM2 { 0x20014000 16 }, CCMRAM_DCODE { 0x20018000 16 })]),
    ("STM32G47..B",                  &[
                                        mem!(BANK_1 { 0x08000000 128 },                            CCMRAM_ICODE { 0x10000000 32 }, SRAM1 { 0x20000000 80 }, SRAM2 { 0x20014000 16 }, CCMRAM_DCODE { 0x20018000 32 }),
                                        mem!(BANK_1 { 0x08000000 64 },  BANK_2 { 0x08040000 64 },  CCMRAM_ICODE { 0x10000000 32 }, SRAM1 { 0x20000000 80 }, SRAM2 { 0x20014000 16 }, CCMRAM_DCODE { 0x20018000 32 })
                                      ]),
    ("STM32G47..C",                  &[
                                        mem!(BANK_1 { 0x08000000 256 },                            CCMRAM_ICODE { 0x10000000 32 }, SRAM1 { 0x20000000 80 }, SRAM2 { 0x20014000 16 }, CCMRAM_DCODE { 0x20018000 32 }),
                                        mem!(BANK_1 { 0x08000000 128 }, BANK_2 { 0x08040000 128 }, CCMRAM_ICODE { 0x10000000 32 }, SRAM1 { 0x20000000 80 }, SRAM2 { 0x20014000 16 }, CCMRAM_DCODE { 0x20018000 32 })
                                      ]),
    ("STM32G4[78]..E",               &[
                                        mem!(BANK_1 { 0x08000000 512 },                            CCMRAM_ICODE { 0x10000000 32 }, SRAM1 { 0x20000000 80 }, SRAM2 { 0x20014000 16 }, CCMRAM_DCODE { 0x20018000 32 }),
                                        mem!(BANK_1 { 0x08000000 256 }, BANK_2 { 0x08040000 256 }, CCMRAM_ICODE { 0x10000000 32 }, SRAM1 { 0x20000000 80 }, SRAM2 { 0x20014000 16 }, CCMRAM_DCODE { 0x20018000 32 })
                                      ]),
    // H5
    ("STM32H5...B",                  &[mem!(BANK_1 { 0x08000000 64 },   BANK_2 { 0x08010000 64 },   SRAM1 { 0x20000000 16 },  SRAM2 { 0x20004000 16 },                           BKPSRAM { 0x40036400 2 }, OTP { 0x08fff000 2 }, FMC_BANK_1 { 0x70000000 256000}, FMC_BANK_3 {  0x7fffffff 256000 }, SDRAM_BANK_1 { 0xc0000000 256000}, SDRAM_BANK_2 { 0xd0000000 256000}, OCTOSPI_BANK_1 { 0xa0000000 256000})]),
    ("STM32H5...C",                  &[mem!(BANK_1 { 0x08000000 128 },  BANK_2 { 0x08020000 128 },  SRAM1 { 0x20000000 128 }, SRAM2 { 0x20020000 80 }, SRAM3 { 0x20034000 64 },  BKPSRAM { 0x40036400 2 }, OTP { 0x08fff000 2 }, FMC_BANK_1 { 0x70000000 256000}, FMC_BANK_3 {  0x7fffffff 256000 }, SDRAM_BANK_1 { 0xc0000000 256000}, SDRAM_BANK_2 { 0xd0000000 256000}, OCTOSPI_BANK_1 { 0xa0000000 256000})]),
    ("STM32H5...E",                  &[mem!(BANK_1 { 0x08000000 256 },  BANK_2 { 0x08040000 256 },  SRAM1 { 0x20000000 128 }, SRAM2 { 0x20020000 80 }, SRAM3 { 0x20034000 64 },  BKPSRAM { 0x40036400 2 }, OTP { 0x08fff000 2 }, FMC_BANK_1 { 0x70000000 256000}, FMC_BANK_3 {  0x7fffffff 256000 }, SDRAM_BANK_1 { 0xc0000000 256000}, SDRAM_BANK_2 { 0xd0000000 256000}, OCTOSPI_BANK_1 { 0xa0000000 256000})]),
    ("STM32H5...G",                  &[mem!(BANK_1 { 0x08000000 512 },  BANK_2 { 0x08080000 512 },  SRAM1 { 0x20000000 256 }, SRAM2 { 0x20040000 64 }, SRAM3 { 0x20050000 320 }, BKPSRAM { 0x40036400 4 }, OTP { 0x08fff000 2 }, FMC_BANK_1 { 0x70000000 256000}, FMC_BANK_3 {  0x7fffffff 256000 }, SDRAM_BANK_1 { 0xc0000000 256000}, SDRAM_BANK_2 { 0xd0000000 256000}, OCTOSPI_BANK_1 { 0xa0000000 256000})]),
    ("STM32H5...I",                  &[mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08100000 1024 }, SRAM1 { 0x20000000 256 }, SRAM2 { 0x20040000 64 }, SRAM3 { 0x20050000 320 }, BKPSRAM { 0x40036400 4 }, OTP { 0x08fff000 2 }, FMC_BANK_1 { 0x70000000 256000}, FMC_BANK_3 {  0x7fffffff 256000 }, SDRAM_BANK_1 { 0xc0000000 256000}, SDRAM_BANK_2 { 0xd0000000 256000}, OCTOSPI_BANK_1 { 0xa0000000 256000})]),
    // H7RS
    ("STM32H7[RS].*",                &[mem!(BANK_1 { 0x08000000 64 }, ITCM { 0x00000000 192 }, DTCM { 0x20000000 192 }, SRAM1 { 0x24000000 128 }, SRAM2 { 0x24020000 128 }, SRAM3 { 0x24040000 128 }, SRAM4 { 0x24060000 72 }, AHB_SRAM1 { 0x30000000 16 }, AHB_SRAM2 { 0x30004000 16 })]),
    // H7
    // --- RM0468 - H7[23]
    ("STM32H7[23]..B",               &[mem!(BANK_1 { 0x08000000 128 },                              ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 320 }, SRAM2 { 0x30000000 16 }, SRAM3 { 0x30004000 16 }, SRAM4 { 0x38000000 16 })]),
    ("STM32H7[23]..E",               &[mem!(BANK_1 { 0x08000000 512 },                              ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 320 }, SRAM2 { 0x30000000 16 }, SRAM3 { 0x30004000 16 }, SRAM4 { 0x38000000 16 })]),
    ("STM32H7[23]..G",               &[mem!(BANK_1 { 0x08000000 1024 },                             ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 320 }, SRAM2 { 0x30000000 16 }, SRAM3 { 0x30004000 16 }, SRAM4 { 0x38000000 16 })]),
    // --- RM0433 - H742/43/53/50
    ("STM32H742.G",                  &[mem!(BANK_1 { 0x08000000 512 },  BANK_2 { 0x08100000 512 },  ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 384 }, SRAM1 { 0x30000000 32 },  SRAM2 { 0x30020000 16 }, SRAM4 { 0x38000000 64 })]),
    ("STM32H742.I",                  &[mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08100000 1024 }, ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 384 }, SRAM1 { 0x30000000 32 },  SRAM2 { 0x30020000 16 }, SRAM4 { 0x38000000 64 })]),
    ("STM32H7[45]3.G",               &[mem!(BANK_1 { 0x08000000 512 },  BANK_2 { 0x08100000 512 },  ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 512 }, SRAM1 { 0x30000000 128 }, SRAM2 { 0x30020000 128 }, SRAM3 { 0x30040000 32 }, SRAM4 { 0x38000000 64 })]),
    ("STM32H7[45]3.I",               &[mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08100000 1024 }, ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 512 }, SRAM1 { 0x30000000 128 }, SRAM2 { 0x30020000 128 }, SRAM3 { 0x30040000 32 }, SRAM4 { 0x38000000 64 })]),
    ("STM32H750.B",                  &[mem!(BANK_1 { 0x08000000 128 },                              ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 512 }, SRAM1 { 0x30000000 128 }, SRAM2 { 0x30020000 128 }, SRAM3 { 0x30040000 32 }, SRAM4 { 0x38000000 64 })]),
    // --- RM0399 - H745/55/47/57
    ("STM32H7[45][57].G",            &[mem!(BANK_1 { 0x08000000 512 },  BANK_2 { 0x08100000 512 },  ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 512 }, SRAM1 { 0x30000000 128 }, SRAM2 { 0x30020000 128 }, SRAM3 { 0x30040000 32 }, SRAM4 { 0x38000000 64 })]),
    ("STM32H7[45][57].I",            &[mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08100000 1024 }, ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 512 }, SRAM1 { 0x30000000 128 }, SRAM2 { 0x30020000 128 }, SRAM3 { 0x30040000 32 }, SRAM4 { 0x38000000 64 })]),
    // --- RM0455 - H7[AB]
    ("STM32H7[AB]3.I",               &[mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08100000 1024 }, ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 1024 }, AHBSRAM { 0x30000000 128 })]),
    ("STM32H7A..G",                  &[mem!(BANK_1 { 0x08000000 512 },  BANK_2 { 0x08100000 512 },  ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 1024 }, AHBSRAM { 0x30000000 128 })]),
    ("STM32H7B..B",                  &[mem!(BANK_1 { 0x08000000 128 },                              ITCM { 0x00000000 64 }, DTCM { 0x20000000 128 },  AXISRAM { 0x24000000 1024 }, AHBSRAM { 0x30000000 128 })]),
    // L0
    // L0x0
    ("STM32L010.4", &[mem!(BANK_1 { 0x08000000 16 }, SRAM { 0x20000000 2 }, EEPROM { 0x08080000 128 bytes rw })]), // STM32L010F4, K4
    ("STM32L010.6", &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 256 bytes rw })]), // STM32L010C6
    ("STM32L010.8", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 256 bytes rw })]), // STM32L010K8, R8
    ("STM32L010.B", &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 20 }, EEPROM { 0x08080000 512 bytes rw })]), // STM32L010RB
    // L0x1 Category 1
    ("STM32L011.3", &[mem!(BANK_1 { 0x08000000 8 }, SRAM { 0x20000000 2 }, EEPROM { 0x08080000 512 bytes rw })]), // STM32L011D3, E3, F3, G3, K3
    ("STM32L011.4", &[mem!(BANK_1 { 0x08000000 16 }, SRAM { 0x20000000 2 }, EEPROM { 0x08080000 512 bytes rw })]), // STM32L011D4, E4, F4, G4, K4
    ("STM32L021.4", &[mem!(BANK_1 { 0x08000000 16 }, SRAM { 0x20000000 2 }, EEPROM { 0x08080000 512 bytes rw })]), // STM32L021D4, F4, G4, K4
    // L0x1 Category 2
    ("STM32L031.4", &[mem!(BANK_1 { 0x08000000 16 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 1024 bytes rw })]), // STM32L031C4, E4, F4, G4, K4
    ("STM32L031.6", &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 1024 bytes rw })]), // STM32L031C6, E6, F6, G6, K6
    ("STM32L041.4", &[mem!(BANK_1 { 0x08000000 16 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 1024 bytes rw })]), // STM32L041C4
    ("STM32L041.6", &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 1024 bytes rw })]), // STM32L041C6, E6, F6, G6, K6
    // L0x1, L0x2, L0x3 Category 3
    ("STM32L051.6", &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 2048 bytes rw })]), // STM32L051C6, K6, R6, T6
    ("STM32L051.8", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 2048 bytes rw })]), // STM32L051C8, K8, R8, T8
    ("STM32L052.6", &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 2048 bytes rw })]), // STM32L052C6, K6, R6, T6
    ("STM32L052.8", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 2048 bytes rw })]), // STM32L052C8, K8, R8, T8
    ("STM32L053.6", &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 2048 bytes rw })]), // STM32L053C6, R6
    ("STM32L053.8", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 2048 bytes rw })]), // STM32L053C8, R8
    ("STM32L062.8", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 2048 bytes rw })]), // STM32L062C8, K8
    ("STM32L063.8", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 2048 bytes rw })]), // STM32L063C8, R8
    // L0x1, L0x2, L0x3 Category 5 (64KB Flash, only EEPROM bank 2)
    ("STM32L071.8", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 20 }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L071C8, K8, V8
    ("STM32L072.8", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 20 }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L072V8
    ("STM32L073.8", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 20 }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L073V8
    ("STM32L083.8", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 20 }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L083V8
    // L0x1, L0x2, L0x3 Category 5 (128KB and 192KB Flash, dual EEPROM banks)
    ("STM32L071.B", &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 20 }, EEPROM_BANK_1 { 0x08080000 3072 bytes rw }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L071CB, KB, RB, VB
    ("STM32L071.Z", &[mem!(BANK_1 { 0x08000000 192 }, SRAM { 0x20000000 20 }, EEPROM_BANK_1 { 0x08080000 3072 bytes rw }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L071CZ, KZ, RZ, VZ
    ("STM32L072.B", &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 20 }, EEPROM_BANK_1 { 0x08080000 3072 bytes rw }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L072CB, KB, RB, VB
    ("STM32L072.Z", &[mem!(BANK_1 { 0x08000000 192 }, SRAM { 0x20000000 20 }, EEPROM_BANK_1 { 0x08080000 3072 bytes rw }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L072CZ, KZ, RZ, VZ
    ("STM32L073.B", &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 20 }, EEPROM_BANK_1 { 0x08080000 3072 bytes rw }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L073CB, RB, VB
    ("STM32L073.Z", &[mem!(BANK_1 { 0x08000000 192 }, SRAM { 0x20000000 20 }, EEPROM_BANK_1 { 0x08080000 3072 bytes rw }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L073CZ, RZ, VZ
    ("STM32L081.B", &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 20 }, EEPROM_BANK_1 { 0x08080000 3072 bytes rw }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L081CB
    ("STM32L081.Z", &[mem!(BANK_1 { 0x08000000 192 }, SRAM { 0x20000000 20 }, EEPROM_BANK_1 { 0x08080000 3072 bytes rw }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L081CZ, KZ
    ("STM32L082.B", &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 20 }, EEPROM_BANK_1 { 0x08080000 3072 bytes rw }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L082KB
    ("STM32L082.Z", &[mem!(BANK_1 { 0x08000000 192 }, SRAM { 0x20000000 20 }, EEPROM_BANK_1 { 0x08080000 3072 bytes rw }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L082CZ, KZ
    ("STM32L083.B", &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 20 }, EEPROM_BANK_1 { 0x08080000 3072 bytes rw }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L083CB, RB, VB
    ("STM32L083.Z", &[mem!(BANK_1 { 0x08000000 192 }, SRAM { 0x20000000 20 }, EEPROM_BANK_1 { 0x08080000 3072 bytes rw }, EEPROM_BANK_2 { 0x08080C00 3072 bytes rw })]), // STM32L083CZ, RZ, VZ
    // L1
    ("STM32L1...C..", &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 32 }, EEPROM { 0x08080000 8192 bytes rw })]), // Cat.3
    ("STM32L1...D", &[mem!(BANK_1 { 0x08000000 192 }, BANK_2 { 0x08030000 192 }, SRAM { 0x20000000 48 }, EEPROM_BANK_1 { 0x08080000 6144 bytes rw }, EEPROM_BANK_2 { 0x08081800 6144 bytes rw })]), // Cat.4
    ("STM32L1...D..", &[mem!(BANK_1 { 0x08000000 192 }, BANK_2 { 0x08030000 192 }, SRAM { 0x20000000 80 }, EEPROM_BANK_1 { 0x08080000 8192 bytes rw }, EEPROM_BANK_2 { 0x08082000 8192 bytes rw })]), // Cat.5/6 (e.g., STM32L151VD-X)
    ("STM32L1...E", &[mem!(BANK_1 { 0x08000000 256 }, BANK_2 { 0x08040000 256 }, SRAM { 0x20000000 80 }, EEPROM_BANK_1 { 0x08080000 8192 bytes rw }, EEPROM_BANK_2 { 0x08082000 8192 bytes rw })]), // Cat.5/6
    ("STM32L10..6..", &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 4 }, EEPROM { 0x08080000 4096 bytes rw })]), // Cat.1/2 (STM32L100C6-A)
    ("STM32L10..6", &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 4 }, EEPROM { 0x08080000 4096 bytes rw })]), // Cat.1/2 (STM32L100C6)
    ("STM32L10..8..", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 4096 bytes rw })]), // Cat.1/2 (STM32L100R8-A)
    ("STM32L10..8", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 8 }, EEPROM { 0x08080000 4096 bytes rw })]), // Cat.1/2 (STM32L100R8)
    ("STM32L10..B..", &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 16 }, EEPROM { 0x08080000 4096 bytes rw })]), // Cat.1/2 (STM32L100RB-A)
    ("STM32L10..B", &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 10 }, EEPROM { 0x08080000 4096 bytes rw })]), // Cat.1/2 (STM32L100RB)
    ("STM32L10..C", &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 16 }, EEPROM { 0x08080000 8192 bytes rw })]), // Cat.3 (STM32L100RC)
    ("STM32L15..6..", &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 16 }, EEPROM { 0x08080000 4096 bytes rw })]), // Cat.1/2 (STM32L151C6-A, etc.)
    ("STM32L15..6", &[mem!(BANK_1 { 0x08000000 32 }, SRAM { 0x20000000 10 }, EEPROM { 0x08080000 4096 bytes rw })]), // Cat.1/2 (STM32L151C6, etc.)
    ("STM32L15..8..", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 32 }, EEPROM { 0x08080000 4096 bytes rw })]), // Cat.1/2 (STM32L151C8-A, etc.)
    ("STM32L15..8", &[mem!(BANK_1 { 0x08000000 64 }, SRAM { 0x20000000 10 }, EEPROM { 0x08080000 4096 bytes rw })]), // Cat.1/2 (STM32L151C8, etc.)
    ("STM32L15..B..", &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 32 }, EEPROM { 0x08080000 4096 bytes rw })]), // Cat.1/2 (STM32L151CB-A, etc.)
    ("STM32L15..B", &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 16 }, EEPROM { 0x08080000 4096 bytes rw })]), // Cat.1/2 (STM32L151CB, etc.)
    ("STM32L15..C", &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 32 }, EEPROM { 0x08080000 8192 bytes rw })]), // Cat.3
    ("STM32L16..C", &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 32 }, EEPROM { 0x08080000 8192 bytes rw })]), // Cat.3
    ("STM32L16..D", &[mem!(BANK_1 { 0x08000000 384 }, SRAM { 0x20000000 48 }, EEPROM_BANK_1 { 0x08080000 6144 bytes rw }, EEPROM_BANK_2 { 0x08081800 6144 bytes rw })]), // Cat.4
    ("STM32L16..E", &[mem!(BANK_1 { 0x08000000 512 }, SRAM { 0x20000000 80 }, EEPROM_BANK_1 { 0x08080000 8192 bytes rw }, EEPROM_BANK_2 { 0x08082000 8192 bytes rw })]), // Cat.5/6
    // L4
    ("STM32L4...8",                  &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 32 },  SRAM2 { 0x20008000 8 },  SRAM2_ICODE { 0x10000000 8 })]),
    ("STM32L4[12]..B",               &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 32 },  SRAM2 { 0x20008000 8 },  SRAM2_ICODE { 0x10000000 8 })]),
    ("STM32L43..B",                  &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 48 },  SRAM2 { 0x2000c000 16 }, SRAM2_ICODE { 0x10000000 16 })]),
    ("STM32L45..C",                  &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 128 }, SRAM2 { 0x20020000 32 }, SRAM2_ICODE { 0x10000000 32 })]),
    ("STM32L4[34]..C",               &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 48 },  SRAM2 { 0x2000c000 16 }, SRAM2_ICODE { 0x10000000 16 })]),
    ("STM32L4[56]..E",               &[mem!(BANK_1 { 0x08000000 512 }, SRAM { 0x20000000 128 }, SRAM2 { 0x20020000 32 }, SRAM2_ICODE { 0x10000000 32 })]),

    ("STM32L47..C",                  &[mem!(BANK_1 { 0x08000000 128 }, BANK_2 { 0x08020000 128 }, SRAM { 0x20000000 96 },  SRAM2_ICODE { 0x10000000 32 })]),
    ("STM32L47..E",                  &[mem!(BANK_1 { 0x08000000 256 }, BANK_2 { 0x08040000 256 }, SRAM { 0x20000000 96 },  SRAM2_ICODE { 0x10000000 32 })]),
    ("STM32L49..E",                  &[mem!(BANK_1 { 0x08000000 256 }, BANK_2 { 0x08040000 256 }, SRAM { 0x20000000 256 }, SRAM2 { 0x20040000 64 }, SRAM2_ICODE { 0x10000000 64 })]),
    ("STM32L4[78]..G",               &[mem!(BANK_1 { 0x08000000 512 }, BANK_2 { 0x08080000 512 }, SRAM { 0x20000000 96 },  SRAM2_ICODE { 0x10000000 32 })]),
    ("STM32L4[9A]..G",               &[mem!(BANK_1 { 0x08000000 512 }, BANK_2 { 0x08080000 512 }, SRAM { 0x20000000 256 }, SRAM2 { 0x20040000 64 }, SRAM2_ICODE { 0x10000000 64 })]),
    // L4+
    ("STM32L4P..E",                  &[
                                        mem!(BANK_1 { 0x08000000 512 },                             SRAM { 0x20000000 320 }),
                                        mem!(BANK_1 { 0x08000000 256 },  BANK_2 { 0x08040000 256 }, SRAM { 0x20000000 320 }),
                                      ]),
    ("STM32L4[PQ]..G",               &[
                                        mem!(BANK_1 { 0x08000000 1024 },                            SRAM { 0x20000000 320 }),
                                        mem!(BANK_1 { 0x08000000  512 }, BANK_2 { 0x08080000 512 }, SRAM { 0x20000000 320 })
                                      ]),
    ("STM32L4R..G",                  &[
                                        mem!(BANK_1 { 0x08000000 1024 },                            SRAM { 0x20000000 640 }),
                                        mem!(BANK_1 { 0x08000000  512 }, BANK_2 { 0x08080000 512 }, SRAM { 0x20000000 640 }),
                                      ]),
    ("STM32L4[RS]..I",               &[
                                        mem!(BANK_1 { 0x08000000 2048 },                              SRAM { 0x20000000 640 }),
                                        mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08100000 1024 }, SRAM { 0x20000000 640 })
                                      ]),
    // L5
    ("STM32L5...C",                  &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 256 })]),
    ("STM32L5...E",                  &[
                                        mem!(BANK_1 { 0x08000000 512 },                            SRAM { 0x20000000 256 }),
                                        mem!(BANK_1 { 0x08000000 256 }, BANK_2 { 0x08040000 256 }, SRAM { 0x20000000 256 }),
                                      ]),
    // N6
    ("STM32N6...0",                  &[mem!(FLEXRAM {0x24000000 400}, AXISRAM { 0x24064000 624 }, AXISRAM2 { 0x24100000 1024 }, AXISRAM3 {0x24200000 448}, AXISRAM4 {0x24270000 448}, AXISRAM5 {0x242E0000 448}, AXISRAM6 {0x24350000 448}, NPURAM {0x243C0000 256}, VENCRAM {0x24400000 128})]),
    // U0
    ("STM32U031.4",                  &[mem!(BANK_1 { 0x08000000 16 },  SRAM { 0x20000000 12 }, OTP { 0x1fff6800 1 })]),
    ("STM32U031.6",                  &[mem!(BANK_1 { 0x08000000 32 },  SRAM { 0x20000000 12 }, OTP { 0x1fff6800 1 })]),
    ("STM32U031.8",                  &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 12 }, OTP { 0x1fff6800 1 })]),
    ("STM32U0[78]3.8",               &[mem!(BANK_1 { 0x08000000 64 },  SRAM { 0x20000000 40 }, OTP { 0x1fff6800 1 })]),
    ("STM32U0[78]3.B",               &[mem!(BANK_1 { 0x08000000 128 }, SRAM { 0x20000000 40 }, OTP { 0x1fff6800 1 })]),
    ("STM32U0[78]3.C",               &[mem!(BANK_1 { 0x08000000 256 }, SRAM { 0x20000000 40 }, OTP { 0x1fff6800 1 })]),
    // U5
    ("STM32U5[34]..B",               &[mem!(BANK_1 { 0x08000000 64 },   BANK_2 { 0x08010000 64 },   SRAM { 0x20000000 192 }, SRAM2 { 0x20030000 64 }, OTP { 0x0bfa0000 512 bytes })]),
    ("STM32U5[34]..C",               &[mem!(BANK_1 { 0x08000000 128 },  BANK_2 { 0x08020000 128 },  SRAM { 0x20000000 192 }, SRAM2 { 0x20030000 64 }, OTP { 0x0bfa0000 512 bytes })]),
    ("STM32U5[43]..E",               &[mem!(BANK_1 { 0x08000000 256 },  BANK_2 { 0x08040000 256 },  SRAM { 0x20000000 192 }, SRAM2 { 0x20030000 64 }, OTP { 0x0bfa0000 512 bytes })]),
    ("STM32U5[78]..G",               &[mem!(BANK_1 { 0x08000000 512 },  BANK_2 { 0x08080000 512 },  SRAM { 0x20000000 192 }, SRAM2 { 0x20030000 64 }, SRAM3 { 0x20040000 512 }, OTP { 0x0bfa0000 512 bytes })]),
    ("STM32U5[78]..I",               &[mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08100000 1024 }, SRAM { 0x20000000 192 }, SRAM2 { 0x20030000 64 }, SRAM3 { 0x20040000 512 }, OTP { 0x0bfa0000 512 bytes })]),
    ("STM32U5[9A]..I",               &[mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08100000 1024 }, SRAM { 0x20000000 768 }, SRAM2 { 0x200c0000 64 }, SRAM3 { 0x200d0000 832 }, SRAM5 { 0x201a0000 832 }, OTP { 0x0bfa0000 512 bytes })]),
    ("STM32U5[9A]..J",               &[mem!(BANK_1 { 0x08000000 2048 }, BANK_2 { 0x08200000 2048 }, SRAM { 0x20000000 768 }, SRAM2 { 0x200c0000 64 }, SRAM3 { 0x200d0000 832 }, SRAM5 { 0x201a0000 832 }, OTP { 0x0bfa0000 512 bytes })]),
    ("STM32U5[FG]..I",               &[mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08200000 1024 }, SRAM { 0x20000000 768 }, SRAM2 { 0x200c0000 64 }, SRAM3 { 0x200d0000 832 }, SRAM5 { 0x201a0000 832 }, SRAM6 { 0x20270000 512 }, OTP { 0x0bfa0000 512 bytes })]),
    ("STM32U5[FG]..J",               &[mem!(BANK_1 { 0x08000000 2048 }, BANK_2 { 0x08200000 2048 }, SRAM { 0x20000000 768 }, SRAM2 { 0x200c0000 64 }, SRAM3 { 0x200d0000 832 }, SRAM5 { 0x201a0000 832 }, SRAM6 { 0x20270000 512 }, OTP { 0x0bfa0000 512 bytes })]),
    // WB
    ("STM32WB10CC",                  &[mem!(BANK_1 { 0x08000000 320 },  SRAM { 0x20000000 12 },  SRAM2A { 0x20030000 32 }, SRAM2B { 0x20038000 4 },  SRAM2A_ICODE { 0x10000000 32 }, SRAM2B_ICODE { 0x10008000 4 })]),
    ("STM32WB15CC",                  &[mem!(BANK_1 { 0x08000000 320 },  SRAM { 0x20000000 12 },  SRAM2A { 0x20030000 32 }, SRAM2B { 0x20038000 4 },  SRAM2A_ICODE { 0x10000000 32 }, SRAM2B_ICODE { 0x10008000 4 })]),
    ("STM32WB30CE",                  &[mem!(BANK_1 { 0x08000000 512 },  SRAM { 0x20000000 32 },  SRAM2A { 0x20030000 32 }, SRAM2B { 0x20038000 32 }, SRAM2A_ICODE { 0x10000000 32 }, SRAM2B_ICODE { 0x10008000 32 })]),
    ("STM32WB50CG",                  &[mem!(BANK_1 { 0x08000000 1024 }, SRAM { 0x20000000 64 },  SRAM2A { 0x20030000 32 }, SRAM2B { 0x20038000 32 }, SRAM2A_ICODE { 0x10000000 32 }, SRAM2B_ICODE { 0x10008000 32 })]),
    ("STM32WB35.C",                  &[mem!(BANK_1 { 0x08000000 256 },  SRAM { 0x20000000 32 },  SRAM2A { 0x20030000 32 }, SRAM2B { 0x20038000 32 }, SRAM2A_ICODE { 0x10000000 32 }, SRAM2B_ICODE { 0x10008000 32 })]),
    ("STM32WB35.E",                  &[mem!(BANK_1 { 0x08000000 512 },  SRAM { 0x20000000 32 },  SRAM2A { 0x20030000 32 }, SRAM2B { 0x20038000 32 }, SRAM2A_ICODE { 0x10000000 32 }, SRAM2B_ICODE { 0x10008000 32 })]),
    ("STM32WB55.C",                  &[mem!(BANK_1 { 0x08000000 256 },  SRAM { 0x20000000 64 },  SRAM2A { 0x20030000 32 }, SRAM2B { 0x20038000 32 }, SRAM2A_ICODE { 0x10000000 32 }, SRAM2B_ICODE { 0x10008000 32 })]),
    ("STM32WB55.E",                  &[mem!(BANK_1 { 0x08000000 512 },  SRAM { 0x20000000 192 }, SRAM2A { 0x20030000 32 }, SRAM2B { 0x20038000 32 }, SRAM2A_ICODE { 0x10000000 32 }, SRAM2B_ICODE { 0x10008000 32 })]),
    ("STM32WB55.Y",                  &[mem!(BANK_1 { 0x08000000 640 },  SRAM { 0x20000000 192 }, SRAM2A { 0x20030000 32 }, SRAM2B { 0x20038000 32 }, SRAM2A_ICODE { 0x10000000 32 }, SRAM2B_ICODE { 0x10008000 32 })]),
    ("STM32WB55.G",                  &[mem!(BANK_1 { 0x08000000 1024 }, SRAM { 0x20000000 192 }, SRAM2A { 0x20030000 32 }, SRAM2B { 0x20038000 32 }, SRAM2A_ICODE { 0x10000000 32 }, SRAM2B_ICODE { 0x10008000 32 })]),
    // WBA
    ("STM32WBA6..G",                 &[mem!(BANK_1 { 0x08000000 512 },  BANK_2 { 0x08080000 512},  SRAM { 0x20000000 192 }, SRAM2 { 0x20070000 64 }, OTP { 0x0bfa0000 512 bytes })]),
    ("STM32WBA6..I",                 &[mem!(BANK_1 { 0x08000000 1024 }, BANK_2 { 0x08100000 1024}, SRAM { 0x20000000 448 }, SRAM2 { 0x20070000 64 }, OTP { 0x0bfa0000 512 bytes })]),
    ("STM32WBA...E",                 &[mem!(BANK_1 { 0x08000000 512 },  SRAM { 0x20000000 96 }, OTP { 0x0bf90000 512 bytes })]),
    ("STM32WBA...G",                 &[mem!(BANK_1 { 0x08000000 1024 }, SRAM { 0x20000000 128 }, OTP { 0x0bf90000 512 bytes })]),
    // WL
    ("STM32WL[5E]..8",               &[mem!(BANK_1 { 0x08000000 64 },  SRAM1 { 0x20000000 10 }, SRAM2 { 0x20002800 10 })]),
    ("STM32WL[5E]..B",               &[mem!(BANK_1 { 0x08000000 128 }, SRAM1 { 0x20000000 24 }, SRAM2 { 0x20006000 24 })]),
    ("STM32WL[5E]..C",               &[mem!(BANK_1 { 0x08000000 256 }, SRAM1 { 0x20000000 32 }, SRAM2 { 0x20008000 32 })]),
]);

struct FlashInfo {
    write_size: u32,
    erase_size: &'static [(u32, u32)],
    erase_value: u8,
}

#[rustfmt::skip]
#[allow(clippy::identity_op)]
static FLASH_INFO: RegexMap<&[FlashInfo]> = RegexMap::new(&[
    ("STM32C0.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  2*1024, 0)] }]),
    ("STM32F030.C",             &[FlashInfo{ erase_value: 0xFF, write_size:  4, erase_size: &[(  2*1024, 0)] }]),
    ("STM32F070.6",             &[FlashInfo{ erase_value: 0xFF, write_size:  4, erase_size: &[(  1*1024, 0)] }]),
    ("STM32F0[79].*",           &[FlashInfo{ erase_value: 0xFF, write_size:  4, erase_size: &[(  2*1024, 0)] }]),
    ("STM32F0.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  4, erase_size: &[(  1*1024, 0)] }]),
    ("STM32F10[0123].[468B]",   &[FlashInfo{ erase_value: 0xFF, write_size:  4, erase_size: &[(  1*1024, 0)] }]),
    ("STM32F1.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  4, erase_size: &[(  2*1024, 0)] }]),
    ("STM32F2.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  4, erase_size: &[( 16*1024, 4), (64*1024, 1), ( 128*1024, 0)] }]),
    ("STM32F3.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  2*1024, 0)] }]),
    ("STM32F4[23]..G",          &[
                                    FlashInfo{ erase_value: 0xFF, write_size:  4, erase_size: &[( 16*1024, 4), (64*1024, 1), ( 128*1024, 0)] },
                                    FlashInfo{ erase_value: 0xFF, write_size:  4, erase_size: &[( 16*1024, 4), (64*1024, 1), ( 128*1024, 0)] }
                                 ]),
    ("STM32F4[67]..G",          &[
                                    FlashInfo{ erase_value: 0xFF, write_size:  4, erase_size: &[( 16*1024, 4), (64*1024, 1), ( 128*1024, 0)] },
                                    FlashInfo{ erase_value: 0xFF, write_size:  4, erase_size: &[( 16*1024, 4), (64*1024, 1), ( 128*1024, 0)] }
                                 ]),
    ("STM32F4.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  4, erase_size: &[( 16*1024, 4), (64*1024, 1), ( 128*1024, 0)] }]),
    ("STM32F7[67]..[IG]",       &[
                                    FlashInfo{ erase_value: 0xFF, write_size: 16, erase_size: &[( 32*1024, 4), (128*1024, 1), ( 256*1024, 0)] }, // Single bank
                                    FlashInfo{ erase_value: 0xFF, write_size: 16, erase_size: &[( 16*1024, 4), ( 64*1024, 1), ( 128*1024, 0)] }, // Dual bank
                                 ]),
    ("STM32F7[4567].*",         &[FlashInfo{ erase_value: 0xFF, write_size: 16, erase_size: &[( 32*1024, 4), (128*1024, 1), ( 256*1024, 0)] }]),
    ("STM32F7.*",               &[FlashInfo{ erase_value: 0xFF, write_size: 16, erase_size: &[( 16*1024, 4), (64*1024, 1), ( 128*1024, 0)] }]),
    ("STM32G0...C",             &[
                                    FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  2*1024, 0)] },
                                    FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  2*1024, 0)] },
                                 ]),
    ("STM32G0.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  2*1024, 0)] }]),
    ("STM32G4[78].*",           &[
                                    FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  4*1024, 0)] }, // Single bank
                                    FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  2*1024, 0)] }  // Dual bank
                                 ]),
    ("STM32G4.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  2*1024, 0)] }]),
    ("STM32H5.*",               &[FlashInfo{ erase_value: 0xFF, write_size: 16, erase_size: &[(  8*1024, 0)] }]),
    ("STM32H7[RS].*",           &[FlashInfo{ erase_value: 0xFF, write_size: 16, erase_size: &[(  8*1024, 0)] }]),
    ("STM32H7[AB].*",           &[FlashInfo{ erase_value: 0xFF, write_size: 32, erase_size: &[(  8*1024, 0)] }]),
    ("STM32H7.*",               &[FlashInfo{ erase_value: 0xFF, write_size: 32, erase_size: &[(128*1024, 0)] }]),
    ("STM32L4[PQRS].*",         &[
                                    FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  8*1024, 0)] }, // Single bank
                                    FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  4*1024, 0)] }, // Dual bank
                                 ]),
    ("STM32L4.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  2*1024, 0)] }]),
    ("STM32L0.*",               &[FlashInfo{ erase_value: 0x00, write_size:  4, erase_size: &[(     128, 0)] }]),
    ("STM32L1.*",               &[FlashInfo{ erase_value: 0x00, write_size:  4, erase_size: &[(     256, 0)] }]),
    ("STM32L5...E",             &[
                                    FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  4*1024, 0)] }, // Single bank
                                    FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  2*1024, 0)] }, // Dual bank
                                 ]),
    ("STM32L5.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  4*1024, 0)] }]),
    ("STM32U0.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  2*1024, 0)] }]),
    ("STM32U5.*",               &[FlashInfo{ erase_value: 0xFF, write_size: 16, erase_size: &[(  8*1024, 0)] }]),
    ("STM32WBA.*",              &[FlashInfo{ erase_value: 0xFF, write_size: 16, erase_size: &[(  8*1024, 0)] }]),
    ("STM32WB1.*",              &[FlashInfo{ erase_value: 0x00, write_size:  8, erase_size: &[(  2*1024, 0)] }]),
    ("STM32WB.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  4*1024, 0)] }]),
    ("STM32WL.*",               &[FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  2*1024, 0)] }]),
    ("STM32.*",                 &[FlashInfo{ erase_value: 0xFF, write_size:  8, erase_size: &[(  2*1024, 0)] }]),
]);

pub fn get(chip: &str) -> Vec<Vec<Memory>> {
    let mems_variations = *MEMS.must_get(chip);
    let flash_variations = FLASH_INFO.must_get(chip);

    assert_eq!(
        mems_variations.len(),
        flash_variations.len(),
        "All memory variants must be present in both the mems and the flash info: {chip}"
    );

    mems_variations
        .into_iter()
        .zip(flash_variations.into_iter())
        .map(|(mems, flash)| {
            let mut res = Vec::new();

            for mem in *mems {
                if mem.name.starts_with("BANK") {
                    if flash.erase_size.len() == 1 || mem.size <= flash.erase_size[0].0 * flash.erase_size[0].1 {
                        res.push(Memory {
                            name: mem.name.to_string(),
                            address: mem.address,
                            size: mem.size,
                            kind: memory::Kind::Flash,
                            settings: Some(Settings {
                                write_size: flash.write_size,
                                erase_size: flash.erase_size[0].0,
                                erase_value: flash.erase_value,
                            }),
                            access: mem.access,
                        });
                    } else {
                        let mut offs = 0;
                        for (i, &(erase_size, count)) in flash.erase_size.iter().enumerate() {
                            if offs >= mem.size {
                                break;
                            }
                            let left = mem.size - offs;
                            let mut size = left;
                            if i != flash.erase_size.len() - 1 {
                                size = size.min(erase_size * count);
                            }
                            #[allow(clippy::redundant_field_names)]
                            res.push(Memory {
                                name: format!("{}_REGION_{}", mem.name, i + 1),
                                address: mem.address + offs,
                                size: size,
                                kind: memory::Kind::Flash,
                                settings: Some(Settings {
                                    write_size: flash.write_size,
                                    erase_size: erase_size,
                                    erase_value: flash.erase_value,
                                }),
                                access: mem.access,
                            });
                            offs += size;
                        }
                    }
                } else if mem.name == "OTP" {
                    res.push(Memory {
                        name: mem.name.to_string(),
                        address: mem.address,
                        size: mem.size,
                        kind: memory::Kind::Flash,
                        settings: Some(Settings {
                            write_size: flash.write_size,
                            erase_size: 0,
                            erase_value: flash.erase_value,
                        }),
                        access: mem.access,
                    });
                } else if mem.name.starts_with("EEPROM") {
                    res.push(Memory {
                        name: mem.name.to_string(),
                        address: mem.address,
                        size: mem.size,
                        kind: memory::Kind::Eeprom,
                        settings: None,
                        access: mem.access,
                    });
                } else {
                    let mut kind = memory::Kind::Ram;
                    if mem.name.contains("FLASH") || mem.name.contains("AXIICP") {
                        kind = memory::Kind::Flash;
                    }
                    res.push(Memory {
                        name: mem.name.to_string(),
                        address: mem.address,
                        size: mem.size,
                        kind,
                        settings: None,
                        access: mem.access,
                    });
                }
            }

            res.sort_by_key(|m| (m.address, m.name.clone()));

            res
        })
        .collect()
}
