#![allow(dead_code)]

use stm32_data_macros::EnumDebug;

#[derive(Debug)]
struct A {
    pub b: String,
}

#[derive(EnumDebug)]
enum C {
    D(A),
    E,
}
