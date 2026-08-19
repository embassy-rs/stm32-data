``` python 
import re, glob

seen = set()
for f in glob.glob("sources/cubedb/mcu/*.xml"):
    text = open(f, encoding="utf-8", errors="ignore").read()
    for m in re.finditer(r'<IP\b[^>]*\bName="DFSDM"[^>]*/?>', text):
        tag = m.group(0)
        cfg  = re.search(r'ConfigFile="([^"]+)"', tag)
        inst = re.search(r'InstanceName="([^"]+)"', tag)
        ver  = re.search(r'Version="([^"]+)"', tag)
        seen.add((
            cfg.group(1)  if cfg  else "?",
            inst.group(1) if inst else "?",
            ver.group(1)  if ver  else "?",
        ))

for row in sorted(seen):
    print(*row)
```
```
DFSDM-STM32F412 DFSDM1 dfsdm1_v1_0_4ch_Cube
DFSDM-STM32F413 DFSDM1 dfsdm1_v1_0_4ch_F413_Cube
DFSDM-STM32F413 DFSDM2 dfsdm1_F413_v1_0_Cube
DFSDM-STM32F7xx DFSDM1 dfsdm1_F7_v1_0_Cube
DFSDM-STM32H7Axx DFSDM1 dfsdm1_v1_0_H7A_8ch8f_Cube
DFSDM-STM32H7Axx DFSDM2 dfsdm1_v1_0_H7A_2ch_Cube
DFSDM-STM32H7xx DFSDM1 dfsdm1_v1_0_H7_Cube
DFSDM-STM32L49x DFSDM1 dfsdm1_v1_0_L49_Cube
DFSDM-STM32L4PQx DFSDM1 dfsdm1_v1_0_4ch_L4PQx_Cube
DFSDM-STM32L4Rx DFSDM1 dfsdm1_v1_0_L4R_Cube
DFSDM-STM32L4x1 DFSDM1 dfsdm1_v1_0_4ch_L4x1_Cube
DFSDM-STM32L4xx DFSDM1 dfsdm1_v1_0_Cube
DFSDM-STM32L5xx DFSDM1 dfsdm1_v2_1_L5_Cube
DFSDM-STM32MP13xx DFSDM1 dfsdm1_v1_0_4ch_MP13_Cube
DFSDM-STM32MP1xx DFSDM1 STM32MP_dfsdm1_v2_1
```

``` bash
sha256sum *.yaml | sort | uniq -w 64`
```
```
26f392e16acce17c671315846b06986eec1f53a2a068ad0a1c07d11a43cb8786  f413_DFSDM1.yaml
28f5e084c539bafe51062149f49a144c7617979412e8ba7a757fb5f6be24043d  f7x7_DFSDM.yaml
36f97c9f04cb41a30bf5a8807c812d014ce4990fe286bb433e11bc5e5f4a383a  l552_DFSDM1.yaml
3b45d3ebe4b39ea7509324a812a05b70c6c26f3435d9774a87a77a1fcaded8d0  h743_DFSDM.yaml
3ca063d5ba0185ce9cff55b3b89945e6a3ca3021a3bba2c7576bc3f56cc13a38  f765_DFSDM1.yaml
61e660ea53e52d8a3b62a90becd90d7df7d2b30f51c8823862e7c5c6749e0820  h735_DFSDM.yaml
659ca84cb7bdbb62cfba6bd1749b4fb2e4c207b748f666f70145eb42496ca339  l4p5_DFSDM1.yaml
78d2ca98021536242421f91f6f84656243f577fd0815021e8e169d295f7b9bd2  l4r5_DFSDM1.yaml
a36debeb6c7802a4399fe17e784aa4f3eab406f8e83cfa287f7e8dc8d12510aa  mp153_DFSDM1.yaml
b1fc1ebb5675f43be274b17b818b1dd8b9d0176f30176f1d1d91669ac07cf6b2  f412_DFSDM.yaml
de520e211fc96ae2ab30f7dcdf72a06f7bb355392213d1416e42b555271108a9  l4x6_DFSDM1.yaml
e391cb862a2bc4607a9878cad8db12a229f1e66294fa17947acf4116dade7821  h7b3_DFSDM1.yaml
```
