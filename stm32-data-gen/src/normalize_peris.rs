#[rustfmt::skip]
static NORMALIZE: &[(&str, &str)] = &[
    ("ADC", "ADC1"),
    ("DAC", "DAC1"),
    ("HRTIM", "HRTIM1"),
    ("HDMI_CEC", "CEC"),
    ("SUBGHZ", "SUBGHZSPI"),
    ("USB_DRD_FS", "USB"),
    ("SBS", "SYSCFG"),
    ("SPDIFRX", "SPDIFRX1")
];

pub fn normalize_peri_name(name: &str) -> &str {
    if let Some((_, res)) = NORMALIZE.iter().find(|(n, _)| *n == name) {
        return res;
    }
    name
}
