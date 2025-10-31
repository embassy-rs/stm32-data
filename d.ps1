<# #>
param (
    [Parameter(Mandatory = $true)]
    [string]$CMD,

    [string]$peri
)

$REV = "db4473fae6a41fcad7f5c824dcaadba3a6e060e9"

Switch ($CMD) {
    "download-all" {
        rm -r -Force ./sources/ -ErrorAction SilentlyContinue
        git clone https://github.com/embassy-rs/stm32-data-sources.git ./sources/
        cd ./sources/
        git checkout $REV
        cd ..
    }
    "install-chiptool" {
        cargo install --git https://github.com/embassy-rs/chiptool
    }
    "extract-all" {
        rm -r -Force tmp/$peri -ErrorAction SilentlyContinue
        mkdir tmp/$peri | Out-Null

        ls sources/svd | foreach-object {
            $f = $_.Name.TrimStart("stm32").TrimEnd(".svd")
            echo $f

            echo "processing $f ..."
            chiptool extract-peripheral --svd "sources/svd/stm32$f.svd" --peripheral "$peri" > "tmp/$peri/$f.yaml" 2> "tmp/$peri/$f.err"
            if ($LASTEXITCODE -eq 0) {
                rm "tmp/$peri/$f.err"
                echo OK
            }
            else {
                rm "tmp/$peri/$f.yaml"
                echo FAIL
            }

        }
    }
    "gen" {
        rm -r -Force build/data -ErrorAction SilentlyContinue
        cargo run --release --bin stm32-data-gen
    }
    "gen-all" {
        rm -r -Force build/data -ErrorAction SilentlyContinue
        rm -r -Force build/stm32-metapac -ErrorAction SilentlyContinue
        cargo run --release --bin stm32-data-gen 
        cargo run --release --bin stm32-metapac-gen
        cd build/stm32-metapac

        $files = ls -Recurse -Filter '*.rs' | Where-Object { $_.FullName -notmatch 'target' } | % { $_.FullName } | Resolve-Path -Relative
        $counter = [pscustomobject] @{ Value = 0 }
        $files | Group-Object -Property { [math]::Floor($counter.Value++ / 200 ) } | % { rustfmt --skip-children --unstable-features --edition 2021 $_.Group }
    }
    default {
        echo "unknown command"
    }
}