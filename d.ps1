<# #>
param (
    [Parameter(Mandatory=$true)]
    [string]$CMD,

    [string]$peri
)

$REV="74b97817d4c4ed0db9d19a8eac46720b3c5b0d57"

Switch ($CMD)
{
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
            } else {
                rm "tmp/$peri/$f.yaml"
                echo FAIL
            }

        }
    }
    default {
        echo "unknown command"
    }
}
