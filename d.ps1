<# #>
param (
    [Parameter(Mandatory = $true)]
    [string]$CMD,

    [string]$peri,
    [string]$logLevel
)

$REV = ((Select-String -Path ".\d" -Pattern "^REV=") -split "=")[1]

function xargs {
    [CmdletBinding()]
    param(
        [Parameter(ValueFromPipeline)]
        [string] $InputObject,

        [int] $MaxArgs = [int]::MaxValue,
        [int] $MaxLength = 30000,
        [switch] $NullDelimited,

        # All unnamed args: first is command, rest are prefix args
        [Parameter(Position=0, ValueFromRemainingArguments)]
        [string[]] $CommandAndPrefix
    )

    begin {
        if ($CommandAndPrefix.Count -lt 1) {
            throw "xargs requires at least a command name."
        }

        $Command    = $CommandAndPrefix[0]
        $PrefixArgs = $CommandAndPrefix[1..($CommandAndPrefix.Count - 1)]

        $batch  = @()
        $buffer = ""
    }

    process {
        if ($NullDelimited) {
            $buffer += $InputObject
            $parts   = $buffer -split "`0"
            $buffer  = $parts[-1]
            $args    = $parts[0..($parts.Count - 2)]
        } else {
            $args = @($InputObject)
        }

        foreach ($arg in $args) {
            $testBatch = $batch + $arg

            $cmdLine = $Command + " " +
                       ($PrefixArgs -join " ") + " " +
                       ($testBatch -join " ")

            $tooLong = $cmdLine.Length -ge $MaxLength
            $tooMany = $testBatch.Count -gt $MaxArgs

            if ($tooLong -or $tooMany) {
                if ($batch.Count -gt 0) {
                    & $Command @PrefixArgs @batch
                }
                $batch = @($arg)
            } else {
                $batch += $arg
            }
        }
    }

    end {
        if ($NullDelimited -and $buffer.Length -gt 0) {
            $batch += $buffer
        }

        if ($batch.Count -gt 0) {
            & $Command @PrefixArgs @batch
        }
    }
}



Switch ($CMD) {
    "download-all" {
        rm -r -Force ./sources/ -ErrorAction SilentlyContinue
        git clone https://github.com/embassy-rs/stm32-data-sources.git ./sources/ --depth 1
        cd ./sources/
        git fetch origin $REV
        git checkout $REV
        cd ..
    }
    "update-all" {
        if (-Not (Test-Path -Path ".\sources")) {
            .\d "download-all"
        } else {
            cd ./sources/
            git fetch origin $REV
            git checkout $REV
            cd ..
        }
    }
    "install-chiptool" {
        cargo install --git https://github.com/embassy-rs/chiptool
    }
    "extract-all" {
        cargo run --release --bin extract-all $peri
    }
    "merge-regs" {
        cargo run --release --bin merge-regs $peri
    }
   "transform" {
        chiptool transform --input regs_merged.yaml --output regs_merged.yaml --transform transforms/$peri.yaml
    }
    "gen" {
        if ($peri -ne "" -and $logLevel -ne "") {
            cargo run --release --bin stm32-data-gen -- --filter $peri --log-level $logLevel
        } elseif ($peri -ne "") {
            cargo run --release --bin stm32-data-gen -- --filter $peri
        } else {
            cargo run --release --bin stm32-data-gen
        }
    }
    "gen-all" {
        cargo run --release --bin stm32-data-gen 
        cargo run --release --bin stm32-metapac-gen
        cd build/stm32-metapac

        ls -Recurse -Filter '*.rs' | Where-Object { $_.FullName -notmatch 'target' } | % { $_.FullName } | Resolve-Path -Relative `
            | xargs rustfmt --skip-children --unstable-features --edition 2021
    }
    default {
        echo "unknown command"
    }
}