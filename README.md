# stm32-data

`stm32-data` is a project aiming to produce clean machine-readable data about the STM32 microcontroller
families, including:

- :heavy_check_mark: Base chip information
  - RAM, flash
  - Packages
- :heavy_check_mark: Peripheral addresses and interrupts
- :heavy_check_mark: Interrupts
- :heavy_check_mark: GPIO AlternateFunction mappings (for all families except F1)
- :x: GPIO mappings for F1
- :construction: Register blocks for all peripherals
- :heavy_check_mark: DMA stream mappings
- :heavy_check_mark: Per-package pinouts
- :heavy_check_mark: Links to applicable reference manuals, datasheets, appnotes PDFs.

:heavy_check_mark: = done, :construction: = work in progress, :x: = to do

The generated JSON files are available [here in the `stm32-data-generated`](https://github.com/embassy-rs/stm32-data-generated/blob/main/data) repo.

## stm32-metapac

If you're looking for the API docs for `stm32-metapac` customized to a
particular chip, they are available here: https://docs.embassy.dev/stm32-metapac

The generated PAC is available [here in the `stm32-data-generated`](https://github.com/embassy-rs/stm32-data-generated/blob/main/stm32-metapac) repo.

## Data sources

These are the data sources currently used.

- STM32Cube database:
  - List of all MCUs
  - Peripheral versions
  - Flash, RAM size
  - GPIO AF mappings
  - Interrupt mappings (which signals from which peripherals are OR'd into a NVIC interrupt line)
  - DMA channel request mappings
  - Packages and package pinouts (TODO)
- STM32Cubeprog database
  - Flash, RAM size
- STM32 mcufinder:
  - Links to documentation PDFs (reference manuals, datasheets...)
- STM32 HAL headers:
  - interrupt number, name
  - peripheral addresses
- stm32-rs SVDs: register blocks.

For register blocks, YAMLs are intially extracted from SVDs, manually cleaned up and committed. From this point on, they're manually maintained.
We don't maintain "patches" for registers unlike the `stm32-rs` project. This has two advantages:

- Fixing mistakes and typos in the SVDs is now much easier (and yes, there are A LOT of those). It doesn't require a complicated patching system, you just edit the YAML to fix the mistake instead of writing a patch that will fix it when applied.
- Ensuring consistency for the same peripherals between chips. (i.e. we check in a single `i2c_v2.yaml` and ensure it is good, vs trying to patch wildly differing SVDs for what's an identical IP block into being consistent). The `stm32-rs` project doesn't have this as a goal, while we do. Writing a single HAL for all stm32 chips (such as  [the `embassy-stm32` HAL](https://github.com/embassy-rs/embassy/tree/main/embassy-stm32)) is impossible otherwise.

## Generate the `stm32-metapac` crate

- Run `./d download-all`

  > This will download all the required data sources into `sources/`.

- Run `cargo run --release --bin stm32-data-gen`

  > This generates all the intermediate JSON's in `build/data/`.
  >
  > > Assignments of registers to peripherals is done in a [perimap](#peripheral-mapping-perimap) and fixes to registers can be done in the files located in `data/registers`.

- Run `cargo run --release --bin stm32-metapac-gen`

> This generates the `stm32-metapac` crate into `build/stm32-metapac/`.

## Adding support for a new peripheral

This will help you add support for a new peripheral to all STM32 families. (Please take the time to add it for all families, even if you personally
are only interested in one. It's easier than it looks, and doing all families at once is significantly less work than adding one now then having to revisit everything later when adding more. It also helps massively in catching mistakes and inconsistencies in the source SVDs.)

- First, make sure you can generate the YAMLs following the steps above. You should be able to regenerate the already-committed yamls, and end up with no diff.
- Install chiptool with `./d install-chiptool`
- Run `./d extract-all LPUART1`. This'll output a bunch of yamls in `tmp/LPUART1`. NOTE sometimes peripherals have a number sometimes not (`LPUART1` vs `LPUART`). You might want to try both and merge the outputted YAMLs into a single directory.
- Diff them between themselves, to identify differences. The differences can either be:
  - 1: Legitimate differences between families, because there are different LPUART versions. For example, added registers/fields.
  - 2: SVD inconsistencies, like different register names for the same register
  - 3: SVD mistakes (yes, there are some)
  - 4: Missing stuff in SVDs, usually enums or doc descriptions.
- Identify how many actually-different (incompatible) versions of the peripheral exist, as we must _not_ merge them. Name them v1, v2.. (if possible, by order of chip release date, see [here](https://docs.google.com/spreadsheets/d/1-R-AjYrMLL2_623G-AFN2A9THMf8FFMpFD4Kq-owPmI/edit#gid=1972450814).
- For each version, pick the "best" YAML (the one that has less enums/docs missing), place them in `data/registers/lpuart_vX.yaml`
- Cleanup the register yamls (see below).
- Minimize the diff between each pair of versions. For example between `lpuart_v1.yaml` and `lpuart_v2.yaml`. If one is missing enums or descriptions, copy it from another.
- Make sure the block
- Add entries to [`perimap`](https://github.com/embassy-rs/stm32-data/blob/main/stm32-data-gen/src/chips.rs#L109), see below.
- Regen, then: 
  - Check `data/chips/*.yaml` has the right `block: lpuart_vX/LPUART` fields.
  - Ensure a successful build of the affected pac. e.g.
    ```
    cd build/stm32-metapac
    cargo build --features stm32u5a9nj,memory-x --target thumbv8m.main-none-eabihf 
    ``` 

Please separate manual changes and changes resulting from regen in separate commits. It helps tremendously with review and rebasing/merging.

## Register cleanup

SVDs have some widespread annoyances that should be fixed when adding register YAMLs to this repo. Check out `chiptool` transforms, they can help in speeding up the cleanups.

- Remove "useless prefixes". For example if all regs in the `RNG` peripheral are named `RNG_FOO`, `RNG_BAR`, the `RNG_` peripheral is not conveying any useful information at all, and must go.
- Remove "useless enums". Useless enums is one of the biggest cause of slow compilation times in STM32 PACs.
  - 0=disabled, 1=enabled. Common in `xxEN` and `xxIE` fields. If a field says "enable foo" and is one bit, it's obvious "true" means enabled and "false" means disabled.
  - "Write 0/1 to clear" enums, common in `xxIF` fields.
  - Check out the `DeleteEnums` chiptool transforms.
- Convert repeated registers or fields (like `FOO0 FOO1, FOO2, FOO3`) to arrays `FOO[n]`.
  - Check out the `MakeRegisterArray`, `MakeFieldArray` chiptool transforms.
- Use `chiptool fmt` on each of the register yamls.

## Adding support for a new family (RCC)

NOTE: At the time of writing all families are supported, so this is only useful in particular situations, for example if you want to regenerate an RCC register yaml from scratch.

Adding support for a new familiy is mostly a matter of adding support for RCC.

Now extract the RCC peripheral registers: `./d extract-all RCC --transform ./transform-RCC.yaml`

Note that we have used a transform to mechanically clean up some of the RCC
definitions. This will produce a YAML file for each chip model in `./tmp/RCC`.

Sometimes the peripheral name will not match the name defined in the SVD files, check the SVD file for the correct peripheral name.

RCC registers are usually identical within a family, except they have different subsets of the enable/reset bits
because different chips have different amounts of peripherals. (i.e. one particular bit position in one particular
register is either present or not, but will never have different meanings in different chips of the same family.)

To verify they're indeed subsets, choose the model with the largest peripheral set possible (e.g.
the STM32G081) and compare its YAML against each of the other models' to verify
that they are all mutually consistent.

Finally, we can merge

```
./merge_regs.py tmp/RCC/g0*.yaml
```

This will produce `regs_merged.yaml`, which we can copy into its final resting
place:

```
mv regs_merged.yaml data/registers/rcc_g0.yaml
```

To assign these newly generated registers to peripherals, utilize the mapping done in `parse.py`.
An example mapping can be seen in the following snippet

```
('STM32G0.*:RCC:.*', 'rcc_g0/RCC'),
```

such mapping assignes the `rcc_g0/RCC` register block to the `RCC` peripheral in every device from the `STM32G0` family.

## Peripheral mapping (perimap)

The `stm32-data-gen` binary has a map to match peripherals to the right version in all chips, the [perimap](https://github.com/embassy-rs/stm32-data/blob/main/stm32-data-gen/src/chips.rs#L109).

When parsing a chip, for each peripheral a "key" string is constructed using this format: `CHIP:PERIPHERAL_NAME:IP_NAME:IP_VERSION`, where:

- `CHIP`: full chip name, for example `STM32L443CC`
- `PERIPHERAL_NAME`: peripheral name, for example `SPI3`. Corresponds to `IP.InstanceName` in the [MCU XML](https://github.com/embassy-rs/stm32-data-sources/blob/949842b4b8742e6bc70ae29a0ede14b4066db819/cubedb/mcu/STM32L443CCYx.xml#L38).
- `IP_NAME`: IP name, for example `SPI`. Corresponds to `IP.Name` in the [MCU XML](https://github.com/embassy-rs/stm32-data-sources/blob/949842b4b8742e6bc70ae29a0ede14b4066db819/cubedb/mcu/STM32L443CCYx.xml#L38).
- `IP_VERSION`: IP version, for example `spi2s1_v3_3_Cube`, Corresponds to `IP.Version` in the [MCU XML](https://github.com/embassy-rs/stm32-data-sources/blob/949842b4b8742e6bc70ae29a0ede14b4066db819/cubedb/mcu/STM32L443CCYx.xml#L38).

`perimap` entries are regexes matching on the above "key" string. First regex that matches wins.

The IP version is particularly useful. It is an ST-internal "IP version" that's incremented every time changes are made to the peripheral, so it
correlates very well to changes in the peripheral's register interface.

You should prefer matching peripherals by IP version whenever possible. For example:

```
('.*:SPI:spi2s1_v2_2', 'spi_v1/SPI'),
('.*:SPI:spi2s1_v3_2', 'spi_v2/SPI'),
```

Sometimes it's not possible to map by IP version, and we have to map by chip name. For example:

```
('STM32H7.*:FLASH:.*', 'flash_h7/FLASH'),
('STM32F0.*:FLASH:.*', 'flash_f0/FLASH'),
('STM32F1.*:FLASH:.*', 'flash_f1/FLASH'),
('STM32F3.*:FLASH:.*', 'flash_f3/FLASH'),
('STM32F4.*:FLASH:.*', 'flash_f4/FLASH'),
...etc
```

Sometimes even the same IP name+version in the same chip family has different registers (different instances of the IP are configured differently), so we have to map by chip name AND peripheral instance name. This should be the last resort. For example:

```
('STM32F7.*:TIM1:.*', 'timer_v1/TIM_ADV'),
('STM32F7.*:TIM8:.*', 'timer_v1/TIM_ADV'),
('.*TIM\d.*:gptimer.*', 'timer_v1/TIM_GP16'),
```

### Peripheral versions

The versions of peripherals can be found in the table [here](https://docs.google.com/spreadsheets/d/1-R-AjYrMLL2_623G-AFN2A9THMf8FFMpFD4Kq-owPmI/edit#gid=0).
