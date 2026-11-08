# tazuna's installer for Windows: the newest release into ~\.local\bin ($env:TAZUNA_INSTALL_DIR elsewhere), which goes on
# your user PATH unless $env:TAZUNA_NO_MODIFY_PATH is set. Later `tazuna update` installs newer ones. x64 build; Windows
# on ARM runs it emulated.
#   irm https://raw.githubusercontent.com/suwandre/tazuna/main/install.ps1 | iex
# In a script block, so the settings below stay out of the session that runs it.
& {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue' # Windows PowerShell's progress bar slows the download many times over
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $repo = 'suwandre/tazuna'
    $dir = if ($env:TAZUNA_INSTALL_DIR) { $env:TAZUNA_INSTALL_DIR } else { Join-Path $HOME '.local\bin' }
    New-Item -ItemType Directory -Force $dir | Out-Null
    $tmp = Join-Path ([IO.Path]::GetTempPath()) "tazuna-install-$PID"
    New-Item -ItemType Directory -Force $tmp | Out-Null
    try {
        $zip = Join-Path $tmp 'tazuna.zip'
        Invoke-WebRequest -UseBasicParsing "https://github.com/$repo/releases/latest/download/tazuna-x86_64-pc-windows-msvc.zip" -OutFile $zip
        Expand-Archive -Path $zip -DestinationPath $tmp -Force
        $exe = Join-Path $dir 'tazuna.exe'
        # a running tazuna cannot be overwritten but can be moved aside; it is removed on its next start or here next time
        Remove-Item "$exe.old" -Force -ErrorAction SilentlyContinue
        if (Test-Path $exe) { Move-Item -Force $exe "$exe.old" }
        Move-Item (Join-Path $tmp 'tazuna.exe') $exe
        Remove-Item "$exe.old" -Force -ErrorAction SilentlyContinue
        Write-Host "installed $(& $exe --version) in $dir"
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
    # the user PATH as stored (unexpanded, so entries like %USERPROFILE%\bin stay as they are)
    $key = Get-Item 'HKCU:\Environment'
    $path = $key.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
    $on = ($path -split ';') + ($env:Path -split ';') | Where-Object { $_ -and ([Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -eq $dir.TrimEnd('\')) }
    if (-not $on) {
        if ($env:TAZUNA_NO_MODIFY_PATH) {
            Write-Host "$dir is not on your PATH: add it to run tazuna by name"
        } else {
            Set-ItemProperty 'HKCU:\Environment' -Name Path -Type ExpandString -Value ((@($path -split ';' | Where-Object { $_ }) + $dir) -join ';')
            # tells running programs the environment changed (setting a variable to nothing broadcasts it)
            [Environment]::SetEnvironmentVariable('TAZUNA_INSTALL_REFRESH', $null, 'User')
            $env:Path = "$env:Path;$dir"
            Write-Host "added $dir to your PATH (new terminals have it)"
        }
    }
    if (-not (Get-Command claude -ErrorAction SilentlyContinue)) {
        Write-Host "tazuna drives the claude CLI: install Claude Code with its native installer (claude.exe) and log in"
    }
    # where tazuna looks for Git Bash (src/platform.rs)
    $bash = @($env:TAZUNA_BASH, $env:CLAUDE_CODE_GIT_BASH_PATH, 'C:\Program Files\Git\bin\bash.exe', 'C:\Program Files (x86)\Git\bin\bash.exe')
    if (-not ($bash | Where-Object { $_ -and (Test-Path $_) })) {
        Write-Host "tazuna runs commands in Git Bash: install Git for Windows (https://git-scm.com/download/win)"
    }
}
